#!/usr/bin/env bash
set -euo pipefail

BACKUP_ROOT=${BACKUP_ROOT:-/var/backups/codex}
RESTORE_POINT=${1:-latest}
SQLITE_DB=${SQLITE_DB:-/var/lib/codex/db/codex.db}
ARTIFACT_DIR=${ARTIFACT_DIR:-/var/lib/codex/artifacts}

if [[ "${RESTORE_POINT}" == "latest" ]]; then
  RESTORE_DIR=$(ls -1 "${BACKUP_ROOT}" | sort | tail -n1)
else
  RESTORE_DIR="${RESTORE_POINT}"
fi

if [[ -z "${RESTORE_DIR}" ]]; then
  echo "No backup found in ${BACKUP_ROOT}" >&2
  exit 1
fi

RESTORE_PATH="${BACKUP_ROOT}/${RESTORE_DIR}"

if [[ ! -d "${RESTORE_PATH}" ]]; then
  echo "Backup ${RESTORE_DIR} not found under ${BACKUP_ROOT}" >&2
  exit 1
fi

echo "Restoring Codex data from ${RESTORE_PATH}" >&2

if systemctl is-active --quiet codex-compose.service; then
  echo "Stopping Codex services" >&2
  systemctl stop codex-compose.service
fi

mkdir -p "$(dirname "${SQLITE_DB}")"
mkdir -p "${ARTIFACT_DIR}"

cp "${RESTORE_PATH}/codex.db" "${SQLITE_DB}"
tar -C "${ARTIFACT_DIR}" -xzf "${RESTORE_PATH}/artifacts.tar.gz"

chown -R codex:codex "$(dirname "${SQLITE_DB}")" "${ARTIFACT_DIR}" || true

if [[ -f "${RESTORE_PATH}/manifest.json" ]]; then
  echo "Restored snapshot metadata:"
  cat "${RESTORE_PATH}/manifest.json"
fi

echo "Starting Codex services" >&2
systemctl start codex-compose.service

echo "Restore complete" >&2
