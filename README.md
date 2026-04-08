# SIEM-Lite

Production-grade SIEM система для микросервисного приложения на .NET 9 + React.

**Масштаб**: 10k EPS → 50k EPS без переписи.  
**Latency**: критические алерты ≤ 30 сек от события.  
**Parse SLA**: <5ms p99 на парсинг+нормализацию (Rust).

## Структура проекта

```
siem-lite/
├── docs/
│   ├── SIEM_PORTAL.md       # Единая SOC-консоль (Rust): прокси API + UI
│   ├── ARCHITECTURE.md      # Архитектура, Mermaid диаграммы, потоки данных
│   ├── SCHEMA.md            # Нормализованная схема + примеры до/после
│   ├── STACK.md             # Таблица стека с обоснованием и ресурсами
│   ├── RUNBOOK.md           # Операционные процедуры, backup, мониторинг
│   └── RISKS_AND_ROADMAP.md # Риски Rust, roadmap Phase 1-3
│
├── siem-portal/             # Rust: единый веб-портал (метрики, алерты, кейсы)
├── siem-operator/           # Rust: нативное десктоп-приложение оператора (egui) → API кейсов
├── rust-parser/             # Rust: высокопроизводительный парсер
│   ├── src/
│   │   ├── main.rs          # HTTP сервер (axum), Kafka producer
│   │   ├── lib.rs           # Публичный API крейта
│   │   ├── parser.rs        # Детектирование формата, парсинг JSON/CEF/syslog
│   │   ├── pii.rs           # PII маскирование (regex-automata DFA)
│   │   ├── enrichment.rs    # GeoIP/ASN lookup (maxminddb mmap)
│   │   ├── normalizer.rs    # Pipeline: parse → PII → enrich
│   │   ├── schema.rs        # NormalizedEvent структура
│   │   ├── config.rs        # Конфигурация из env/файла
│   │   └── metrics.rs       # Prometheus метрики
│   ├── benches/
│   │   └── parse_benchmark.rs  # Criterion бенчмарки
│   └── Cargo.toml
│
├── vector/
│   ├── agent.yaml           # Vector Agent (sidecar на каждой ноде)
│   └── aggregator.yaml      # Vector Aggregator (stateless, VRL нормализация)
│
├── clickhouse/
│   └── init.sql             # Схема: events, alerts, materialized views, TTL
│
├── sigma-rules/                     # Спецификация правил (Sigma YAML); рантайм — detection-engine-rs
│   ├── brute_force_api.yaml         # T1110: Brute-force на API/SignalR
│   ├── rate_limit_evasion.yaml      # T1595: Аномальный объём запросов
│   ├── sql_injection.yaml           # T1190: SQLi/NoSQLi попытки
│   └── privilege_escalation.yaml    # T1068: Доступ к admin endpoints
│
├── alerting/
│   ├── alertmanager.yaml            # Роутинг: severity → Slack/Email/PagerDuty
│   ├── prometheus-rules.yaml        # Alert rules: SIEM health + detection
│   └── templates/siem.tmpl          # Шаблоны сообщений
│
├── grafana/
│   ├── provisioning/                # Datasources + dashboards provisioning
│   └── dashboards/siem-overview.json  # Главный дашборд
│
├── deploy/
│   └── docker/
│       ├── docker-compose.yml       # Полный стек: все сервисы
│       ├── .env.example             # Шаблон переменных (пароли и опц. API key)
│       ├── Dockerfile.parser        # Multi-stage: rust:slim → debian:slim
│       ├── prometheus.yml           # Prometheus scrape config
│       ├── loki-config.yaml         # Loki конфигурация
│       ├── clickhouse/config.xml    # ClickHouse: memory, compression, RBAC
│       └── secrets/README.md        # Инструкция по созданию секретов
│
└── scripts/
    └── generate-certs.sh            # TLS сертификаты для Vector mTLS
```

## Полный запуск (Docker Compose)

