# SIEM-Lite Runbook & Operations

## 1. Checklist первого запуска

### Pre-flight проверки

```bash
# 1. Создать секреты
cd deploy/docker/secrets
echo -n "your-smtp-password" > smtp_password.txt
echo -n "https://hooks.slack.com/services/T00/B00/XXXX" > slack_webhook_url.txt
echo -n "your-pagerduty-key" > pagerduty_key.txt
echo -n "ClickHousePass123!" > clickhouse_password.txt
echo -n "MinIOSecret456!" > minio_secret_key.txt
chmod 600 *.txt

# 2. Создать директорию для GeoIP баз
docker volume create siem-lite_geoip-data

# Скачать GeoLite2 (требует регистрацию на maxmind.com)
# https://dev.maxmind.com/geoip/geolite2-free-geolocation-data
# После скачивания:
docker run --rm -v siem-lite_geoip-data:/target \
  alpine sh -c "cp /host/GeoLite2-City.mmdb /target/ && cp /host/GeoLite2-ASN.mmdb /target/"

# 3. Сгенерировать TLS сертификаты для Vector mTLS
cd scripts
chmod +x generate-certs.sh && ./generate-certs.sh

# 4. Проверить docker compose конфигурацию
docker compose -f deploy/docker/docker-compose.yml config --quiet && echo "Config OK"
```

### Запуск

```bash
# Старт в правильном порядке
docker compose -f deploy/docker/docker-compose.yml up -d redpanda
sleep 30
docker compose -f deploy/docker/docker-compose.yml up -d redpanda-init
sleep 10
docker compose -f deploy/docker/docker-compose.yml up -d clickhouse
sleep 30
docker compose -f deploy/docker/docker-compose.yml up -d --build siem-parser vector-aggregator
docker compose -f deploy/docker/docker-compose.yml up -d prometheus alertmanager loki grafana minio

# Проверка статуса всех сервисов
docker compose -f deploy/docker/docker-compose.yml ps
```

### Валидация пайплайнов

```bash
# 1. Проверить что Vector принимает события
curl -X POST http://localhost:9000/ingest \
  -H "Content-Type: application/json" \
  -d '{"Level":"Info","Message":"SIEM startup test","Timestamp":"2024-01-15T10:00:00Z"}'

# 2. Проверить что siem-parser работает
curl -s http://localhost:7000/health | jq .
# Ожидаемый ответ: {"status":"healthy","version":"0.1.0"}

# 3. Проверить что события попадают в Redpanda
docker exec siem-redpanda rpk topic consume siem.events \
  --brokers=localhost:9092 \
  --num=1 \
  --format=json

# 4. Проверить что ClickHouse принимает данные
docker exec siem-clickhouse clickhouse-client \
  --user=siem --password=ClickHousePass123! \
  --query="SELECT count() FROM siem.events WHERE timestamp > now() - INTERVAL 5 MINUTE"

# 5. Проверить метрики siem-parser
curl -s http://localhost:9100/metrics | grep siem_parser_events_parsed_total

# 6. Открыть Grafana
# http://localhost:3000 (admin/ClickHousePass123!)
```

### Тестирование правил детекции (detection-engine-rs)

Правила реализованы в `detection-engine-rs`; каталог `sigma-rules/` описывает ту же логику в формате Sigma.

```bash
# Симулировать brute-force атаку (10 запросов за 2 минуты с одного IP)
for i in $(seq 1 12); do
  curl -s -X POST http://localhost:9000/ingest \
    -H "Content-Type: application/json" \
    -d "{
      \"Level\": \"Warning\",
      \"Message\": \"HTTP POST /api/auth/login responded 401\",
      \"Timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",
      \"Properties\": {
        \"ClientIp\": \"203.0.113.99\",
        \"StatusCode\": 401,
        \"RequestPath\": \"/api/auth/login\",
        \"RequestMethod\": \"POST\"
      }
    }"
  sleep 5
done

# Проверить что алерт появился в Alertmanager
curl -s http://localhost:9093/api/v2/alerts | jq '.[] | select(.labels.rule_id == "brute_force_api")'

# Проверить события в ClickHouse
docker exec siem-clickhouse clickhouse-client \
  --user=siem --password=ClickHousePass123! \
  --query="
    SELECT source_ip, count() as cnt, groupArray(status_code) as codes
    FROM siem.events
    WHERE timestamp > now() - INTERVAL 5 MINUTE
      AND source_ip IS NOT NULL
    GROUP BY source_ip
    ORDER BY cnt DESC
    LIMIT 10
    FORMAT PrettyCompact
  "
```

