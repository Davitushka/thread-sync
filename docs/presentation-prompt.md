# Промпт для AI-генератора презентаций

## Как использовать

1. Открой **Gamma.app** (или Tome.app) → Create → Paste prompt
2. Вставь промпт ниже полностью
3. Выбери стиль: **Technology / Cybersecurity** (тёмная тема)
4. Сгенерируй, затем отредактируй если нужно

---

## ПРОМПТ (копируй отсюда)

Create a professional presentation about "SIEM-Lite — Intrusion Detection System Based on Log Analysis". The presentation should have 25 slides with a dark cybersecurity theme (dark blue/black backgrounds, electric blue accents, monospace fonts for code). Use modern, clean design with subtle grid/tech patterns as backgrounds.

For EACH slide, include a relevant image/icon/illustration as described below. Generate or suggest images that match the cybersecurity theme.

---

### Slide 1 — Title Slide
**Title:** SIEM-Lite
**Subtitle:** Система обнаружения вторжений на основе анализа логов
**Footer:** Production-grade SIEM для микросервисных приложений | 10k→50k EPS | Алерты ≤30 сек

**Image suggestion:** Full-width hero image showing a cybersecurity dashboard with glowing blue nodes and data flow lines on a dark background. Abstract network visualization with interconnected nodes.

---

### Slide 2 — Agenda
**Title:** Оглавление

- Проблема: зачем нужна SIEM
- Обзор решения SIEM-Lite
- Архитектура и потоки данных
- Технологический стек (6 языков)
- Rust-парсер: производительность <5ms
- Detection Engine: Sigma правила
- ClickHouse: аналитика в реальном времени
- Алертинг и визуализация
- Демо: запуск за 5 минут
- Масштабирование и Roadmap

**Image suggestion:** Numbered list with cyber-shield icons next to each section. Minimalist design.

---

### Slide 3 — The Problem
**Title:** Зачем нужна SIEM-система?

- Микросервисы генерируют **тысячи событий в секунду**
- Атаки незаметны в потоке: brute-force, SQLi, privilege escalation
- Ручной анализ **невозможен** при 10k+ EPS
- Compliance: ISO 27001, SOC 2, PCI DSS требуют централизованный мониторинг
- Рынок: Splunk, ELK, Wazuh — дорого, сложно, избыточно

**Image suggestion:** Illustration showing a wall of server racks with thousands of log lines flowing out, and a magnifying glass highlighting a few malicious patterns (red) among normal events (blue/green).

---

### Slide 4 — The Solution
**Title:** SIEM-Lite: Лёгкая, быстрая, открытая

| Параметр | Значение |
|----------|----------|
| Масштаб | 10k → 50k EPS без переписи |
| Latency алертов | ≤ 30 секунд |
| Парсинг | <5ms p99 (Rust) |
| Стоимость | Open-source |
| Деплой | 5 минут через Docker |

**Image:** A comparison scale showing SIEM-Lite as lightweight (small, fast rocket) vs competitors as heavy (large, slow cargo ships).

**Image suggestion:** Minimalist comparison graphic — on the left a heavy, complex box labeled "Splunk/ELK" with $$$, on the right a sleek, lightweight box labeled "SIEM-Lite" with $.

---

### Slide 5 — Architecture Overview
**Title:** Архитектура системы

Show the data flow as a diagram:

```
Vector Agent → siem-parser (Rust) → Redpanda → ClickHouse → Grafana
                                                    → Detection Engine (Go) → Alertmanager
```

**Key layers:**
- **Collection:** Vector Agent на каждой ноде
- **Processing:** Rust парсер (PII masking, GeoIP)
- **Queue:** Redpanda (12 партиций)
- **Storage:** ClickHouse (ReplacingMergeTree)
- **Detection:** Go + Sigma правила
- **Alerting:** Slack / Email / PagerDuty

**Image suggestion:** Clean architecture diagram with 6 boxes connected by arrows, each with a technology icon (Rust crab, Go gopher, ClickHouse logo, etc.). Use blue/teal color scheme.

