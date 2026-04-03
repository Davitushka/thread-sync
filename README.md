# SIEM-Lite

Production-grade SIEM система для микросервисного приложения на .NET 9 + React.

**Масштаб**: 10k EPS → 50k EPS без переписи.  
**Latency**: критические алерты ≤ 30 сек от события.  
**Parse SLA**: <5ms p99 на парсинг+нормализацию (Rust).

## Структура проекта

```
siem-lite/
├── docs/
│   ├── ARCHITECTURE.md      # Архитектура, Mermaid диаграммы, потоки данных
│   ├── SCHEMA.md            # Нормализованная схема + примеры до/после
│   ├── STACK.md             # Таблица стека с обоснованием и ресурсами
│   ├── RUNBOOK.md           # Операционные процедуры, backup, мониторинг
│   └── RISKS_AND_ROADMAP.md # Риски Rust, roadmap Phase 1-3
│
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
├── sigma-rules/
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
│       ├── Dockerfile.parser        # Multi-stage: rust:slim → debian:slim
│       ├── prometheus.yml           # Prometheus scrape config
│       ├── loki-config.yaml         # Loki конфигурация
│       ├── clickhouse/config.xml    # ClickHouse: memory, compression, RBAC
│       └── secrets/README.md        # Инструкция по созданию секретов
│
└── scripts/
    └── generate-certs.sh            # TLS сертификаты для Vector mTLS
```

## Быстрый старт

```bash
# 1. Создать секреты
cd deploy/docker/secrets
echo -n "smtp-pass" > smtp_password.txt
echo -n "https://hooks.slack.com/..." > slack_webhook_url.txt
echo -n "pd-key" > pagerduty_key.txt
echo -n "ClickHousePass123!" > clickhouse_password.txt
echo -n "MinIOSecret456!" > minio_secret_key.txt

# 2. Генерировать TLS сертификаты
bash scripts/generate-certs.sh

# 3. Запустить стек
docker compose -f deploy/docker/docker-compose.yml up -d

# 4. Открыть Grafana
open http://localhost:3000  # admin/ClickHousePass123!
```

Подробнее: [RUNBOOK.md](docs/RUNBOOK.md)

## Стек

| Слой | Технология | Язык |
|------|-----------|------|
| Collection | Vector 0.43 | Rust |
| Parsing/Normalization | siem-parser (custom) | **Rust** |
| Queue | Redpanda 23.x | C++ |
| Storage | ClickHouse 24.x + MinIO | C++ / Go |
| Detection | sigma-go + custom correlator | Go |
| Alerting | Alertmanager 0.27 | Go |
| Visualization | Grafana 11.4 | TypeScript |
| Self-monitoring | Prometheus + Loki | Go |

## Secrets Setup

Секреты **никогда** не коммитятся в репозиторий. Перед первым запуском создайте файлы в `deploy/docker/secrets/`:

```bash
cd deploy/docker/secrets

# Обязательные
echo -n "ClickHousePass123!"                       > clickhouse_password.txt
echo -n "MinIOSecret456!"                          > minio_secret_key.txt
echo -n "your-smtp-password"                       > smtp_password.txt
echo -n "https://hooks.slack.com/services/T.../..." > slack_webhook_url.txt
echo -n "your-pagerduty-routing-key"               > pagerduty_key.txt

chmod 600 *.txt
```

Подробнее: [deploy/docker/secrets/README.md](deploy/docker/secrets/README.md)

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
| `detection_events_processed_total` = 0 | Kafka consumer не подключился | Проверить `docker logs detection-engine`, убедиться что Redpanda healthy |
| Алерты не пишутся в siem.alerts | Alertmanager не достигает siem-parser | Проверить маршрут `clickhouse-siem` в Alertmanager, `curl http://localhost:7000/alerts/ingest` |
| Disk alert срабатывает постоянно | Мало места на диске или метрика не найдена | Проверить `curl http://localhost:9363/metrics | grep DiskAvailable` |

## Документация

- [Архитектура и потоки данных](docs/ARCHITECTURE.md)
- [Стек с обоснованием](docs/STACK.md)
- [Схема нормализации](docs/SCHEMA.md)
- [Runbook & Operations](docs/RUNBOOK.md)
- [Риски и Roadmap](docs/RISKS_AND_ROADMAP.md)
