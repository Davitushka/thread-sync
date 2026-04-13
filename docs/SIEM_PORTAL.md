# SIEM Portal (`siem-portal`)

См. [указатель `docs/`](README.md). Единый **web-first Unified Suite** и HTTP API-шлюз на Rust (Axum): агрегирует статус компонентов siem-lite, хостит SPA для аналитика и **проксирует** запросы к уже существующим сервисам. **Новых таблиц БД не создаёт.** detection-engine, alerting-конфиги и case-management **не переписывает** — только вызывает их HTTP API и добавляет безопасный read-only слой поиска событий поверх ClickHouse.

## Запуск

Сервис включён в `deploy/docker/docker-compose.yml` (порт хоста **8091**).

- UI: `http://localhost:8091/` или **`http://127.0.0.1:8091/`** (на Windows иногда `localhost` уходит в IPv6 и страница «не грузится» — тогда только `127.0.0.1`).
- Health: `GET http://localhost:8091/health`

### Страница не открывается

1. **Убедись, что портал запущен.** В логах должно быть `siem-portal listening` и строка с **`http://127.0.0.1:8091/`**. Проверка: `curl http://127.0.0.1:8091/health` → `{"status":"ok",...}`.
2. **Docker:** из корня репозитория подними стек (или хотя бы сервис `siem-portal`). После смены файлов в `siem-portal/web/` или `siem-portal/static/` **пересобери образ** (`docker compose build siem-portal`), иначе в контейнере может быть старый frontend bundle.
3. **Порт занят** — задай другой: `SIEM_PORTAL_ADDR=127.0.0.1:8092` и открой `http://127.0.0.1:8092/`.
4. **Только `http://`, не `https://`** — TLS на портале по умолчанию не поднят.
5. **Локальный `cargo run` без Docker:** адреса вроде `http://case-management:8088` с хоста не резолвятся. Задай upstream на свои порты, например:  
   `SIEM_PORTAL_CASEMGMT_URL=http://127.0.0.1:8088` и остальные `SIEM_PORTAL_*_URL` аналогично (или подними полный compose).

## Переменные окружения

| Переменная | Назначение | По умолчанию (Docker-сеть) |
|------------|------------|----------------------------|
| `SIEM_PORTAL_ADDR` | Listen address | `0.0.0.0:8091` |
| `SIEM_PORTAL_HTTP_TIMEOUT_SEC` | Таймаут upstream HTTP | `10` |
| `SIEM_PORTAL_CASEMGMT_URL` | Базовый URL case-management-rs | `http://case-management:8088` |
| `SIEM_PORTAL_PROMETHEUS_URL` | Базовый URL Prometheus | `http://prometheus:9090` |
| `SIEM_PORTAL_ALERTMANAGER_URL` | Базовый URL Alertmanager | `http://alertmanager:9093` |
| `SIEM_PORTAL_CORRELATOR_URL` | Базовый URL correlator | `http://correlator:9111` |
| `SIEM_PORTAL_GRAFANA_URL` | Базовый URL Grafana (health) | `http://grafana:3000` |
| `SIEM_PORTAL_CLICKHOUSE_URL` | HTTP URL ClickHouse для event search | `http://clickhouse:8123` |
| `SIEM_PORTAL_CLICKHOUSE_USER`, `SIEM_PORTAL_CLICKHOUSE_DATABASE`, `SIEM_PORTAL_CLICKHOUSE_PASSWORD(_FILE)` | Доступ к ClickHouse | `siem`, `siem`, secret/env |
| `SIEM_PORTAL_PUBLIC_*` | URL **для браузера** (кнопки, iframe) | см. compose |

Публичные ссылки (`SIEM_PORTAL_PUBLIC_*`) нужны потому, что браузер пользователя не резолвит Docker DNS (`grafana`, `prometheus`, …).

## API портала (что вызывается у других систем)

### Сводка здоровья

| Метод | Путь | Назначение |
|-------|------|------------|
| `GET` | `/api/v1/stack/status` | Параллельно вызывает: `GET {case-management}/health`, `GET {prometheus}/-/healthy`, `GET {alertmanager}/-/healthy`, `GET {grafana}/api/health` |

### Конфиг для UI

| Метод | Путь | Назначение |
|-------|------|------------|
| `GET` | `/api/v1/ui/config` | JSON с объектом `links` (публичные URL Grafana, Prometheus, Alertmanager, case-management, дашборд overview) и описанием suite-модулей |

### Прокси к Prometheus ([Query API](https://prometheus.io/docs/prometheus/latest/querying/api/))

| Метод | Путь | Upstream |
|-------|------|----------|
| `GET` | `/api/v1/proxy/prometheus/query?query=...&time=...` | `GET {prometheus}/api/v1/query?...` |
| `GET` | `/api/v1/proxy/prometheus/query_range?query=...&start=...&end=...&step=...` | `GET {prometheus}/api/v1/query_range?...` |

