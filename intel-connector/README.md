# intel-connector

Python-сервис: **MISP** и/или **HTTP JSON** и/или **локальный файл** → таблица **`siem.threat_intel`** в ClickHouse; опционально **Redis** для `SISMEMBER` в `siem-parser`.

Полная схема переменных и примеры — в [`docs/INTEL_CONNECTOR.md`](../docs/INTEL_CONNECTOR.md).

## Локальный прогон (без MISP)

```bash
pip install -r requirements.txt
set CLICKHOUSE_HOST=127.0.0.1
set CLICKHOUSE_PASSWORD=ClickHousePass123!
set INTEL_LOCAL_FEED_PATH=intel-connector/examples/feed-sample.json
set INTEL_RUN_ONCE=1
python -m intel_connector
```

> В Docker-образе пароль передаётся через `CLICKHOUSE_PASSWORD_FILE=/run/secrets/clickhouse_password` (Docker Secret), а не через `CLICKHOUSE_PASSWORD`.

## Docker

Сборка из корня репозитория (как в Compose):

`docker build -f deploy/docker/Dockerfile.intel .`
