#!/usr/bin/env bash
# Полное восстановление SIEM-Lite из backup
# ВНИМАНИЕ: Удаляет существующие данные!

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "[$(date '+%H:%M:%S')] $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; exit 1; }

BACKUP_NAME=""
SKIP_CONFIRM=false

while [[ $# -gt 0 ]]; do
  case $1 in
    --backup) BACKUP_NAME="$2"; shift 2;;
    --yes)    SKIP_CONFIRM=true; shift;;
    *) echo "Unknown: $1"; exit 1;;
  esac
done

if [[ -z "$BACKUP_NAME" ]]; then
  fail "Specify --backup NAME (use verify-backup.sh --list to see available backups)"
fi

if [[ "$SKIP_CONFIRM" != "true" ]]; then
  warn "WARNING: This will DESTROY existing data and restore from backup: ${BACKUP_NAME}"
  read -rp "Type 'RESTORE' to confirm: " confirm
  [[ "$confirm" != "RESTORE" ]] && { log "Aborted"; exit 0; }
fi

log "=== Starting full restore from ${BACKUP_NAME} ==="

# 1. Stop ingestion to prevent data corruption
log "Stopping ingestion pipeline..."
docker stop siem-parser vector-aggregator 2>/dev/null || warn "Could not stop containers"

# 2. ClickHouse restore
log "Restoring ClickHouse..."
bash "${SCRIPT_DIR}/restore-clickhouse.sh" --backup "${BACKUP_NAME}" --yes

# 3. Restart services
log "Restarting services..."
docker start vector-aggregator siem-parser 2>/dev/null || warn "Could not start containers"

ok "Full restore complete from backup: ${BACKUP_NAME}"