---

### Slide 6 — Data Flow
**Title:** Путь одного события

1. **Сбор** — Vector Agent читает логи приложения
2. **Агрегация** — VRL-нормализация
3. **Парсинг** — HTTP POST батчи → JSON/CEF/syslog
4. **Обогащение** — PII masking + GeoIP
5. **Очередь** — Redpanda, snappy, 24h retention
6. **Хранение** — ClickHouse, TTL 365 дней
7. **Детектирование** — Sigma правила + корреляция
8. **Алерт** → Slack / Email / PagerDuty

**Image suggestion:** Horizontal pipeline diagram with 8 stages, each represented by a glowing node with an icon. Data packet (glowing dot) flows from left to right. Dark background with electric blue accents.

---

### Slide 7 — Technology Stack
**Title:** Технологический стек

| Слой | Технология | Язык | RAM @ 10k EPS |
|------|-----------|------|---------------|
| Collection | Vector 0.43 | Rust | 256M |
| Parsing | siem-parser | **Rust** | 256M |
| Queue | Redpanda 23.x | C++ | 1.5G |
| Storage | ClickHouse 24.x | C++ | 4G |
| Detection | sigma-go + correlator | **Go** | 256M |
| Alerting | Alertmanager 0.27 | Go | 256M |
| Visualization | Grafana 11.4 | TypeScript | 512M |
| Monitoring | Prometheus + Loki | Go | 2G |

**Итого: ~12 GB RAM**

**Image suggestion:** Technology logos arranged in a grid (Rust, Go, ClickHouse, Grafana, Prometheus, Redpanda, Vector). Clean, monochrome with colored highlights.

---

### Slide 8 — Programming Languages
**Title:** 6 языков программирования

| Язык | Компонент | Файлов |
|------|-----------|--------|
| **Rust** | Парсинг, PII, GeoIP | 13 `.rs` |
| **Go** | Detection Engine | `.go` |
| **SQL** | ClickHouse схема | `init.sql` |
| **JavaScript** | Grafana дашборды | `.json` |
| **Bash** | Скрипты | `.sh` |
| **VRL** | Vector трансформации | в `.yaml` |

**Конфигурация:** YAML, TOML, JSON, Dockerfile, Mermaid

**Image suggestion:** Language logos (Rust, Go, SQL, JS, Bash) arranged as orbiting planets around a central "SIEM-Lite" core. Dark space background.

---

### Slide 9 — Rust Parser: Why Rust?
**Title:** Rust-парсер: ядро производительности

**Преимущества Rust:**
- ⚡ **p99 < 5ms** — критично для SLA
- 🛡️ **Memory safety** — нет buffer overflow, data races
- 🪶 **Малый footprint** — 256M RAM при 10k EPS
- 🔧 **Zero-cost abstractions** — DFA regex без backtracking

**Модули:**
- `parser.rs` — JSON, CEF, syslog
- `pii.rs` — маскирование PII (DFA regex)
- `enrichment.rs` — GeoIP/ASN + LRU-кэш
- `normalizer.rs` — Pipeline + SLA check
- `metrics.rs` — 5 Prometheus метрик

**Image suggestion:** Rust programming language logo (crab) with performance metrics displayed as glowing gauges (speed, memory, latency) on a dark background.

---

### Slide 10 — Rust Parser: Code Example
**Title:** PII Masking — DFA без backtracking

```rust
// regex-automata с DFA — детерминированный автомат
static EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
});

pub fn mask_pii(input: &str) -> Option<String> {
    if EMAIL_RE.is_match(input) {
        Some(EMAIL_RE.replace_all(input, "***@***.***").into_owned())
    } else {
        None // zero-allocation, если нет совпадений
    }
}
```

**Release профиль:**
```
opt-level = 3 | LTO: fat | codegen-units: 1
strip: true | panic: abort
```

**Image suggestion:** Side-by-side comparison: on the left, a regex with backtracking shown as a tangled, slow maze (red); on the right, a DFA shown as a clean, straight path (green/blue).

