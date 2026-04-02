#!/usr/bin/env bash
# ClickHouse backup в MinIO/S3 через встроенный BACKUP TO S3
#
# Использование:
#   bash backup-clickhouse.sh              # полный backup
#   bash backup-clickhouse.sh --incremental # инкрементальный (diff от последнего)
#   bash backup-clickhouse.sh --table siem.events  # backup одной таблицы
#   bash backup-clickhouse.sh --dry-run    # показать запрос без выполнения
#
# Переменные окружения:
#   CH_HOST          — ClickHouse host (default: localhost)
#   CH_PORT          — ClickHouse HTTP port (default: 8123)
#   CH_USER          — ClickHouse user (default: default)
#   CH_PASSWORD      — ClickHouse password
#   S3_ENDPOINT      — MinIO/S3 endpoint (default: http://minio:9000)
#   S3_BUCKET        — S3 bucket name (default: siem-backups)
#   S3_ACCESS_KEY    — S3 access key
#   S3_SECRET_KEY    — S3 secret key
#   SLACK_WEBHOOK    — Slack webhook URL для уведомлений (опционально)
#   BACKUP_RETENTION — Дней хранения (default: 30)

set -euo pipefail

# ── Конфигурация ──────────────────────────────────────────────────────────────
CH_HOST="${CH_HOST:-localhost}"
CH_PORT="${CH_PORT:-8123}"
CH_USER="${CH_USER:-default}"
CH_PASSWORD="${CH_PASSWORD:-}"
S3_ENDPOINT="${S3_ENDPOINT:-http://minio:9000}"
S3_BUCKET="${S3_BUCKET:-siem-backups}"
S3_ACCESS_KEY="${S3_ACCESS_KEY:-minioadmin}"
S3_SECRET_KEY="${S3_SECRET_KEY:-}"
SLACK_WEBHOOK="${SLACK_WEBHOOK:-}"
BACKUP_RETENTION="${BACKUP_RETENTION:-30}"

INCREMENTAL=false
TABLE=""
DRY_RUN=false
TIMESTAMP=$(date -u '+%Y%m%d_%H%M%S')
BACKUP_NAME="clickhouse_${TIMESTAMP}"
LOG_FILE="/tmp/siem-backup-${TIMESTAMP}.log"

# ── Цветной вывод ─────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[$(date -u '+%H:%M:%S')]${NC} $*" | tee -a "$LOG_FILE"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*" | tee -a "$LOG_FILE"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*" | tee -a "$LOG_FILE"; }
fail() { echo -e "${RED}[FAIL]${NC} $*" | tee -a "$LOG_FILE"; }

# ── Аргументы ─────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case $1 in
    --incremental) INCREMENTAL=true; shift;;
    --table)       TABLE="$2"; shift 2;;
    --dry-run)     DRY_RUN=true; shift;;
    --name)        BACKUP_NAME="$2"; shift 2;;
    *) echo "Unknown: $1"; exit 1;;
  esac
done

# ── Slack уведомление ──────────────────────────────────────────────────────────
notify_slack() {
  local color="$1"
  local title="$2"
  local body="$3"
  [[ -z "$SLACK_WEBHOOK" ]] && return 0

  curl -sf -X POST "$SLACK_WEBHOOK" \
    -H 'Content-Type: application/json' \
    -d "{\"attachments\":[{\"color\":\"${color}\",\"title\":\"${title}\",\"text\":\"${body}\"}]}" \
    &>/dev/null || warn "Slack notification failed"
}

# ── ClickHouse запрос ──────────────────────────────────────────────────────────
ch_query() {
  local sql="$1"
  if [[ "$DRY_RUN" == "true" ]]; then
    log "DRY RUN: $sql"
    return 0
  fi
  curl -sf \
    --user "${CH_USER}:${CH_PASSWORD}" \
    "http://${CH_HOST}:${CH_PORT}/" \
    --data-binary "$sql" \
    2>>"$LOG_FILE"
}

# ── Найти последний backup (для инкрементального) ─────────────────────────────
get_last_backup_name() {
  # Список backup через ClickHouse system.backups (доступен после BACKUP)
  local result
  result=$(curl -sf \
    --user "${CH_USER}:${CH_PASSWORD}" \
    "http://${CH_HOST}:${CH_PORT}/?query=SELECT+name+FROM+system.backups+ORDER+BY+start_time+DESC+LIMIT+1+FORMAT+TabSeparated" \
    2>/dev/null || echo "")
  echo "$result"
}