**Важно:** прокси открывает произвольные PromQL-запросы — используйте только во **внутренней** сети или за SSO.

### Прокси к Alertmanager ([HTTP API v2](https://prometheus.io/docs/alerting/latest/clients/))

| Метод | Путь | Upstream |
|-------|------|----------|
| `GET` | `/api/v1/proxy/alertmanager/v2/alerts` | `GET {alertmanager}/api/v2/alerts` |
| `GET` | `/api/v1/proxy/alertmanager/v2/status` | `GET {alertmanager}/api/v2/status` |

### Прокси к case-management-rs

| Метод | Путь | Upstream |
|-------|------|----------|
| `GET` | `/api/v1/proxy/cases?status=&severity=&limit=&offset=&q=` | `GET {case-management}/api/v1/cases?...` |
| `POST` | `/api/v1/proxy/cases` | `POST {case-management}/api/v1/cases` |
| `GET` | `/api/v1/proxy/cases/:id` | `GET {case-management}/api/v1/cases/:id` |
| `PATCH` | `/api/v1/proxy/cases/:id` | `PATCH {case-management}/api/v1/cases/:id` |
| `POST` | `/api/v1/proxy/cases/:id/timeline` | `POST {case-management}/api/v1/cases/:id/timeline` |
| `POST` | `/api/v1/proxy/cases/:id/events` | `POST {case-management}/api/v1/cases/:id/events` |
| `POST` | `/api/v1/proxy/cases/:id/alerts` | `POST {case-management}/api/v1/cases/:id/alerts` |
| `GET` | `/api/v1/proxy/cases/:id/investigate` | `GET {case-management}/api/v1/cases/:id/investigate` |

См. исходные маршруты: `case-management-rs/src/main.rs`.

### Прокси к correlator

| Метод | Путь | Upstream |
|-------|------|----------|
| `GET` | `/api/v1/proxy/correlator/stats` | `GET {correlator}/api/v1/stats` |
| `GET` | `/api/v1/proxy/correlator/rules` | `GET {correlator}/api/v1/rules` |

### Native event search

| Метод | Путь | Назначение |
|-------|------|------------|
| `GET` | `/api/v1/events/search?...` | Безопасный read-only поиск событий по `siem.events` |
| `GET` | `/api/v1/events/:id` | Детали одного события |
| `GET` | `/api/v1/entities/:kind/:value/context` | Быстрый контекст по `ip`, `user`, `host` |

Поиск идёт не через raw SQL из браузера, а через whitelisted фильтры в портале.

## Grafana и ClickHouse

- **Дашборды и Explore по ClickHouse** остаются в **Grafana** (как в архитектуре проекта).
- В первой версии Unified Suite есть и **нативный event search** внутри портала, но для глубокого анализа SQL/Explore Grafana остаётся основным deep-dive инструментом.
- На главной странице портала есть **iframe** с дашбордом Overview; если сессия не передалась, откройте ссылку в новой вкладке. Для осознанного встраивания в доверенной среде можно включить `GF_SECURITY_ALLOW_EMBEDDING` в Grafana (см. документацию Grafana).

## Отличие от `siem-admin`

| Компонент | Назначение |
|-----------|------------|
| `siem-portal` | Всегда в основном compose: Unified Suite для аналитика + прокси API + event search. |
| `siem-admin` | Профиль `admin`, Docker socket, сиды — админка стека. |

## Интеграция с `siem-operator`

Desktop-клиент `siem-operator` теперь рассматривается как **гибридная оболочка** над Unified Suite. Портал остаётся главным продуктовым входом и единым API-шлюзом:

- `GET /api/v1/proxy/cases?...` для case KPI/asset risk (через кейсы).
- `GET /api/v1/proxy/alertmanager/v2/alerts` для ленты `Events`.
- `GET /api/v1/proxy/prometheus/query?...` для `Overview` observability KPI.
- `GET /api/v1/events/search?...` для нативного event search в web suite.
- `GET /api/v1/proxy/correlator/stats` / `rules` для detection views.

Рекомендация: для оператора выставлять `SIEM_OPERATOR_API` на адрес `siem-portal`, а для ежедневной работы использовать WebView / browser-режим Unified Suite как основной путь.

### Hybrid SIEM вкладки в Operator

Для нового UX `siem-operator` использует следующие потоки:

- `Detections`: `GET /api/v1/proxy/prometheus/query?query=ALERTS`
- `Alerts`/`Events`: `GET /api/v1/proxy/alertmanager/v2/alerts`
- `Investigations`: `GET {case-management}/api/v1/cases/:id/investigate` (через `SIEM_OPERATOR_API`)
- `Assets`/`Cases`: `GET {case-management}/api/v1/cases?...`
- `StackControl`: локальные `docker compose` команды из `deploy/docker`

Контекст triage->investigation->case сохраняется в persisted state (`selected_investigation_entity`), чтобы оператор мог продолжить расследование после перезапуска UI.
