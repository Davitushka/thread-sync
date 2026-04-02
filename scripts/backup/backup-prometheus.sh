#!/usr/bin/env bash
# Prometheus backup через API snapshot
#
# Создаёт снимок TSDB через /api/v1/admin/tsdb/snapshot и копирует в S3.
# Prometheus должен быть запущен с --web.enable-admin-api

set -euo pipefail

PROM_URL="${PROM_URL:-http://localhost:9090}"
S3_ENDPOINT="${S3_ENDPOINT:-http://minio:9000}"
S3_BUCKET="${S3_BUCKET:-siem-backups}"
S3_ACCESS_KEY="${S3_ACCESS_KEY:-minioadmin}"
S3_SECRET_KEY="${S3_SECRET_KEY:-}"
TIMESTAMP=$(date -u '+%Y%m%d_%H%M%S')

log()  { echo "[$(date -u '+%H:%M:%S')] $*"; }
ok()   { echo "[OK] $*"; }
fail() { echo "[FAIL] $*"; exit 1; }

main() {
  log "Creating Prometheus TSDB snapshot..."

  local snap_result
  snap_result=$(curl -sf -X POST "${PROM_URL}/api/v1/admin/tsdb/snapshot" 2>/dev/null \
    || fail "Prometheus snapshot API failed. Check --web.enable-admin-api flag")

  local snap_name
  snap_name=$(echo "$snap_result" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['name'])" 2>/dev/null \
    || fail "Could not parse snapshot name from: $snap_result")

  ok "Snapshot created: ${snap_name}"

  # Prometheus сохраняет снимки в data/snapshots/
  local prom_data_dir
  if docker ps --filter "name=prometheus" --format "{{.Names}}" 2>/dev/null | grep -q prometheus; then
    local snap_dir="/tmp/prom-snap-${TIMESTAMP}"
    docker cp "siem-prometheus:/prometheus/snapshots/${snap_name}" "$snap_dir" 2>/dev/null \
      || fail "Could not copy snapshot from container"

    local archive="/tmp/prom-backup-${TIMESTAMP}.tar.gz"
    tar -czf "$archive" -C "$(dirname "$snap_dir")" "$(basename "$snap_dir")"
    ok "Archive: ${archive} ($(du -sh "$archive" | cut -f1))"

    # Загрузить в S3
    if command -v mc &>/dev/null; then
      mc alias set siem-minio "$S3_ENDPOINT" "$S3_ACCESS_KEY" "$S3_SECRET_KEY" &>/dev/null || true
      mc cp "$archive" "siem-minio/${S3_BUCKET}/prometheus/" 2>/dev/null \
        && ok "Uploaded to S3" \
        || log "S3 upload skipped (mc failed)"
    fi

    rm -rf "$snap_dir" "$archive"
  else
    log "Prometheus not running in Docker, skipping file copy"
    log "Snapshot available at: data/snapshots/${snap_name}"
  fi

  ok "Prometheus backup complete"
}

main "$@"
