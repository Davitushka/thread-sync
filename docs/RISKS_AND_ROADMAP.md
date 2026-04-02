# Риски, компромиссы и roadmap

## 1. Что упрощено в SIEM-Lite vs Enterprise SIEM

| Область | SIEM-Lite (текущее) | Enterprise SIEM | Workaround в lite |
|---------|---------------------|-----------------|-------------------|
| **Корреляция** | Простые sliding window в Go + Redis | Сложные multi-hop корреляции, граф событий (Splunk UEBA) | Добавить правила в correlator постепенно |
| **Threat Intel** | Нет интеграции с MISP/VirusTotal | Автоматический enrich по IP/hash/domain | Ручная проверка в Grafana; roadmap: Phase 2 |
| **SOAR** | Нет | Автоматический playbook (TheHive, Cortex) | Webhook в incident tracking (Jira) |
| **Machine Learning** | Только простые threshold anomalies | UEBA, entity analytics (Exabeam, Azure Sentinel) | Roadmap: Phase 3, MLflow |
| **Multi-tenancy** | Один namespace в ClickHouse | Полная изоляция tenant | Row-level security + отдельные БД |
| **Compliance reporting** | Нет готовых отчётов | SOC2, PCI-DSS, HIPAA dashboards | Grafana отчёты + SQL запросы |
| **Log parsing rules** | 4 типа источников | 500+ pre-built parsers | VRL скрипты, пополняемые вручную |
| **Retention policy UI** | Конфиг в SQL/YAML | Web UI для настройки retention | Ручное изменение TTL в SQL |
| **HA/Failover** | Single-node ClickHouse | ClickHouse cluster, ZooKeeper/Keeper | ReplicatedMergeTree + Keeper (Phase 2) |
| **Audit logs для SIEM** | Частичный (Loki) | Полный immutable audit trail | Добавить append-only Loki stream |

## 2. Масштабирование до 50k+ EPS без полной переписи

### Ботлнеки и решения

```
Current: 10k EPS (1 node)
Target:  50k EPS (distributed)

Узкое место → Решение
────────────────────────────────────────────────────────────────
siem-parser         → Горизонтальное масштабирование за Nginx L4:
                      5 инстансов × 10k EPS = 50k EPS
                      Нет изменений в коде (stateless)

Vector Aggregator   → 10 реплик за HAProxy (или K8s Deployment replicas=10)
                      Stateless — масштабируется без координации

Redpanda            → Расширить до 3-node cluster:
                      rpk cluster add-broker <ip>:9092
                      Увеличить партиции: rpk topic alter siem.events --partitions=60

ClickHouse          → ClickHouse Cluster (3 shards × 2 replicas):
                      - Шардирование: cityHash64(source_type) % 3
                      - Добавить ClickHouse Keeper (встроенный ZooKeeper)
                      - Мигрировать: INSERT INTO Distributed(...) SELECT * FROM events
                      Время миграции: ~2-4 часа для 1TB данных

Detection Engine    → Партиционировать правила по severity:
                      correlator-critical: только critical правила
                      correlator-high: high + medium правила
                      Kafka consumer groups: разные group_id
```

### K8s манифест для горизонтального масштабирования siem-parser

```yaml
# deploy/k8s/siem-parser-hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: siem-parser-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: siem-parser
  minReplicas: 2
  maxReplicas: 10
  metrics:
    - type: Pods
      pods:
        metric:
          name: siem_parser_events_parsed_total
        target:
          type: AverageValue
          averageValue: "8000"  # 8k EPS per pod → scale up
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
```

## 3. Roadmap: от SIEM-Lite к Full SIEM

### Phase 1 (текущее) — SIEM-Lite, 0-6 месяцев

- ✅ Log collection (Vector 4 источника)
- ✅ Parsing + PII masking (Rust)
- ✅ Storage (ClickHouse single-node)
- ✅ Detection (4 Sigma правила)
- ✅ Alerting (Alertmanager → Slack/Email/PD)
- ✅ Visualization (Grafana)

### Phase 2 — SIEM-Standard, 6-12 месяцев

