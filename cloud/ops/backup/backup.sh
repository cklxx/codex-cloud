#!/usr/bin/env bash
set -euo pipefail

BACKUP_ROOT=${BACKUP_ROOT:-/var/backups/codex}
TIMESTAMP=$(date -u +"%Y%m%dT%H%M%SZ")
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"
SQLITE_DB=${SQLITE_DB:-/var/lib/codex/db/codex.db}
ARTIFACT_DIR=${ARTIFACT_DIR:-/var/lib/codex/artifacts}
RETENTION_DAYS=${RETENTION_DAYS:-14}

mkdir -p "${BACKUP_DIR}"

log() {
  echo "[$(date -u +"%Y-%m-%dT%H:%M:%SZ")] $*"
}

log "Creating backup directory ${BACKUP_DIR}"

if command -v sqlite3 >/dev/null 2>&1; then
  log "Dumping SQLite database"
  sqlite3 "${SQLITE_DB}" ".backup '${BACKUP_DIR}/codex.db'"
else
  log "sqlite3 not available; copying database file"
  cp "${SQLITE_DB}" "${BACKUP_DIR}/codex.db"
fi

log "Archiving artifacts from ${ARTIFACT_DIR}"
tar -C "${ARTIFACT_DIR}" -czf "${BACKUP_DIR}/artifacts.tar.gz" .

log "Generating manifest"
cat > "${BACKUP_DIR}/manifest.json" <<MANIFEST
{
  "created_at": "${TIMESTAMP}",
  "sqlite_source": "${SQLITE_DB}",
  "artifact_source": "${ARTIFACT_DIR}"
}
MANIFEST

log "Pruning backups older than ${RETENTION_DAYS} days"
find "${BACKUP_ROOT}" -mindepth 1 -maxdepth 1 -type d -mtime +"${RETENTION_DAYS}" -exec rm -rf {} +

log "Backup complete: ${BACKUP_DIR}"
