#!/usr/bin/env bash
# ClickHouse restore из MinIO/S3
#
# Использование:
#   bash restore-clickhouse.sh --backup clickhouse_20240115_120000
#   bash restore-clickhouse.sh --latest          # восстановить из последнего backup
#   bash restore-clickhouse.sh --list            # список доступных backup
#   bash restore-clickhouse.sh --backup NAME --table siem.events  # только одна таблица

set -euo pipefail

CH_HOST="${CH_HOST:-localhost}"
CH_PORT="${CH_PORT:-8123}"
CH_USER="${CH_USER:-default}"
CH_PASSWORD="${CH_PASSWORD:-}"
S3_ENDPOINT="${S3_ENDPOINT:-http://minio:9000}"
S3_BUCKET="${S3_BUCKET:-siem-backups}"
S3_ACCESS_KEY="${S3_ACCESS_KEY:-minioadmin}"
S3_SECRET_KEY="${S3_SECRET_KEY:-}"
SLACK_WEBHOOK="${SLACK_WEBHOOK:-}"

BACKUP_NAME=""
TABLE=""
LIST_ONLY=false
LATEST=false

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[$(date -u '+%H:%M:%S')]${NC} $*"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
fail() { echo -e "${RED}[FAIL]${NC} $*"; }

while [[ $# -gt 0 ]]; do
  case $1 in
    --backup)  BACKUP_NAME="$2"; shift 2;;
    --table)   TABLE="$2"; shift 2;;
    --list)    LIST_ONLY=true; shift;;
    --latest)  LATEST=true; shift;;
    *) echo "Unknown: $1"; exit 1;;
  esac
done

notify_slack() {
  [[ -z "$SLACK_WEBHOOK" ]] && return 0
  curl -sf -X POST "$SLACK_WEBHOOK" \
    -H 'Content-Type: application/json' \
    -d "{\"attachments\":[{\"color\":\"$1\",\"title\":\"$2\",\"text\":\"$3\"}]}" &>/dev/null || true
}

ch_query() {
  curl -sf \
    --user "${CH_USER}:${CH_PASSWORD}" \
    "http://${CH_HOST}:${CH_PORT}/" \
    --data-binary "$1"
}

list_backups() {
  log "Available backups in s3://${S3_BUCKET}/clickhouse/:"
  if command -v mc &>/dev/null; then
    mc alias set siem-minio "$S3_ENDPOINT" "$S3_ACCESS_KEY" "$S3_SECRET_KEY" &>/dev/null || true
    mc ls "siem-minio/${S3_BUCKET}/clickhouse/" 2>/dev/null | sort -r || warn "Could not list backups"
  else
    # Через ClickHouse system.backups
    ch_query "SELECT name, start_time, end_time, status FROM system.backups ORDER BY start_time DESC FORMAT PrettyCompact" \
      2>/dev/null || warn "Could not query system.backups"
  fi
}

get_latest_backup() {
  ch_query "SELECT name FROM system.backups WHERE status='BACKUP_COMPLETE' ORDER BY start_time DESC LIMIT 1 FORMAT TabSeparated" \
    2>/dev/null | head -1 | tr -d '[:space:]'
}

main() {
  if [[ "$LIST_ONLY" == "true" ]]; then
    list_backups
    return 0
  fi

  if [[ "$LATEST" == "true" ]]; then
    BACKUP_NAME=$(get_latest_backup)
    if [[ -z "$BACKUP_NAME" ]]; then
      fail "No completed backups found"
      exit 1
    fi
    log "Latest backup: ${BACKUP_NAME}"
  fi

  if [[ -z "$BACKUP_NAME" ]]; then
    fail "Specify --backup NAME or --latest"
    echo "Usage: bash restore-clickhouse.sh --backup clickhouse_20240115_120000"
    echo "       bash restore-clickhouse.sh --latest"
    echo "       bash restore-clickhouse.sh --list"
    exit 1
  fi

  log "=== ClickHouse Restore ==="
  log "Backup: ${BACKUP_NAME}"
  log "Source: s3(${S3_ENDPOINT}/${S3_BUCKET}/clickhouse/${BACKUP_NAME})"
  [[ -n "$TABLE" ]] && log "Table: ${TABLE}"

  # Предупреждение о перезаписи данных
  warn "This will OVERWRITE existing data in the database!"
  read -rp "Continue? (yes/no): " confirm
  [[ "$confirm" != "yes" ]] && { log "Aborted"; exit 0; }

  local s3_src="s3('${S3_ENDPOINT}/${S3_BUCKET}/clickhouse/${BACKUP_NAME}', '${S3_ACCESS_KEY}', '${S3_SECRET_KEY}')"
  local restore_sql

  if [[ -n "$TABLE" ]]; then
    restore_sql="RESTORE TABLE ${TABLE} FROM ${s3_src}"
  else
    restore_sql="RESTORE DATABASE siem FROM ${s3_src}"
  fi

  log "Executing restore..."
  local start_time
  start_time=$(date +%s)

  local result
  if ! result=$(ch_query "$restore_sql"); then
    fail "Restore failed!"
    fail "Error: $result"
    notify_slack "danger" "ClickHouse Restore FAILED" \
      "Backup: ${BACKUP_NAME}\nError: $(echo "$result" | head -1)"
    exit 1
  fi

  local duration=$(( $(date +%s) - start_time ))
  ok "Restore completed from ${BACKUP_NAME} in ${duration}s"

  # Проверяем восстановленные данные
  log "Verification..."
  local row_count
  row_count=$(ch_query "SELECT count() FROM siem.events FORMAT TabSeparated" 2>/dev/null || echo "0")
  ok "siem.events: ${row_count} rows restored"

  notify_slack "good" "ClickHouse Restore SUCCESS" \
    "Backup: ${BACKUP_NAME}\nDuration: ${duration}s\nRows: ${row_count}"
}

main "$@"