```
Добавить:
├── Threat Intel Integration
│   ├── MISP connector (Python microservice, pull IoC каждые 1ч)
│   ├── Обогащение событий: IP → threat_score, malware_family
│   └── ClickHouse таблица: siem.threat_intel (IPv4, domain, hash → tags)
│
├── ClickHouse HA
│   ├── 3-node cluster с ReplicatedMergeTree
│   ├── ClickHouse Keeper (без ZooKeeper)
│   └── Distributed table для прозрачного шардирования
│
├── LDAP/Active Directory интеграция
│   ├── User enrichment: user_id → display_name, department, manager
│   └── RBAC в Grafana через LDAP groups
│
├── Расширенный correlator
│   ├── Lateral movement detection (user activity graph в Redis)
│   ├── Account compromise detection (geo velocity check)
│   └── Data exfiltration (upload volume anomaly)
│
└── Incident Management
    └── TheHive integration: алерт → case → investigation
```

### Phase 3 — SIEM-Enterprise, 12-24 месяца

```
Добавить:
├── UEBA (User and Entity Behavior Analytics)
│   ├── MLflow + Python: isolation forest для anomaly detection
│   ├── Baseline per-user, per-service (30-day rolling window)
│   └── Feature store в ClickHouse materialized views
│
├── SOAR (Security Orchestration, Automation, Response)
│   ├── n8n (self-hosted) или Shuffle для playbook automation
│   ├── Auto-block IP через firewall API (pfSense/OPNsense/AWS WAF)
│   └── Auto-disable compromised accounts через AD API
│
├── Full Compliance Reporting
│   ├── Grafana PDF reports (scheduled)
│   ├── Pre-built: GDPR Article 33, ISO27001 A.12.4, PCI-DSS Req.10
│   └── Evidence collection автоматизация
│
└── Extended Parsers
    ├── Windows Event Logs (WinRM/NXLog → Vector)
    ├── AWS CloudTrail, GCP Audit Logs
    ├── 30+ приложений через community Sigma rules
    └── CEF/LEEF parser для security appliances
```

## 4. Rust-специфичные риски

| Риск | Вероятность | Влияние | Митигация |
|------|-------------|---------|-----------|
| **Borrow checker learning curve** | Высокая для новых разработчиков | Средний (замедляет onboarding) | Rustlings курс для всех + пара-программирование на первые 2 недели |
| **Время компиляции** | Высокая (release: 3-8 мин) | Низкий (только при деплое) | Sccache для кеширования, incremental build в dev, pre-built Docker layers |
| **`unsafe` блоки** | Низкая (код не использует unsafe) | Критический (memory safety нарушена) | Cargo clippy `#[deny(unsafe_code)]` в lib.rs, обязательный code review |
| **Версионирование crates** | Средняя (breaking changes) | Средний | Cargo.lock в репозитории, Dependabot с еженедельными обновлениями |
| **Отладка production** | Средняя | Высокий (Rust паники сложнее дебажить) | `RUST_BACKTRACE=1`, structured logging через tracing, pprof через tokio-console |
| **rdkafka librdkafka** | Низкая (C библиотека через FFI) | Высокий (unsafe FFI) | Версия 0.36 протестирована, cmake-build для статической линковки |
| **GeoIP mmap** | Низкая | Средний (segfault если MMDB повреждён) | Валидация файла при старте, graceful degradation без GeoIP |

### Обязательные lint правила (добавить в `src/lib.rs`)

```rust
#![deny(unsafe_code)]
#![deny(unused_must_use)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
```

### CI pipeline для Rust (GitHub Actions)

```yaml
# .github/workflows/rust.yml
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - name: Format check
        run: cargo fmt --check
        working-directory: rust-parser
      - name: Clippy (deny warnings)
        run: cargo clippy --all-targets -- -D warnings
        working-directory: rust-parser
      - name: Tests
        run: cargo test --locked
        working-directory: rust-parser
      - name: Benchmark (smoke)
        run: cargo bench --bench parse_benchmark -- --output-format bencher | tee bench.txt
        working-directory: rust-parser
      - name: Security audit
        uses: rustsec/audit-check@v2
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
          working-directory: rust-parser
```
