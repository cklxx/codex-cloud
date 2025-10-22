#!/usr/bin/env bash
set -euo pipefail

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "hyperfine is required. Install it from https://github.com/sharkdp/hyperfine" >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required for benchmarking" >&2
  exit 1
fi

API_BASE=${API_BASE:-http://127.0.0.1:8000}
RUNS=${RUNS:-15}
WARMUP=${WARMUP:-3}
OUTPUT_DIR=${OUTPUT_DIR:-}
HEADERS=${HEADERS:-}

if [[ -n "$OUTPUT_DIR" ]]; then
  mkdir -p "$OUTPUT_DIR"
  JSON_EXPORT="--export-json=$OUTPUT_DIR/hyperfine.json"
  MARKDOWN_EXPORT="--export-markdown=$OUTPUT_DIR/hyperfine.md"
else
  JSON_EXPORT=""
  MARKDOWN_EXPORT=""
fi

BENCH_COMMANDS=(
  "curl --silent --show-error --output /dev/null --write-out '%{http_code}' $HEADERS $API_BASE/health"
  "curl --silent --show-error --output /dev/null --write-out '%{http_code}' $HEADERS $API_BASE/tasks"
  "curl --silent --show-error --output /dev/null --write-out '%{http_code}' $HEADERS $API_BASE/api/codex/environments"
)

echo "Running hyperfine against $API_BASE" >&2
hyperfine \
  --warmup "$WARMUP" \
  --runs "$RUNS" \
  $JSON_EXPORT \
  $MARKDOWN_EXPORT \
  "${BENCH_COMMANDS[@]}"