## 2. Добавление нового источника логов (< 15 минут)

### Пример: добавить nginx access logs

**Шаг 1** (2 мин): Добавить source в Vector Agent config:

```yaml
# vector/agent.yaml — добавить в sources:
nginx_access:
  type: file
  include:
    - "/var/log/nginx/access.log"
  read_from: beginning
  multiline:
    mode: halt_before
    start_pattern: '^\d{1,3}\.'
    condition_pattern: '^\d{1,3}\.'
    timeout_ms: 1000
```

**Шаг 2** (5 мин): Добавить VRL-парсер в Vector Aggregator:

```yaml
# vector/aggregator.yaml — добавить в transforms:
parse_nginx:
  type: remap
  inputs:
    - route_events.default  # или отдельный route
  source: |
    # Nginx combined log format
    parsed, err = parse_nginx_log(.message, "combined")
    if err == null {
      .source_type = "nginx"
      .event_type = "network"
      .source_ip = parsed.client
      .http_method = parsed.method
      .url_path = parsed.path  # query string уже отсутствует в parse_nginx_log
      .status_code = to_int(parsed.status) ?? null
      .duration_ms = to_float(parsed.request_time_seconds) ?? null * 1000
      .user_id = if parsed.user == "-" { null } else { parsed.user }
      .severity = if .status_code != null && .status_code >= 500 { "error" }
                  else if .status_code != null && .status_code >= 400 { "warning" }
                  else { "info" }
    }
```

**Шаг 3** (3 мин): Добавить маршрут в route_events:

```yaml
# В transforms.route_events.route добавить:
nginx: '.source_hint == "docker" && exists(.container_name) && match(string!(.container_name), r"nginx")'
```

**Шаг 4** (2 мин): Применить конфигурацию:

```bash
# Vector поддерживает hot reload без рестарта
docker exec siem-vector-aggregator \
  wget --spider http://localhost:8686/health

# Отправить SIGHUP для перезагрузки конфига
docker kill --signal=HUP siem-vector-aggregator

# Проверить что новый источник активен
curl -s http://localhost:8686/components | jq '.[] | select(.component_id == "parse_nginx")'
```

**Шаг 5** (1 мин): Валидация:

```bash
# Генерируем тестовый nginx запрос
curl -s http://your-app/health > /dev/null

# Проверяем в ClickHouse
docker exec siem-clickhouse clickhouse-client \
  --query="SELECT count() FROM siem.events WHERE source_type='nginx' AND timestamp > now() - INTERVAL 2 MINUTE"
```

## 3. Мониторинг самой SIEM

### Ключевые метрики (Grafana дашборд "SIEM Health")

| Метрика | Нормальное значение | Порог алерта |
|---------|---------------------|--------------|
| `siem_parser_parse_duration_seconds{quantile="0.99"}` | <2ms | >5ms (3 мин) |
| `rate(siem_parser_events_parsed_total{status="error"}[5m]) / rate(...[5m])` | <0.1% | >1% (3 мин) |
| `kafka_consumer_group_lag{topic="siem.events"}` | <1000 | >100000 (5 мин) |
| `rate(siem_parser_events_parsed_total[1m])` | ~167 ev/s @ 10k EPS | 0 (2 мин) |
| ClickHouse disk free (hot) | >30% | <15% |

### Проверка здоровья компонентов

