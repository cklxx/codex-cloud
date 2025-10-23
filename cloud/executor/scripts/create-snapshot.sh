#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
STATE_DIR=${CODEX_EXECUTOR_STATE_DIR:-${SCRIPT_DIR}/../state}
mkdir -p "${STATE_DIR}"

IGNITE_BIN=${IGNITE_BIN:-ignite}
BASE_IMAGE=${CODEX_EXECUTOR_BASE_IMAGE:-codex-executor-base}
BASE_TAG=${CODEX_EXECUTOR_BASE_TAG:-latest}
SNAPSHOT_NAME=${CODEX_EXECUTOR_SNAPSHOT_NAME:-codex-executor-snapshot}
SNAPSHOT_EXPORT_PATH=${CODEX_EXECUTOR_SNAPSHOT_EXPORT:-${STATE_DIR}/${SNAPSHOT_NAME}.tar}
OCI_REF="${BASE_IMAGE}:${BASE_TAG}"

if ! command -v "${IGNITE_BIN}" >/dev/null 2>&1; then
  echo "Ignite binary '${IGNITE_BIN}' not found on PATH" >&2
  exit 1
fi

# Import the OCI image so Ignite can materialise it into a Firecracker VM
if ! "${IGNITE_BIN}" image inspect "${OCI_REF}" >/dev/null 2>&1; then
  echo "Importing ${OCI_REF} into Ignite"
  "${IGNITE_BIN}" image import "${OCI_REF}" --oci --name "${BASE_IMAGE}" >/dev/null
fi

VM_NAME=${CODEX_EXECUTOR_VM_NAME:-codex-build-$(date +%s)}
SNAPSHOT_FRIENDLY_NAME="${SNAPSHOT_NAME}-$(date +%s)"

echo "Launching temporary VM ${VM_NAME} for snapshot capture"
"${IGNITE_BIN}" run "${OCI_REF}" \
  --name "${VM_NAME}" \
  --snapshot \
  --ssh \
  --cpus ${CODEX_EXECUTOR_CPUS:-2} \
  --memory ${CODEX_EXECUTOR_MEMORY:-4096} \
  --ignite-spawn >/dev/null

trap '"'"${IGNITE_BIN}"'" stop "'"${VM_NAME}"'" >/dev/null 2>&1 || true' EXIT

# Give workloads a chance to provision caches before snapshotting
PREWARM_SCRIPT=${CODEX_EXECUTOR_PREWARM_SCRIPT:-}
if [[ -n "${PREWARM_SCRIPT}" ]]; then
  echo "Executing prewarm script ${PREWARM_SCRIPT} inside VM"
  "${IGNITE_BIN}" exec "${VM_NAME}" -- bash -lc "${PREWARM_SCRIPT}"
fi

echo "Creating snapshot ${SNAPSHOT_FRIENDLY_NAME}"
"${IGNITE_BIN}" snapshot create "${VM_NAME}" --name "${SNAPSHOT_FRIENDLY_NAME}" >/dev/null

echo "Exporting snapshot to ${SNAPSHOT_EXPORT_PATH}"
"${IGNITE_BIN}" snapshot export "${SNAPSHOT_FRIENDLY_NAME}" "${SNAPSHOT_EXPORT_PATH}"

echo "Snapshot ${SNAPSHOT_FRIENDLY_NAME}" > "${STATE_DIR}/last-snapshot.txt"

"${IGNITE_BIN}" stop "${VM_NAME}" >/dev/null
"${IGNITE_BIN}" rm "${VM_NAME}" >/dev/null
trap - EXIT

echo "${SNAPSHOT_FRIENDLY_NAME}"
