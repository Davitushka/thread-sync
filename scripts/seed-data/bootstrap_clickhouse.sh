#!/usr/bin/env bash
# Загрузка демо-данных в ClickHouse: события, алерты, threat_intel (SOC Workbench).
# Использование:
#   ./scripts/seed-data/bootstrap_clickhouse.sh
#   CLICKHOUSE_PASSWORD=secret ./scripts/seed-data/bootstrap_clickhouse.sh
#
# Контейнер по умолчанию: siem-clickhouse (docker compose из deploy/docker).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SEED_SQL="${ROOT}/scripts/seed-data/seed_test_events.sql"
CONTAINER="${CLICKHOUSE_CONTAINER:-siem-clickhouse}"
USER="${CLICKHOUSE_USER:-siem}"
PASSWORD="${CLICKHOUSE_PASSWORD:-ClickHousePass123!}"

if [[ ! -f "$SEED_SQL" ]]; then
  echo "error: seed SQL not found: $SEED_SQL" >&2
  exit 1
fi

if ! docker ps --format '{{.Names}}' | grep -qx "$CONTAINER"; then
  echo "error: container '$CONTAINER' is not running (start stack: docker compose -f deploy/docker/docker-compose.yml up -d)" >&2
  exit 1
fi

echo "Applying $SEED_SQL -> $CONTAINER (user=$USER)..."
docker exec -i "$CONTAINER" clickhouse-client \
  --user "$USER" \
  --password "$PASSWORD" \
  --multiquery < "$SEED_SQL"

echo "bootstrap_clickhouse: OK"