Рабочая директория — **корень репозитория** (`siem-lite/`). Нужны **Docker 24+** и **Compose v2** (команда `docker compose`).

### 1. Секреты

Создайте файлы в [`deploy/docker/secrets/`](deploy/docker/secrets/README.md) (репозиторий их не хранит):

| Файл | Назначение |
|------|------------|
| `clickhouse_password.txt` | Пароль пользователя `siem` в ClickHouse; **тот же** пароль зашивается в Grafana admin (`GF_SECURITY_ADMIN_PASSWORD__FILE`) |
| `minio_secret_key.txt` | Пароль root в MinIO Console (**логин** `siemadmin`) |
| `smtp_password.txt`, `slack_webhook_url.txt`, `pagerduty_key.txt` | Alertmanager (можно заглушки для чисто локальной проверки) |

Пример (Linux/macOS/Git Bash; в PowerShell используйте `Set-Content -NoNewline`):

```bash
cd deploy/docker/secrets
echo -n "ClickHousePass123!"   > clickhouse_password.txt
echo -n "MinIOSecret456!"     > minio_secret_key.txt
echo -n "placeholder-smtp"   > smtp_password.txt
echo -n "https://hooks.slack.com/services/placeholder" > slack_webhook_url.txt
echo -n "placeholder-pd"     > pagerduty_key.txt
```

На Windows из корня репозитория: `pwsh -File scripts/init-secrets.ps1`

**Согласованность:** если задаёте свои пароли через [`deploy/docker/.env`](deploy/docker/.env.example), значение `CLICKHOUSE_PASSWORD` должно **байт-в-байт** совпадать с `clickhouse_password.txt`.

### 2. Опционально: переменные окружения

Скопируйте `deploy/docker/.env.example` → `deploy/docker/.env` и при необходимости измените пароли. Запуск:

```bash
docker compose --env-file deploy/docker/.env -f deploy/docker/docker-compose.yml up -d --build
```

Без `.env` подставляются значения по умолчанию из compose-файла (удобно для первого раза).

### 3. Опционально: TLS для Vector, GeoIP