---

### Slide 11 — Detection Engine
**Title:** Sigma правила: обнаружение атак

**Что такое Sigma?**
- Открытый формат detection rules (YAML)
- MITRE ATT&CK маппинг
- Конвертируется в Splunk, Elastic, Graylog

| Правило | MITRE | Порог |
|---------|-------|-------|
| Brute-force API | T1110 | 10+ failed logins / 2 min |
| SQL Injection | T1190 | SQLi паттерны в запросах |
| Rate Limit Evasion | T1595 | >500 запросов / мин |
| Privilege Escalation | T1068 | Доступ к admin endpoints |

**Image suggestion:** MITRE ATT&CK matrix visualization with 4 highlighted cells (T1110, T1190, T1595, T1068) glowing in red. Dark background.

---

### Slide 12 — Detection: Correlation
**Title:** Корреляция событий

```
Redpanda consumer → Detection Engine (Go) → Alertmanager
                        ↓
                      Redis (sliding windows)
```

**Принцип работы:**
- **Sliding window** в Redis: 2/5/15 минут
- **Threshold-based:** brute-force (>10), rate limit (>500)
- **Correlator:** объединяет события от одного IP
- **Группировка:** один алерт на атаку, а не на каждое событие

**Image suggestion:** Timeline graphic showing events clustering — multiple small dots (events) merging into one larger alert bubble. Redis icon as the storage layer.

---

### Slide 13 — ClickHouse Storage
**Title:** Хранение аналитики

**Таблица `siem.events`:**
- `ReplacingMergeTree(ingest_ts)` — авто-дедупликация
- `ORDER BY (source_type, timestamp, event_id)`
- **TTL:** 365 дней
- **LowCardinality** для source_type, severity, host
- **Materialized columns:** severity_num, ingest_lag_ms

**Materialized Views:**
- `events_by_severity` — агрегация за 5 мин
- `events_by_source` — распределение источников
- `top_source_ips` — топ IP за окно

**Image suggestion:** Columnar database visualization — vertical columns of data with labels (timestamp, IP, severity, message), showing how ClickHouse stores data column-wise vs row-wise.

---

### Slide 14 — Alerting Pipeline
**Title:** Маршрутизация алертов

```
Severity routing:
  critical ──▶ PagerDuty  (немедленно)
  high     ──▶ Slack      (#security-alerts)
  medium   ──▶ Slack      (#siem-alerts)
  low      ──▶ Email      (daily digest)
```

| Правило | Условие | Для кого |
|---------|---------|----------|
| SIEMIngestionStopped | `rate = 0` 2 мин | Ops (PagerDuty) |
| SIEMParseLatencyHigh | `p99 > 5ms` 3 мин | Ops (Slack) |
| SIEMErrorRateHigh | `error > 1%` | Dev (Slack) |
| SIEMDiskSpaceLow | `< 15%` | Ops (PagerDuty) |

**Image suggestion:** Routing diagram showing a central Alertmanager box branching into 4 channels (PagerDuty phone, Slack logo, Email envelope) with color-coded severity levels (red, orange, yellow, green).

---

### Slide 15 — Grafana Dashboards
**Title:** Визуализация и мониторинг

**SIEM Overview дашборд:**
- 📊 EPS график — real-time
- 🥧 Severity breakdown — pie chart
- 🌍 Top source IPs — GeoIP карта
- 🚨 Detection alerts — таблица с MITRE ID
- ⏱️ Parse latency — p50/p95/p99 гистограмма
- 📈 ClickHouse ingestion lag

**Доступ:** `http://localhost:3000`

**Image suggestion:** Mock Grafana dashboard screenshot — dark theme with multiple panels: line chart (EPS), pie chart (severity), world map with glowing dots (GeoIP), alert table (red/yellow rows).

---

### Slide 16 — Security: PII Masking
**Title:** Безопасность данных

