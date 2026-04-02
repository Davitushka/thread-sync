#!/usr/bin/env bash
# Верификация backup — проверяет целостность и возможность восстановления
#
# Использование:
#   bash verify-backup.sh                    # верифицировать последний backup
#   bash verify-backup.sh --backup NAME      # конкретный backup
#   bash verify-backup.sh --restore-test     # полный тест restore в temp БД

set -euo pipefail

CH_HOST="${CH_HOST:-localhost}"
CH_PORT="${CH_PORT:-8123}"
CH_USER="${CH_USER:-default}"
CH_PASSWORD="${CH_PASSWORD:-}"
S3_ENDPOINT="${S3_ENDPOINT:-http://minio:9000}"
S3_BUCKET="${S3_BUCKET:-siem-backups}"
S3_ACCESS_KEY="${S3_ACCESS_KEY:-minioadmin}"
S3_SECRET_KEY="${S3_SECRET_KEY:-}"

BACKUP_NAME=""
RESTORE_TEST=false

GREEN='\033[0;32m'; RED='\033[0;31m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[$(date -u '+%H:%M:%S')]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; }

while [[ $# -gt 0 ]]; do
  case $1 in
    --backup)       BACKUP_NAME="$2"; shift 2;;
    --restore-test) RESTORE_TEST=true; shift;;
    *) echo "Unknown: $1"; exit 1;;
  esac
done

ch_query() {
  curl -sf --user "${CH_USER}:${CH_PASSWORD}" \
    "http://${CH_HOST}:${CH_PORT}/" --data-binary "$1"
}

main() {
  log "=== Backup Verification ==="
  local passed=0
  local failed=0

  # 1. Список backup в ClickHouse
  log "Checking system.backups..."
  local backups
  backups=$(ch_query "SELECT name, status, start_time, end_time FROM system.backups ORDER BY start_time DESC LIMIT 5 FORMAT PrettyCompact" 2>/dev/null || echo "ERROR")
  if [[ "$backups" == *"ERROR"* ]]; then
    fail "Cannot query system.backups"
    ((failed++)) || true
  else
    ok "system.backups accessible"
    echo "$backups"
    ((passed++)) || true
  fi

  # 2. Проверяем S3 доступность
  log "Checking S3 connectivity..."
  if curl -sf "${S3_ENDPOINT}/minio/health/live" &>/dev/null; then
    ok "MinIO/S3 is healthy"
    ((passed++)) || true
  else
    fail "MinIO/S3 is not accessible at ${S3_ENDPOINT}"
    ((failed++)) || true
  fi

  # 3. Проверяем наличие backup файлов в S3
  if command -v mc &>/dev/null; then
    log "Checking backup files in S3..."
    mc alias set siem-minio "$S3_ENDPOINT" "$S3_ACCESS_KEY" "$S3_SECRET_KEY" &>/dev/null || true
    local file_count
    file_count=$(mc ls "siem-minio/${S3_BUCKET}/clickhouse/" 2>/dev/null | wc -l || echo 0)
    if [[ "$file_count" -gt 0 ]]; then
      ok "Found ${file_count} backup(s) in S3"
      ((passed++)) || true
    else
      fail "No backups found in s3://${S3_BUCKET}/clickhouse/"
      ((failed++)) || true
    fi
  fi

  # 4. Restore test в временную БД
  if [[ "$RESTORE_TEST" == "true" && -n "$BACKUP_NAME" ]]; then
    log "Running restore test to siem_verify database..."
    local s3_src="s3('${S3_ENDPOINT}/${S3_BUCKET}/clickhouse/${BACKUP_NAME}', '${S3_ACCESS_KEY}', '${S3_SECRET_KEY}')"

    ch_query "CREATE DATABASE IF NOT EXISTS siem_verify" 2>/dev/null || true
    local restore_result
    if restore_result=$(ch_query "RESTORE DATABASE siem FROM ${s3_src} INTO siem_verify SETTINGS allow_non_empty_tables=true" 2>&1); then
      ok "Restore test: SUCCESS"
      local row_count
      row_count=$(ch_query "SELECT count() FROM siem_verify.events FORMAT TabSeparated" 2>/dev/null || echo "0")
      ok "Verified rows: ${row_count}"
      # Cleanup
      ch_query "DROP DATABASE IF EXISTS siem_verify" 2>/dev/null || true
      ((passed++)) || true
    else
      fail "Restore test FAILED: $restore_result"
      ((failed++)) || true
    fi
  fi

  log "=== Verification: ${passed} passed, ${failed} failed ==="
  [[ $failed -eq 0 ]]
}

main "$@"