- **mTLS Agent → Aggregator:** в локальном `vector/aggregator.yaml` TLS отключён. Для прода сгенерируйте сертификаты: `bash scripts/generate-certs.sh` и включите TLS в конфигах Vector (см. [RUNBOOK](docs/RUNBOOK.md)).
- **GeoIP:** без файлов `.mmdb` siem-parser стартует, geo-поля будут пустыми; см. раздел [GeoIP Setup](#geoip-setup) ниже.

### 4. Запуск всего стека

Одной командой (сборка образов Rust при необходимости):

```bash
docker compose -f deploy/docker/docker-compose.yml up -d --build
```

Проверка:

```bash
docker compose -f deploy/docker/docker-compose.yml ps
curl -s http://localhost:7000/health
```

Почему часть панелей в Grafana пустая: метрики **Prometheus** (`siem_events_total` и др.) и таблицы **ClickHouse** заполняются по-разному — см. [`docs/DATA_PROMETHEUS_GRAFANA.md`](docs/DATA_PROMETHEUS_GRAFANA.md).

Чтобы **дашборды ClickHouse** (Overview, Alert Management, SOC Workbench) не были пустыми сразу после первого старта, загрузите сид:

```bash
bash scripts/seed-data/bootstrap_clickhouse.sh
```

Альтернатива: `docker compose -f deploy/docker/docker-compose.yml --profile seed up soc-seed`. Подробнее: [`scripts/seed-data/README.md`](scripts/seed-data/README.md). Для метрик Prometheus (`siem_events_total`, детекция) уже крутится `log-generator`; дополнительный всплеск: `docker compose -f deploy/docker/docker-compose.yml run --rm siem-stress`.

### 5. Опционально: панель SIEM Admin

Сервис `siem-admin` в профиле `admin` и **не стартует** вместе с основным стеком:

```bash
docker compose -f deploy/docker/docker-compose.yml --profile admin up -d --build siem-admin
```

UI: http://localhost:8089 (нужен доступ Docker-сокета на хосте, см. compose).

Кнопка **Fill All Data** в админке выполняет тот же сценарий, что `scripts/seed-data/bootstrap_clickhouse.sh`: полный `seed_test_events.sql` в ClickHouse (включая `siem.threat_intel`), затем нагрузка и прогрев парсера. Путь к SQL: переменная `SOC_SEED_SQL_PATH` или файл `/app/seed/seed_test_events.sql` (в образе и через volume в compose).

### 6. Опционально: pgAdmin для Postgres (`soc_cases`)

Веб-pgAdmin с заранее добавленным сервером (хост `postgres` внутри Docker, БД `soc_cases`, пользователь `siem_soc`):

```bash
docker compose -f deploy/docker/docker-compose.yml --profile tools up -d pgadmin
```

Откройте **http://localhost:5050** и войдите с учётными данными из `PGADMIN_EMAIL` / `PGADMIN_PASSWORD` ([`deploy/docker/.env.example`](deploy/docker/.env.example); пароль веб-интерфейса по умолчанию: `changeme-pgadmin`). Для подключения к базе укажите пароль пользователя **`siem_soc`**, тот же что **`POSTGRES_PASSWORD`**. Настройки сервера: [`deploy/docker/pgadmin/servers.json`](deploy/docker/pgadmin/servers.json).

### Эндпоинты после старта

| Сервис | URL | Учётные данные / примечание |
|--------|-----|-----------------------------|
| Grafana | http://localhost:3000 | `admin` + пароль из `clickhouse_password.txt` |
| Prometheus | http://localhost:9090 | — |
| Alertmanager | http://localhost:9093 | — |
| Case management (главная приложения) | http://localhost:8088/ | Список кейсов: `/cases`; расследование: `/cases/:id/investigate`. API — [RUNBOOK §9](docs/RUNBOOK.md) |
| SIEM Portal (SOC консоль, Rust) | http://localhost:8091 | Прокси к Prometheus, Alertmanager и case-management без новых БД — [docs/SIEM_PORTAL.md](docs/SIEM_PORTAL.md) |
| siem-parser | http://localhost:7000/health | Метрики: http://localhost:9100/metrics |
| Vector HTTP ingest | http://localhost:8080/logs | NDJSON (см. [vector/aggregator.yaml](vector/aggregator.yaml)) |
| Loki (логи контейнеров) | в Grafana → Explore, datasource **Loki** | Promtail шлёт stdout/stderr Docker в Loki (`siem-promtail`) |
| MinIO Console | http://localhost:9001 | `siemadmin` + пароль из `minio_secret_key.txt`; после `up` создаются бакеты `siem-cold`, `siem-archive` (`minio-init`) |
| SIEM Admin (профиль) | http://localhost:8089 | После `compose --profile admin up` |
| pgAdmin (профиль `tools`) | http://localhost:5050 | После `compose --profile tools up -d pgadmin`; см. §6 |

**Поток событий SIEM:** приложения / генератор → Vector `:8080/logs` → Kafka `siem.events` → ClickHouse (`events_kafka_queue` / MV) → Grafana (ClickHouse). **События по умолчанию не складываются в MinIO** — S3 для cold tier подключается отдельно в конфиге ClickHouse (см. [clickhouse/init.sql](clickhouse/init.sql)). В MinIO уже есть пустые бакеты под будущий tier/бэкапы.

**Grafana datasource ClickHouse:** пароль берётся из переменной `CLICKHOUSE_DATASOURCE_PASSWORD` в сервисе Grafana (по умолчанию совпадает с `CLICKHOUSE_PASSWORD` в compose); он должен совпадать с паролем пользователя `siem` и с `clickhouse_password.txt`.

### Наполнение дашбордов и остановка

- Сид демо-событий: `bash scripts/seed-data/seed.sh` (или дождаться `siem-log-generator` / `siem-stress` из compose).
- Остановка без удаления томов:  
  `docker compose -f deploy/docker/docker-compose.yml stop`  
- Полное удаление контейнеров и **данных** томов:  
  `docker compose -f deploy/docker/docker-compose.yml down --volumes`

Операции, бэкапы, проверки пайплайна: [docs/RUNBOOK.md](docs/RUNBOOK.md).

## Стек

| Слой | Технология | Язык |
|------|-----------|------|
| Collection | Vector 0.43 | Rust |
| Parsing/Normalization | siem-parser (custom) | **Rust** |
| Queue | Redpanda 23.x | C++ |
| Storage | ClickHouse 24.x + MinIO | C++ / Go |
| Detection | correlator (detection-engine-rs): Kafka → правила → Alertmanager (Rust) | Rust |
| Alerting | Alertmanager 0.27 | Go |
| Visualization | Grafana 11.4 | TypeScript |
| Self-monitoring | Prometheus + Loki | Go |

## Переменные Compose и защита ingest

Пароли сервисов в compose задаются как `${VAR:-значение_по_умолчанию}`; переопределение — через `deploy/docker/.env` (см. раздел **Полный запуск** выше).

**siem-parser:** при установке `SIEM_PARSER_API_KEY` в `.env` или в environment сервиса для `POST /parse` и `POST /alerts/ingest` нужны заголовки `X-API-Key` или `Authorization: Bearer …`. В Alertmanager можно добавить `http_config.bearer_token_file` (см. комментарий в `alerting/alertmanager.yaml`). По умолчанию ключ пустой — Vector→Kafka не затрагивается.

## Secrets Setup

Краткий чеклист файлов — в разделе **Полный запуск** выше. Детали и production: [deploy/docker/secrets/README.md](deploy/docker/secrets/README.md).

> **Для production** рекомендуется [SOPS + age](https://github.com/getsops/sops) для шифрования файлов секретов в git.

## GeoIP Setup

GeoIP обогащение опционально — без него события пишутся без geo-полей.

```bash
# Зарегистрироваться на maxmind.com, скачать GeoLite2-City.mmdb и GeoLite2-ASN.mmdb
# Скопировать в docker volume:
docker volume create siem-lite_geoip-data
docker run --rm -v siem-lite_geoip-data:/target -v /path/to/mmdb:/src alpine \
  sh -c "cp /src/GeoLite2-City.mmdb /target/ && cp /src/GeoLite2-ASN.mmdb /target/"
```

## Troubleshooting

| Симптом | Причина | Решение |
|---------|---------|---------|
| `siem-parser` не стартует | Нет Kafka при старте | Зависимость от `redpanda` healthy — проверить `docker compose ps` |
| ClickHouse auth error | Несовпадение пароля | Проверить `deploy/docker/secrets/clickhouse_password.txt` |
| Grafana нет данных | MV пустые (нет событий) | Запустить сид: `bash scripts/seed-data/seed.sh` |
| `detection_events_processed_total` = 0 | Correlator не потребляет Kafka | `docker logs siem-correlator`, Redpanda healthy, `curl http://localhost:9111/ready` |
| Алерты не пишутся в siem.alerts | Alertmanager не достигает siem-parser | Проверить маршрут `clickhouse-siem` в Alertmanager, `curl http://localhost:7000/alerts/ingest` |
| Disk alert срабатывает постоянно | Мало места на диске или метрика не найдена | Проверить `curl http://localhost:9363/metrics | grep DiskAvailable` |

## Документация

- [Архитектура и потоки данных](docs/ARCHITECTURE.md)
- [Стек с обоснованием](docs/STACK.md)
- [Схема нормализации](docs/SCHEMA.md)
- [Runbook & Operations](docs/RUNBOOK.md)
- [Риски и Roadmap](docs/RISKS_AND_ROADMAP.md)