| Тип данных | Маска |
|------------|-------|
| 📧 Email | `***@***.***` |
| 📱 Телефон | `***-***-****` |
| 🔑 Токен JWT | `***REDACTED***` |
| 💳 Карта | `****-****-****-1234` |

**Принципы:**
- Маскирование **до** попадания в хранилище
- DFA regex — без backtracking, без overhead
- URL query strings sanitization
- Docker Secrets для SMTP, Slack, PagerDuty
- mTLS для Vector Agent → Aggregator

**Image suggestion:** Shield icon with data flowing through it — on the left side raw data with visible emails/tokens, on the right side masked data with `***` patterns.

---

### Slide 17 — Self-Monitoring
**Title:** Мониторинг самой SIEM

| Метрика | Норма | Алерт |
|---------|-------|-------|
| Parse p99 | <2ms | >5ms (3 мин) |
| Error rate | <0.1% | >1% (3 мин) |
| Kafka lag | <1000 | >100000 (5 мин) |
| CH disk | >30% | <15% |

**Prometheus + Loki** для self-monitoring

Каждый сервис: `/health` endpoint

**Image suggestion:** Dashboard with 4 gauges (like car dashboard) showing green/yellow/red zones. Each gauge labeled with a metric name.

---

### Slide 18 — Quick Start Demo
**Title:** Запуск за 5 минут

```bash
# 1. Секреты (1 мин)
echo -n "ClickHousePass123!" > secrets/clickhouse_password.txt
echo -n "slack-webhook" > secrets/slack_webhook_url.txt

# 2. Запуск (3 мин)
docker compose -f deploy/docker/docker-compose.yml up -d

# 3. Проверка (1 мин)
curl http://localhost:7000/health
curl http://localhost:3000/api/health
```

**Grafana:** `admin` / `ClickHousePass123!`

**Image suggestion:** Timer/stopwatch showing "5:00" with three checkmarks appearing sequentially. Clean, minimal design.

---

### Slide 19 — Attack Detection Demo
**Title:** Обнаружение brute-force атаки

```bash
# 12 попыток входа с одного IP
for i in $(seq 1 12); do
  curl -X POST http://localhost:7000/parse \
    -d '[{"Level":"Warning","Message":"401 Unauthorized",
          "Properties":{"ClientIp":"203.0.113.99"}}]'
  sleep 3
done
```

**Результат через 30 сек:**
- 🚨 Алерт в Alertmanager: `brute_force_api`
- 📊 События в ClickHouse: `source_ip=203.0.113.99, count=12`
- 💬 Уведомление в Slack

**Image suggestion:** Terminal window showing the script running, with a Slack notification popup on the right saying " ALERT: Brute-force detected from 203.0.113.99".

---

### Slide 20 — Performance SLA
**Title:** Целевые показатели

| Метрика | SLA | Текущее |
|---------|-----|---------|
| Парсинг p99 | <5ms | ~2ms |
| Алерт latency | ≤30 сек | 15-25 сек |
| Доступность | 99.9% | Vector buffers |
| CH query p95 | <1 сек | 200-500ms |

**Ресурсы @ 10k EPS:**
- CPU: ~11 cores
- RAM: ~12 GB
- Disk: ~50 GB/мес
- Network: ~50 Mbps

**Image suggestion:** Bar chart comparing SLA targets (gray bars) vs actual performance (blue bars) — all actual values below SLA targets, showing the system meets its goals.

---

### Slide 21 — Adding a New Log Source
**Title:** Новый источник логов за 15 минут

1. **Vector Agent** (2 мин) — `type: file` source
2. **VRL парсер** (5 мин) — `parse_nginx_log()`
3. **Маршрут** (1 мин) — `route_events`
4. **Hot reload** (2 мин) — `kill -HUP`
5. **Валидация** (1 мин) — `SELECT count()`

```yaml
# VRL пример для nginx
parsed, err = parse_nginx_log(.message, "combined")
if err == null {
  .source_type = "nginx"
  .severity = if .status_code >= 500 { "error" } else { "info" }
}
```

**Image suggestion:** Lego block being added to a pipeline — representing modular, plug-and-play architecture.