# ── Ротация старых backup ──────────────────────────────────────────────────────
rotate_old_backups() {
  local cutoff_date
  cutoff_date=$(date -u -d "${BACKUP_RETENTION} days ago" '+%Y%m%d' 2>/dev/null \
    || date -u -v-"${BACKUP_RETENTION}"d '+%Y%m%d' 2>/dev/null \
    || echo "19700101")

  log "Rotating backups older than ${BACKUP_RETENTION} days (before ${cutoff_date})..."

  # Используем mc (MinIO client) если доступен
  if command -v mc &>/dev/null; then
    mc alias set siem-minio "$S3_ENDPOINT" "$S3_ACCESS_KEY" "$S3_SECRET_KEY" &>/dev/null || true
    mc find "siem-minio/${S3_BUCKET}/clickhouse/" \
      --older-than "${BACKUP_RETENTION}d" \
      --exec "mc rm {}" 2>>"$LOG_FILE" \
      | tee -a "$LOG_FILE" \
      || warn "mc rotation failed (non-fatal)"
  else
    warn "mc not found, skipping S3 rotation (install: https://min.io/docs/minio/linux/reference/minio-mc.html)"
  fi
}

# ── Основной backup ────────────────────────────────────────────────────────────
main() {
  log "=== ClickHouse Backup ==="
  log "Host: ${CH_HOST}:${CH_PORT}"
  log "Destination: s3(${S3_ENDPOINT}/${S3_BUCKET}/clickhouse/${BACKUP_NAME})"
  log "Incremental: ${INCREMENTAL}"
  [[ -n "$TABLE" ]] && log "Table: ${TABLE}"

  # Проверяем соединение с ClickHouse
  if [[ "$DRY_RUN" == "false" ]]; then
    if ! curl -sf --user "${CH_USER}:${CH_PASSWORD}" "http://${CH_HOST}:${CH_PORT}/ping" &>/dev/null; then
      fail "Cannot connect to ClickHouse at ${CH_HOST}:${CH_PORT}"
      notify_slack "danger" "ClickHouse Backup FAILED" \
        "Cannot connect to ClickHouse\nHost: ${CH_HOST}:${CH_PORT}\nTime: $(date -u)"
      exit 1
    fi
    ok "ClickHouse connection OK"
  fi

  # Формируем SQL запрос
  local s3_dest="s3('${S3_ENDPOINT}/${S3_BUCKET}/clickhouse/${BACKUP_NAME}', '${S3_ACCESS_KEY}', '${S3_SECRET_KEY}')"
  local backup_sql

  if [[ -n "$TABLE" ]]; then
    backup_sql="BACKUP TABLE ${TABLE} TO ${s3_dest}"
  elif [[ "$INCREMENTAL" == "true" ]]; then
    local last_backup
    last_backup=$(get_last_backup_name)
    if [[ -n "$last_backup" ]]; then
      local base_dest="s3('${S3_ENDPOINT}/${S3_BUCKET}/clickhouse/${last_backup}', '${S3_ACCESS_KEY}', '${S3_SECRET_KEY}')"
      backup_sql="BACKUP DATABASE siem TO ${s3_dest} SETTINGS base_backup = ${base_dest}"
      log "Incremental from: ${last_backup}"
    else
      warn "No previous backup found, falling back to full backup"
      backup_sql="BACKUP DATABASE siem TO ${s3_dest}"
    fi
  else
    backup_sql="BACKUP DATABASE siem TO ${s3_dest}"
  fi

  log "Executing: ${backup_sql:0:120}..."

  local start_time
  start_time=$(date +%s)

  local result
  if ! result=$(ch_query "$backup_sql"); then
    fail "Backup failed!"
    log "Error: $result"
    notify_slack "danger" "ClickHouse Backup FAILED" \
      "Database: siem\nError: $(echo "$result" | head -1)\nTime: $(date -u)"
    exit 1
  fi

  local end_time
  end_time=$(date +%s)
  local duration=$((end_time - start_time))

  ok "Backup completed: ${BACKUP_NAME} in ${duration}s"
  log "Result: $result"

  # Ротация
  rotate_old_backups

  # Сохраняем метаданные backup
  local meta_file="/tmp/siem-backup-meta-${BACKUP_NAME}.json"
  cat > "$meta_file" <<EOF
{
  "backup_name": "${BACKUP_NAME}",
  "timestamp": "$(date -u '+%Y-%m-%dT%H:%M:%SZ')",
  "type": "$([ "$INCREMENTAL" == "true" ] && echo "incremental" || echo "full")",
  "table": "${TABLE}",
  "duration_sec": ${duration},
  "s3_endpoint": "${S3_ENDPOINT}",
  "s3_bucket": "${S3_BUCKET}"
}
EOF
  log "Metadata: $meta_file"

  notify_slack "good" "ClickHouse Backup SUCCESS" \
    "Backup: ${BACKUP_NAME}\nDuration: ${duration}s\nType: $([ "$INCREMENTAL" == "true" ] && echo "incremental" || echo "full")"

  log "=== Backup complete ==="
}

main "$@"