```bash
# Один скрипт для проверки всех компонентов
#!/bin/bash
set -e

check_http() {
  local name=$1 url=$2
  if curl -sf "$url" > /dev/null; then
    echo "✓ $name: healthy"
  else
    echo "✗ $name: UNHEALTHY ($url)"
    exit 1
  fi
}

check_http "siem-parser"    "http://localhost:7000/health"
check_http "vector-agg"     "http://localhost:8686/health"
check_http "prometheus"     "http://localhost:9090/-/healthy"
check_http "alertmanager"   "http://localhost:9093/-/healthy"
check_http "grafana"        "http://localhost:3000/api/health"
check_http "loki"           "http://localhost:3100/ready"

# Redpanda
docker exec siem-redpanda rpk cluster info --brokers=localhost:9092 > /dev/null && \
  echo "✓ redpanda: healthy" || echo "✗ redpanda: UNHEALTHY"

# ClickHouse
docker exec siem-clickhouse clickhouse-client \
  --query="SELECT 'ok'" > /dev/null && \
  echo "✓ clickhouse: healthy" || echo "✗ clickhouse: UNHEALTHY"

echo "All checks passed"
```

## 4. Backup / Restore

### Backup стратегия

```bash
# ── ClickHouse backup (инкрементальный через BACKUP TABLE) ──
docker exec siem-clickhouse clickhouse-client --query="
  BACKUP TABLE siem.events TO S3('http://minio:9000/siem-backups/clickhouse/$(date +%Y%m%d)', 'siemadmin', 'MinIOSecret456!')
  SETTINGS base_backup = S3('http://minio:9000/siem-backups/clickhouse/base', 'siemadmin', 'MinIOSecret456!')
"

# ── Prometheus TSDB backup ──
curl -X POST http://localhost:9090/api/v1/admin/tsdb/snapshot
# Snapshot создаётся в /prometheus/snapshots/

# ── Redpanda — используем consumer offset (replay из S3 при необходимости) ──
# Основные данные хранятся в ClickHouse; Redpanda — транзитный слой

# ── Автоматический cron ──
# 0 2 * * * /opt/siem/scripts/backup.sh >> /var/log/siem-backup.log 2>&1
```

### Restore ClickHouse

```bash
# Остановить ingestion (чтобы избежать конфликтов)
docker stop siem-vector-aggregator siem-parser

# Восстановить из S3 backup
docker exec siem-clickhouse clickhouse-client --query="
  RESTORE TABLE siem.events FROM S3('http://minio:9000/siem-backups/clickhouse/20240115', 'siemadmin', 'MinIOSecret456!')
"

# Возобновить ingestion
docker start siem-vector-aggregator siem-parser
```

## 5. Процедуры при инцидентах

### Ingestion остановлен (алерт SIEMIngestionStopped)

```bash
# 1. Проверить агента
docker logs siem-vector-aggregator --tail=50

# 2. Проверить Redpanda
docker exec siem-redpanda rpk topic list --brokers=localhost:9092

# 3. Проверить siem-parser
curl -s http://localhost:7000/health
docker logs siem-parser --tail=50

# 4. При переполнении disk buffer Vector:
docker exec siem-vector-aggregator du -sh /var/lib/vector/

# 5. Принудительный рестарт пайплайна
docker restart siem-vector-aggregator siem-parser
```

### ClickHouse disk full

```bash
# Немедленно: удалить старые партиции вручную
docker exec siem-clickhouse clickhouse-client --query="
  ALTER TABLE siem.events DROP PARTITION '$(date -d '8 days ago' +%Y%m%d)'
"

# Запустить TTL принудительно
docker exec siem-clickhouse clickhouse-client --query="
  ALTER TABLE siem.events MATERIALIZE TTL
"

# Проверить размер партиций
docker exec siem-clickhouse clickhouse-client --query="
  SELECT partition, sum(bytes_on_disk) as bytes, formatReadableSize(bytes) as size
  FROM system.parts
  WHERE table = 'events' AND active
  GROUP BY partition
  ORDER BY partition DESC
  FORMAT PrettyCompact
"
```

### Слишком много алертов (alert fatigue)

```bash
# Создать silence в Alertmanager на 1 час для конкретного IP
curl -X POST http://localhost:9093/api/v2/silences \
  -H "Content-Type: application/json" \
  -d '{
    "matchers": [{"name": "source_ip", "value": "1.2.3.4", "isRegex": false}],
    "startsAt": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "endsAt": "'$(date -u -d '+1 hour' +%Y-%m-%dT%H:%M:%SZ)'",
    "comment": "Pen-test in progress",
    "createdBy": "ops-engineer"
  }'
```

