# Стек технологий SIEM-Lite

Версии образов ниже согласованы с `deploy/docker/docker-compose.yml` (при расхождении правьте таблицу или compose). Указатель всех документов: [`README.md`](README.md).

## Таблица компонентов

| Компонент | Выбор | Версия | Язык | Лицензия | CPU/RAM @ 10k EPS | Почему | Альтернативы | Trade-offs |
|-----------|-------|--------|------|----------|---------------------|--------|--------------|-----------|
| **Log Collection** | Vector Agent | 0.43 | Rust | MPL-2.0 | 0.3 CPU / 128MB | Zero-copy, DFA transforms, disk buffer | Fluent Bit 2.x (C, меньше RAM), Fluentd (Ruby, больше plugins) | Vector: богатый VRL, но нет Lua; Fluent Bit: ниже memory footprint |
| **Log Aggregation** | Vector Aggregator | 0.43 | Rust | MPL-2.0 | 1 CPU / 256MB | Stateless, горизонтально масштабируется, VRL pipeline | Logstash 8.x (JVM, 512MB+ baseline), Cribl Stream (проприетарный) | Vector stateless — нет join между событиями |
| **Custom Parser** | siem-parser | 0.1 | Rust | Apache-2.0 | 1 CPU / 128MB | <5ms p99 SLA требует zero-GC, DFA regex, mmap GeoIP | Go parser (GC паузы ~1ms), Python (неприемлемо для SLA) | Rust: сложнее нанять, длинная компиляция |
| **Message Queue** | Redpanda | 23.3 | C++ | BSL-1.1 | 1 CPU / 512MB | Kafka-совместим, без JVM, 5-10x меньше latency | Apache Kafka 3.7 (JVM, 1GB+), NATS JetStream (меньше экосистема) | BSL лицензия ограничивает SaaS-перепродажу |
| **Storage (hot/warm)** | ClickHouse | 24.8 | C++ | Apache-2.0 | 2 CPU / 2GB | Лучшая in-class компрессия (5-10:1), колоночное хранение, fast aggregation | OpenSearch 2.x (JVM, хуже компрессия), Loki (только full-text) | CH: сложнее UPDATE/DELETE, не для OLTP |
| **Storage (cold)** | MinIO | 2024.11 | Go | AGPL-3.0 | 0.5 CPU / 256MB | S3-совместим, self-hosted, интеграция с CH tiered storage | AWS S3 (no egress cost в on-prem), GCS | MinIO AGPL — нужна enterprise лицензия для embedded |
| **Detection & correlation** | detection-engine-rs (в Compose сервис **`correlator`**, бинарь `correlator`; бинарь `detector` в крейте в стандартном Compose не поднимается) | 0.1 | Rust | Apache-2.0 | 1 CPU / 256MB (+Redis для state ~100–200MB) | Kafka consumer, правила на Rust (зеркало логики Sigma YAML в `sigma-rules/`), stateful окна в **Redis** | sigma-go, ElastAlert 2 | Правила не грузятся из YAML в рантайме — изменения в коде; YAML остаётся спецификацией |
| **Detection state store** | Redis | 7.x | C | BSD-3-Clause | 0.25 CPU / 256MB | Счётчики и окна correlator с TTL | KeyDB, Valkey | Память под горячие ключи |
| **GeoIP/ASN** | MaxMind GeoLite2 | 2024-11 | - | CC BY-SA 4.0 | mmap, 0 overhead | Стандарт индустрии, mmap reader, бесплатная tier | ip-api.com (внешний вызов, latency), DB-IP (похожая точность) | GeoLite2 менее точна чем GeoIP2 (платная) |
| **Alerting** | Alertmanager | 0.27 | Go | Apache-2.0 | 0.2 CPU / 64MB | Зрелый, grouping/dedup/silence/inhibition, интеграция с Grafana | Opsgenie (проприетарный), VictorOps | Нет native correlation между алертами |
| **Visualization** | Grafana | 11.4 | TypeScript/Go | AGPL-3.0 | 0.5 CPU / 256MB | ClickHouse plugin, богатые дашборды, RBAC | Kibana (требует Elasticsearch), Metabase (нет ClickHouse) | AGPL — встраивание в продукт требует лицензии |
| **Self-monitoring** | Prometheus | 2.55 | Go | Apache-2.0 | 0.5 CPU / 512MB | Стандарт для метрик, интеграция с Grafana | VictoriaMetrics (лучше масштаб, Apache-2.0), InfluxDB | Prometheus: хранение 15d по умолчанию |
| **Self-logs** | Loki | 3.3 | Go | AGPL-3.0 | 0.5 CPU / 512MB | Log aggregation самой SIEM, легковесный, Grafana native | Elasticsearch (тяжелее), CloudWatch (не self-hosted) | Loki: нет full-text индекса (только label query) |
| **Secrets** | Docker Secrets | built-in | - | Apache-2.0 | - | Нативный для Docker, файлы не попадают в env | HashiCorp Vault (более мощный), SOPS (file-based) | Docker Secrets только в Swarm mode; в Compose — файлы |

## Требования к ресурсам @ 10k EPS (один хост)

```
Компонент            CPU (cores)   RAM      Disk/mo
─────────────────────────────────────────────────────
Redpanda             2             1.5GB    ~50GB logs
ClickHouse           4             4GB      ~200GB (сжатый: ~30GB при 5:1)
siem-parser          2             512MB    –
Vector Aggregator    2             512MB    256MB buffer
correlator           1             256MB    –
Redis                0.25          256MB    RDB/AOF по политике
PostgreSQL           0.5           512MB    кейсы (case-management)
Prometheus           2             2GB      ~10GB (15d)
Loki                 1             1GB      ~20GB
Grafana              1             512MB    –
Alertmanager         0.5           256MB    –
MinIO                1             512MB    ~2TB/year (cold)
─────────────────────────────────────────────────────
ИТОГО                ~17 cores     ~12.5GB  ~2.3TB/year (cold included)
Рекомендуемый сервер: 32 vCPU, 32GB RAM, 500GB NVMe + 2TB HDD
```

## Масштабирование до 50k EPS

| Компонент | 10k EPS | 50k EPS | Способ масштабирования |
|-----------|---------|---------|----------------------|
| siem-parser | 1 инстанс, 2 CPU | 5 инстансов, 2 CPU каждый | Горизонтально за L4 LB |
| Vector Aggregator | 2 реплики | 10 реплик | Горизонтально, stateless |
| Redpanda | 1 node, 12 партиций | 3 nodes, 60 партиций | Добавить brokers |
| ClickHouse | 1 node | ClickHouse Cluster (3 shards × 2 replicas) | Шардирование по source_type |
| correlator | 1 инстанс | 3 инстанса (разные правила / group_id) | Партиционирование правил |
