#!/usr/bin/env bash
set -euo pipefail

API_BASE=${API_BASE:-http://127.0.0.1:8000}
CODEX_BASE=${CODEX_BASE:-$API_BASE/api/codex}
EMAIL=${EMAIL:-codex-cli@example.com}
PASSWORD=${PASSWORD:-codex-cli}
NAME=${NAME:-"Codex CLI"}
REPOSITORY_NAME=${REPOSITORY_NAME:-codex-cloud}
GIT_URL=${GIT_URL:-git@github.com:cklxx/codex-cloud.git}
BRANCH=${BRANCH:-main}
ENV_ID=${ENV_ID:-local-dev}
PROMPT=${PROMPT:-"Verify Codex CLI compatibility"}
ATTEMPTS=${ATTEMPTS:-1}
ARTIFACT_DIR=${ARTIFACT_DIR:-}

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to run the CLI contract test" >&2
  exit 1
fi

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

log() {
  echo "[cli-contract] $*" >&2
}

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/../.." && pwd)
CARGO_ROOT="$REPO_ROOT/codex-rs"

ensure_repository() {
  local payload
  payload=$(cat <<JSON
{\"name\":\"$REPOSITORY_NAME\",\"git_url\":\"$GIT_URL\",\"default_branch\":\"$BRANCH\"}
JSON
  )

  if REPO_JSON=$(request POST "$API_BASE/repositories" \
    -H 'Content-Type: application/json' \
    -H "$AUTH" \
    -d "$payload" 2>/dev/null); then
    REPO_ID=$(python - <<'PY' <<<"$REPO_JSON"
import json, sys
print(json.load(sys.stdin)["id"])
PY
    )
    log "Repository created with id $REPO_ID"
    echo "$REPO_ID"
    return
  fi

  log "Repository may already exist; attempting lookup"
  local list
  list=$(request GET "$API_BASE/repositories" -H "$AUTH")
  REPO_ID=$(printf '%s' "$list" | python - "$GIT_URL" <<'PY'
import json, sys

def norm(url: str) -> str:
    return url.strip().rstrip('/').lower()

target = norm(sys.argv[1])
data = json.loads(sys.stdin.read() or "[]")
for row in data:
    if norm(row.get("git_url", "")) == target:
        print(row["id"])
        break
else:
    raise SystemExit(1)
PY
  ) || {
    log "Failed to locate repository with git_url=$GIT_URL"
    exit 1
  }
  log "Reusing existing repository id $REPO_ID"
  echo "$REPO_ID"
}

request() {
  local method=$1
  local url=$2
  shift 2
  curl --silent --show-error --fail-with-body -X "$method" "$url" "$@"
}

ensure_environment() {
  local payload
  payload=$(cat <<JSON
{\"id\":\"$ENV_ID\",\"label\":\"$ENV_ID\",\"repository_id\":\"$1\",\"branch\":\"$BRANCH\",\"is_pinned\":true}
JSON
  )

  if ! request POST "$API_BASE/environments" \
    -H 'Content-Type: application/json' \
    -H "$AUTH" \
    -d "$payload" >/dev/null 2>&1; then
    log "Environment may already exist; verifying availability"
  fi

  local envs
  envs=$(request GET "$CODEX_BASE/environments" -H "$AUTH")
  if python - "$ENV_ID" <<'PY' <<<"$envs"
import json, sys

target = sys.argv[1]
data = json.loads(sys.stdin.read() or "[]")
if not any(row.get("id") == target for row in data):
    raise SystemExit(1)
PY
  then
    log "Environment $ENV_ID is available"
  else
    log "Environment $ENV_ID not found after verification"
    exit 1
  fi
}

log "Ensuring bootstrap user exists"
if ! request POST "$API_BASE/auth/users" \
  -H 'Content-Type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\",\"name\":\"$NAME\"}" > /dev/null; then
  log "User may already exist; continuing"
fi

log "Requesting access token"
LOGIN_JSON=$(request POST "$API_BASE/auth/session" \
  -H 'Content-Type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}")
TOKEN=$(python - <<'PY' <<<"$LOGIN_JSON"
import json, sys
print(json.load(sys.stdin)["access_token"])
PY
)
AUTH="Authorization: Bearer $TOKEN"

log "Creating repository"
REPO_ID=$(ensure_repository)

log "Creating environment $ENV_ID"
ensure_environment "$REPO_ID"

AUTH_JSON="$TMP_DIR/auth.json"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
ID_TOKEN="eyJhbGciOiAibm9uZSIsICJ0eXAiOiAiSldUIn0.eyJzdWIiOiAiMTIzIn0.signature"
cat >"$AUTH_JSON" <<JSON
{
  "OPENAI_API_KEY": null,
  "tokens": {
    "id_token": "$ID_TOKEN",
    "access_token": "$TOKEN",
    "refresh_token": "local-refresh",
    "account_id": "local-cli"
  },
  "last_refresh": "$TIMESTAMP"
}
JSON

LOG_FILE="$TMP_DIR/codex-cli.log"
CONTEXT_JSON="$TMP_DIR/context.json"
TASKS_JSON="$TMP_DIR/tasks.json"

log "Running codex cloud exec"
pushd "$CARGO_ROOT" >/dev/null
set +e
CODEX_HOME="$TMP_DIR" \
CODEX_CLOUD_TASKS_BASE_URL="$CODEX_BASE" \
cargo run -p codex-cli --quiet -- cloud exec --env "$ENV_ID" --attempts "$ATTEMPTS" "$PROMPT" \
  2>&1 | tee "$LOG_FILE"
CLI_STATUS=${PIPESTATUS[0]}
set -e
popd >/dev/null

if request GET "$API_BASE/tasks" -H "$AUTH" >"$TASKS_JSON" 2>/dev/null; then
  log "Captured task snapshot"
else
  log "Unable to capture task snapshot"
  rm -f "$TASKS_JSON"
fi

cat >"$CONTEXT_JSON" <<JSON
{
  "timestamp": "$TIMESTAMP",
  "api_base": "$API_BASE",
  "codex_base": "$CODEX_BASE",
  "email": "$EMAIL",
  "repository": {
    "id": "$REPO_ID",
    "name": "$REPOSITORY_NAME",
    "git_url": "$GIT_URL",
    "branch": "$BRANCH"
  },
  "environment_id": "$ENV_ID",
  "prompt": "$PROMPT",
  "attempts": "$ATTEMPTS"
}
JSON

if [[ -n "$ARTIFACT_DIR" ]]; then
  mkdir -p "$ARTIFACT_DIR"
  cp "$AUTH_JSON" "$ARTIFACT_DIR/auth.json"
  cp "$LOG_FILE" "$ARTIFACT_DIR/cli-output.log"
  cp "$CONTEXT_JSON" "$ARTIFACT_DIR/context.json"
  if [[ -f "$TASKS_JSON" ]]; then
    cp "$TASKS_JSON" "$ARTIFACT_DIR/tasks.json"
  fi
fi

exit "$CLI_STATUS"