## 6. Runbook по алертам

### `SIEMIngestionStopped` (critical)
```bash
# 1. Проверить статус Vector
docker compose -f deploy/docker/docker-compose.yml ps vector-aggregator
docker compose -f deploy/docker/docker-compose.yml logs --tail=50 vector-aggregator

# 2. Проверить метрику
curl -s http://localhost:9598/metrics | grep siem_parser_events_parsed_total

# 3. Перезапустить если нужно
docker compose -f deploy/docker/docker-compose.yml restart vector-aggregator
```

### `SIEMHighParseLatency` / `SIEMHighParseErrorRate` (warning)
```bash
# p99 latency через Prometheus
curl -sg 'http://localhost:9090/api/v1/query?query=siem:parse_latency_p99:rate5m' | jq '.data.result'

# Ошибки парсинга — смотрим DLQ
docker exec siem-redpanda rpk topic consume siem.events.dlq --num 10

# Error rate
curl -sg 'http://localhost:9090/api/v1/query?query=siem:parser_events_error:rate5m/siem:parser_events_ok:rate5m' | jq
```

### `SIEMKafkaLagHigh` (warning)
```bash
# Текущий lag (recording rule)
curl -sg 'http://localhost:9090/api/v1/query?query=siem:kafka_consumer_lag:sum' | jq '.data.result'

# Просмотр offset напрямую через Redpanda
docker exec siem-redpanda rpk group describe clickhouse_siem_events

# Если CH не успевает — увеличить потоки ingestion в 02-kafka_ingest.sql:
# ALTER TABLE siem.events_kafka MODIFY SETTING kafka_num_consumers = 4;
```

### `ClickHouseHighDiskUsage` (warning)
```bash
# Свободное место
docker exec siem-clickhouse clickhouse-client \
  --query "SELECT disk_name, formatReadableSize(free_space), formatReadableSize(total_space) FROM system.disks"

# Принудительная очистка по TTL
docker exec siem-clickhouse clickhouse-client \
  --query "ALTER TABLE siem.events MODIFY TTL timestamp + INTERVAL 7 DAY"

# OPTIMIZE + merge
docker exec siem-clickhouse clickhouse-client --query "OPTIMIZE TABLE siem.events FINAL"
```

### `VectorAggregatorDown` (critical)
```bash
docker compose -f deploy/docker/docker-compose.yml up -d vector-aggregator
docker compose -f deploy/docker/docker-compose.yml logs --tail=30 vector-aggregator
```

### `BruteForceDetected` / `AccountTakeoverSuspected` (critical — T1110)
```bash
# Найти события по IP в ClickHouse
docker exec siem-clickhouse clickhouse-client --query "
  SELECT toStartOfMinute(timestamp) AS t, status_code, count()
  FROM siem.events
  WHERE source_ip = '<source_ip>' AND timestamp > now() - INTERVAL 1 HOUR
  GROUP BY t, status_code ORDER BY t"

# Блокировать IP на уровне docker network (аварийно):
# docker network disconnect siem-internal <container>

# Дашборд для расследования
# http://localhost:3000/d/siem-overview?var-source_ip=<ip>
```

### `RateLimitEvasion` (warning — T1595)
```bash
docker exec siem-clickhouse clickhouse-client --query "
  SELECT url_path, count() AS hits
  FROM siem.events
  WHERE source_ip = '<source_ip>' AND timestamp > now() - INTERVAL 5 MINUTE
  GROUP BY url_path ORDER BY hits DESC LIMIT 20"
```

### `SQLInjectionAttempt` (critical — T1190)
```bash
# Найти паттерны в URL
docker exec siem-clickhouse clickhouse-client --query "
  SELECT url_path, status_code, count()
  FROM siem.events
  WHERE source_ip = '<source_ip>'
    AND timestamp > now() - INTERVAL 15 MINUTE
    AND (url_path LIKE '%union%' OR url_path LIKE '%select%'
         OR url_path LIKE '%27%' OR url_path LIKE '%3B%')
  GROUP BY url_path, status_code ORDER BY count() DESC"

# Немедленно: проверить WAF/rate-limit правила на API Gateway
# Уведомить: security-team@example.com
```

