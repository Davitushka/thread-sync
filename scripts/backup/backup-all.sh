#!/usr/bin/env bash
# Полный backup всех компонентов SIEM-Lite
# Запускается по cron: 0 2 * * * bash /opt/siem/scripts/backup/backup-all.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMESTAMP=$(date -u '+%Y%m%d_%H%M%S')
LOG_FILE="/var/log/siem/backup-${TIMESTAMP}.log"

mkdir -p /var/log/siem 2>/dev/null || mkdir -p /tmp/siem-logs
LOG_FILE="${LOG_FILE:-/tmp/siem-logs/backup-${TIMESTAMP}.log}"

SLACK_WEBHOOK="${SLACK_WEBHOOK:-}"
INCREMENTAL="${INCREMENTAL:-false}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[$(date -u '+%H:%M:%S')]${NC} $*" | tee -a "$LOG_FILE"; }
ok()   { echo -e "${GREEN}[OK]${NC} $*" | tee -a "$LOG_FILE"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*" | tee -a "$LOG_FILE"; }
fail() { echo -e "${RED}[FAIL]${NC} $*" | tee -a "$LOG_FILE"; }

notify_slack() {
  [[ -z "$SLACK_WEBHOOK" ]] && return 0
  curl -sf -X POST "$SLACK_WEBHOOK" \
    -H 'Content-Type: application/json' \
    -d "{\"text\":\"$*\"}" &>/dev/null || true
}

main() {
  log "=== SIEM-Lite Full Backup ${TIMESTAMP} ==="
  local errors=0
  local start_time
  start_time=$(date +%s)

  # 1. ClickHouse
  log "--- ClickHouse backup ---"
  local ch_args=()
  [[ "$INCREMENTAL" == "true" ]] && ch_args+=(--incremental)
  if bash "${SCRIPT_DIR}/backup-clickhouse.sh" "${ch_args[@]}" 2>&1 | tee -a "$LOG_FILE"; then
    ok "ClickHouse backup: SUCCESS"
  else
    fail "ClickHouse backup: FAILED"
    ((errors++)) || true
  fi

  # 2. Prometheus (метаданные + rules)
  log "--- Prometheus backup ---"
  if bash "${SCRIPT_DIR}/backup-prometheus.sh" 2>&1 | tee -a "$LOG_FILE"; then
    ok "Prometheus backup: SUCCESS"
  else
    warn "Prometheus backup: FAILED (non-fatal for metrics)"
    # Не увеличиваем errors — prometheus метрики пересчитываются
  fi

  # 3. Grafana dashboards (копируем JSON файлы)
  log "--- Grafana dashboards backup ---"
  local grafana_backup_dir="/tmp/siem-grafana-backup-${TIMESTAMP}"
  mkdir -p "$grafana_backup_dir"
  if docker cp siem-grafana:/var/lib/grafana/dashboards "$grafana_backup_dir/" 2>/dev/null; then
    ok "Grafana dashboards backed up to ${grafana_backup_dir}"
  else
    warn "Grafana docker cp failed (container may not be running)"
  fi

  # 4. Alertmanager конфиги
  log "--- Config backup ---"
  local config_archive="/tmp/siem-configs-${TIMESTAMP}.tar.gz"
  if tar -czf "$config_archive" \
    alerting/ vector/ sigma-rules/ \
    grafana/provisioning/ \
    --exclude='*.mmdb' \
    --exclude='secrets/' \
    2>/dev/null; then
    ok "Config backup: ${config_archive}"
    # Загрузить в S3/MinIO
    if command -v mc &>/dev/null; then
      mc cp "$config_archive" "siem-minio/${S3_BUCKET:-siem-backups}/configs/" 2>/dev/null || true
    fi
  else
    warn "Config backup failed (non-fatal)"
  fi

  local duration=$(( $(date +%s) - start_time ))
  log "=== Backup complete in ${duration}s, errors: ${errors} ==="

  if [[ $errors -eq 0 ]]; then
    notify_slack ":white_check_mark: *SIEM-Lite Backup SUCCESS* (${TIMESTAMP}) — ${duration}s"
    return 0
  else
    notify_slack ":x: *SIEM-Lite Backup PARTIAL FAILURE* (${TIMESTAMP}) — ${errors} component(s) failed"
    return 1
  fi
}

main "$@"
