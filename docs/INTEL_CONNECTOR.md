# Threat intelligence (`intel-connector`)

Сервис **Phase 2**: загрузка IoC в `siem.threat_intel` (ClickHouse) и опционально в Redis SET'ы для **мгновенного матча** в `siem-parser`.

## Запуск (Docker Compose)

Профиль **`intel`** (основной стек без изменений):

```bash
docker compose -f deploy/docker/docker-compose.yml --profile intel up -d --build intel-connector
```

По умолчанию подхватывается локальный файл `intel-connector/examples/feed-sample.json` и данные пишутся в ClickHouse с `feed=local_feed`; при `INTEL_SYNC_REDIS=1` (по умолчанию) наборы `siem:intel:ipv4`, `siem:intel:domain`, `siem:intel:sha256` пересоздаются в Redis.

## Обогащение в парсере

В `siem-parser` задайте (в `.env` или environment):

```env
SIEM__INTEL__REDIS_URL=redis://:changeme@redis:6379/0
```

> Redis в compose запускается с `--requirepass`; URL должен содержать пароль в формате `redis://:password@host:port/db`. В compose-файле переменная `INTEL_REDIS_URL` для intel-connector подставляется автоматически через `${REDIS_PASSWORD:-changeme}`.

После перезапуска парсера события с `source_ip` ∈ `siem:intel:ipv4` получают в `metadata`: `threat_intel_match=true`, `threat_intel_ioc_type=ipv4`. Метрика Prometheus: `siem_parser_intel_ioc_match_total`.

## Переменные окружения

| Переменная | Назначение |
|------------|------------|
| `CLICKHOUSE_HOST`, `CLICKHOUSE_PORT`, `CLICKHOUSE_USER`, `CLICKHOUSE_DATABASE` | Подключение к ClickHouse |
| `CLICKHOUSE_PASSWORD` или `CLICKHOUSE_PASSWORD_FILE` | Пароль |
| `INTEL_POLL_INTERVAL_SEC` | Интервал опроса (по умолчанию 3600) |
| `INTEL_RUN_ONCE` | `1` — один цикл и выход (удобно для cron/K8s Job) |
| `INTEL_MISP_URL`, `INTEL_MISP_API_KEY` | Экземпляр MISP (`POST …/attributes/restSearch`, заголовок `Authorization`) |
| `INTEL_MISP_LIMIT` | Лимит атрибутов (по умолчанию 5000) |
| `INTEL_FEED_URL` | HTTP(S) JSON: массив IoC или `{"iocs":[…]}` |
| `INTEL_HTTP_FEED_NAME` | Значение колонки `feed` для HTTP-фида (по умолчанию `http_feed`) |
| `INTEL_LOCAL_FEED_PATH` | Путь к JSON внутри контейнера (по умолчанию `/app/examples/feed-sample.json`) |
| `INTEL_SYNC_REDIS` | `1` — зеркалировать IoC в Redis |
| `INTEL_REDIS_URL` | URL Redis для зеркала (должен содержать пароль: `redis://:password@host:port/db`) |
| `INTEL_INSECURE_SKIP_VERIFY` | `1` — не проверять TLS (только отладка) |

Формат элемента IoC в JSON: `ioc_type` (`ipv4` \| `domain` \| `sha256` \| `ipv6`), `ioc_value`, опционально `threat_label`, `tags`, `confidence`.

## MISP

Укажите базовый URL без завершающего слэша и API-ключ с правом `attributes/restSearch`. Типы: `ip-src`, `ip-dst`, `domain`, `hostname`, `sha256`, `ipv6` и др. (см. код маппинга в `intel_connector/main.py`).

См. также: [`RISKS_AND_ROADMAP.md`](RISKS_AND_ROADMAP.md) (Phase 2), [`README.md`](../intel-connector/README.md).