### `PrivilegeEscalationAttempt` (critical — T1068)
```bash
docker exec siem-clickhouse clickhouse-client --query "
  SELECT user_id, url_path, status_code, timestamp
  FROM siem.events
  WHERE source_ip = '<source_ip>'
    AND url_path LIKE '%admin%'
  ORDER BY timestamp DESC LIMIT 50"

# Заблокировать сессию пользователя через API приложения (см. docs/ARCHITECTURE.md)
```

### `DataExfiltrationSuspected` (critical — T1041)
```bash
docker exec siem-clickhouse clickhouse-client --query "
  SELECT user_id, url_path, count() AS downloads
  FROM siem.events
  WHERE source_ip = '<source_ip>'
    AND status_code IN (200, 206)
    AND (url_path LIKE '%export%' OR url_path LIKE '%download%')
    AND timestamp > now() - INTERVAL 30 MINUTE
  GROUP BY user_id, url_path ORDER BY downloads DESC"

# Отозвать токен пользователя user_id немедленно
# Уведомить DPO если затронуты персональные данные
```

### `CriticalErrorSpike` (warning — T1499)
```bash
docker exec siem-clickhouse clickhouse-client --query "
  SELECT source_type, severity, count()
  FROM siem.events
  WHERE severity IN ('error', 'critical') AND timestamp > now() - INTERVAL 10 MINUTE
  GROUP BY source_type, severity ORDER BY count() DESC"
```

### `SIEMEPSAnomaly` (warning — T1499/T1078)
```bash
# Текущий и baseline EPS
curl -sg 'http://localhost:9090/api/v1/query?query=siem:events_total:rate5m' | jq
curl -sg 'http://localhost:9090/api/v1/query?query=siem:events_baseline:avg1h' | jq

# Разбивка по source_type
docker exec siem-clickhouse clickhouse-client --query "
  SELECT source_type, count() AS cnt
  FROM siem.events WHERE timestamp > now() - INTERVAL 5 MINUTE
  GROUP BY source_type ORDER BY cnt DESC"
```

---

## 7. Проверка Kafka consumer (detection-engine)

```bash
# Проверить что detection-engine потребляет события
docker logs detection-engine --tail 50 | grep -E "Kafka consumer|started|error"

# Метрики: события обработаны / ошибки парсинга
curl -s http://localhost:9110/metrics | grep -E "detection_events_processed|detection_parse_errors|detection_kafka"

# Consumer lag через Redpanda Admin API
curl -s http://localhost:9644/v1/groups | python3 -m json.tool

# Тест: отправить событие через Vector и проверить что detection-engine его получил
curl -s -X POST http://localhost:8080/logs \
  -H "Content-Type: application/json" \
  -d '{"Level":"Warning","Message":"Test auth failed","StatusCode":401,"RequestPath":"/auth/login","ClientIp":"1.2.3.4"}'
# Через ~2 сек: docker logs detection-engine --tail 10 | grep ALERT

# Проверить /ready endpoint (проверяет Redis + Kafka)
curl -s http://localhost:9110/ready | python3 -m json.tool

# Correlator
curl -s http://localhost:9111/ready | python3 -m json.tool
curl -s http://localhost:9111/api/v1/stats | python3 -m json.tool
```

---

## 8. Оценка производительности

```bash
# Benchmark: измерить текущий EPS
watch -n5 'curl -s http://localhost:9100/metrics | \
  grep "siem_parser_events_parsed_total{" | \
  awk "{print \$1, \$2}"'

# Load test: отправить 10k событий за 60 сек
cat > /tmp/load-test.sh << 'EOF'
#!/bin/bash
for i in $(seq 1 10000); do
  echo "{
    \"Level\": \"Info\",
    \"Message\": \"Load test event $i\",
    \"Timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",
    \"Properties\": {\"ClientIp\": \"10.0.0.$((RANDOM % 254 + 1))\"}
  }"
done | \
xargs -P 50 -I{} curl -s -X POST http://localhost:9000/ingest \
  -H "Content-Type: application/json" -d {} &
wait
echo "Load test complete"
EOF
bash /tmp/load-test.sh
```
