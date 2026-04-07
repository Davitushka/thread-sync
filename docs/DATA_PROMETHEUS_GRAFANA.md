# Данные в Grafana: ClickHouse vs Prometheus

В проекте **siem-lite** панели используют два независимых источника. Пустой график чаще всего означает «нет серий в **этом** источнике», а не «SIEM сломан».

## Цепочки данных

| Что видите | Источник | Как появляются данные |
|------------|----------|------------------------|
| Таблицы/графики по `siem.events`, `siem.alerts`, MV, `threat_intel` | **Grafana → ClickHouse** | Ingest: Vector → Kafka → consumer → CH; либо SQL-сид `scripts/seed-data/seed_test_events.sql` (bootstrap, **Fill All Data** в SIEM Admin). |
| `siem_events_total`, гистограммы парсера, часть **Alert rules** в Prometheus | **Grafana → Prometheus** | Только **siem-parser**: нормализация события → инкремент счётчиков на `:7000/metrics`. Прямой INSERT в ClickHouse **не** трогает эти метрики. |
| Алерт **SIEMIngestionStopped** | **Prometheus rules** | Срабатывает, если нет трафика и по пути **Vector→Kafka** (`vector_component_sent_events_total`, sink `to_redpanda`), и по **siem-parser** (`siem_parser_events_parsed_total`). |
| `vector_component_*` | **Prometheus** | Логи на `vector-aggregator:8080/logs` → внутренние счётчики Vector (`:9598/metrics`). |
| `detection_events_processed_total` (job `correlator`) | **Prometheus** | В Compose события из Kafka обрабатывает только **correlator** (`:9111/metrics`). |
| `node_*`, `container_*` | **Prometheus** | `node-exporter`, `cAdvisor` (должны быть в `up` в Prometheus). |

## Дашборды (файлы в `grafana/dashboards/`)

| Дашборд | ClickHouse | Prometheus | Заметки |
|---------|------------|------------|---------|
| siem-overview | да | частично | CH — события; Prom — EPS/инфра при наличии scrape. |
| siem-validation | да | да | Таблица проверок CH + панели Vector/parser/detection. |
| siem-operations | мало/нет | да | В основном операционные метрики. |
| siem-detection | да | да | CH-алерты и Prom-метрики детектора. |
| siem-alert-management | да | да | |
| siem-data-quality | да | да | |
| siem-infrastructure | нет | да | Нужны node-exporter + cadvisor. |
| siem-soc-workbench | да | нет | IoC и JOIN с `threat_intel`. |

## Где смотреть «заполненность»

1. **Grafana** → дашборд *SIEM-Lite — проверки компонентов* (`siem-validation`): таблица «Проверки данных (ClickHouse)».
2. **SIEM Admin** (профиль `admin`, порт 8089) → вкладка **Dashboard Checklist**: блок **Prometheus** и блоки по дашбордам; API `GET /api/prometheus-status`, `GET /api/data-status`.
3. **Prometheus UI** → http://localhost:9090 → **Status → Targets** (все `UP`?) и **Graph** для `sum(siem_events_total)` / `sum(siem_parser_events_parsed_total)`.

## Минимальный сценарий «и CH, и Prom не пустые»

1. Поднять стек: `docker compose -f deploy/docker/docker-compose.yml up -d`.
2. Загрузить сид в CH: `bash scripts/seed-data/bootstrap_clickhouse.sh` **или** SIEM Admin → **Fill All Data** (включает SQL + stress + `/parse`).
3. В Grafana выбрать диапазон **Last 24 hours**.
4. Если `siem_events_total` всё ещё 0 — выполнить **Fill All Data** ещё раз или **Critical Surge** (непрерывный `/parse`).

---

## Промпт для ИИ (Cursor и т.п.)

Скопируйте блок ниже, если нужно доработать дашборды или сиды в этом репозитории:

```
Репозиторий siem-lite: Grafana дашборды в grafana/dashboards/*.json (datasource uid clickhouse-siem или prometheus-siem).
Правила алертов Prometheus в alerting/prometheus-rules.yaml используют метрику siem_events_total — она заполняется только siem-parser (rust-parser), не ClickHouse INSERT.
Панели на ClickHouse читают siem.events, siem.alerts, siem.threat_intel, materialized views.
Сид SQL: scripts/seed-data/seed_test_events.sql; выполнение через bootstrap_clickhouse.sh или POST /api/fill-all-data в siem-admin.
Проверки: tests/pipeline/test_grafana_dashboards.py, SIEM Admin /api/prometheus-status и Dashboard Checklist.
Задача: <опишите, что изменить — например «добавить панель PromQL для …» или «исправить пустую панель X на дашборде Y»>.
Не смешивать в одном запросе ожидания CH и siem_events_total без явного пояснения пользователю.
```
