#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
POOL_SIZE=${CODEX_EXECUTOR_POOL_SIZE:-3}
PREFIX=${CODEX_EXECUTOR_SNAPSHOT_PREFIX:-codex-executor-pool}

if ! [[ "${POOL_SIZE}" =~ ^[0-9]+$ ]]; then
  echo "POOL_SIZE must be an integer (got '${POOL_SIZE}')" >&2
  exit 1
fi

for ((i = 1; i <= POOL_SIZE; i++)); do
  SNAPSHOT_NAME="${PREFIX}-${i}"
  echo "Refreshing snapshot ${SNAPSHOT_NAME} (${i}/${POOL_SIZE})"
  CODEX_EXECUTOR_SNAPSHOT_NAME="${SNAPSHOT_NAME}" \
    CODEX_EXECUTOR_STATE_DIR=${CODEX_EXECUTOR_STATE_DIR:-${SCRIPT_DIR}/../state} \
    "${SCRIPT_DIR}/create-snapshot.sh"
done