---

### Slide 22 — Roadmap
**Title:** План развития: Phase 1-3

**Phase 1: Production hardening** ✅
- Rust-парсер, PII, GeoIP
- ClickHouse схема, 4 Sigma правила

**Phase 2: Масштабирование до 50k EPS**
- ClickHouse кластер (2 реплики)
- Redpanda кластер (3 ноды)
- Tiered storage → MinIO

**Phase 3: Расширение detection**
- 20+ Sigma правил
- ML anomaly detection
- Threat intelligence feeds
- Case management (TheHive)

**Image suggestion:** Road/timeline with 3 milestones marked Phase 1 (green check), Phase 2 (yellow current), Phase 3 (blue future). Each milestone has sub-items as small flags.

---

### Slide 23 — Risks & Mitigation
**Title:** Риски и ограничения

| Риск | Влияние | Митигация |
|------|---------|-----------|
| Single-node ClickHouse | Потеря данных | Phase 2: репликация |
| Redpanda dev-mode | Нет durability | Убрать флаг |
| Mutex LRU cache | Bottleneck | Заменить на moka |
| Redis без auth | Security | Добавить пароль |
| Rust кривая обучения | Сложнее поддержка | Документация |

**Image suggestion:** Risk matrix (2x2) with colored dots — High (red), Medium (yellow), Low (green). Each risk plotted by impact vs probability.

---

### Slide 24 — Comparison
**Title:** Сравнение с аналогами

| Критерий | **SIEM-Lite** | Splunk | ELK | Wazuh |
|----------|--------------|--------|-----|-------|
| Стоимость | Free | $$$$ | Free | Free |
| Парсинг | Rust (<5ms) | SPL | Logstash | Filebeat |
| Ресурсы | ~12 GB | 32+ GB | 16+ GB | 8+ GB |
| Деплой | 5 мин | Недели | Дни | Часы |
| Sigma | ✅ Нативно | ❌ | ❌ | ✅ |
| PII mask | ✅ На горячем | ❌ | ❌ | Частично |

**Image suggestion:** Winner's podium — SIEM-Lite on #1 (gold), others on #2 and #3. Or a radar chart comparing 5 dimensions (cost, speed, resources, features, ease).

---

### Slide 25 — Thank You
**Title:** Спасибо за внимание

**Документация:**
- `README.md` — быстрый старт
- `docs/ARCHITECTURE.md` — архитектура
- `docs/RUNBOOK.md` — операционные процедуры
- `docs/RISKS_AND_ROADMAP.md` — план развития

**Следующие шаги:**
1. `docker compose up -d`
2. Подключить Vector Agent
3. Добавить Sigma правила
4. Настроить нотификации

**Вопросы?**

**Image suggestion:** Clean closing slide with SIEM-Lite logo, links to documentation, and a subtle background pattern (circuit board or network topology).

---

## DESIGN GUIDELINES

**Color palette:**
- Primary: `#2563eb` (electric blue)
- Secondary: `#1e40af` (deep blue)
- Accent: `#3b82f6` (light blue)
- Background: `#0f172a` (dark navy)
- Text: `#e2e8f0` (light gray)
- Warning: `#f59e0b` (amber)
- Danger: `#ef4444` (red)
- Success: `#10b981` (green)

**Typography:**
- Headings: Bold, 32-44px
- Body: Regular, 18-22px
- Code: Monospace (JetBrains Mono or Fira Code), 14-16px

**Image style:**
- Dark backgrounds with subtle grid patterns
- Glowing/neon accents on data elements
- Minimalist icons with consistent stroke width
- Use tech logos where appropriate (Rust, Go, Grafana, etc.)
- Architecture diagrams with colored arrows
- Code blocks on dark backgrounds with syntax highlighting

**Layout rules:**
- Max 6 bullet points per slide
- Max 20 words per bullet
- Code blocks: max 15 lines
- Always include one visual element (icon, image, diagram)

---

## КОНЕЦ ПРОМПТА
