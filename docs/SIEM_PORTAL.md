# SIEM Portal (`siem-portal`)

Единый **веб-интерфейс и HTTP API-шлюз** на Rust (Axum): агрегирует статус компонентов siem-lite и **проксирует** запросы к уже существующим сервисам. **Новых таблиц БД не создаёт.** detection-engine, alerting-конфиги и case-management **не переписывает** — только вызывает их HTTP API.

## Запуск

Сервис включён в `deploy/docker/docker-compose.yml` (порт хоста **8091**).

- UI: `http://localhost:8091/`
- Health: `GET http://localhost:8091/health`

## Переменные окружения

| Переменная | Назначение | По умолчанию (Docker-сеть) |
|------------|------------|----------------------------|
| `SIEM_PORTAL_ADDR` | Listen address | `0.0.0.0:8091` |
| `SIEM_PORTAL_HTTP_TIMEOUT_SEC` | Таймаут upstream HTTP | `10` |
| `SIEM_PORTAL_CASEMGMT_URL` | Базовый URL case-management-rs | `http://case-management:8088` |
| `SIEM_PORTAL_PROMETHEUS_URL` | Базовый URL Prometheus | `http://prometheus:9090` |
| `SIEM_PORTAL_ALERTMANAGER_URL` | Базовый URL Alertmanager | `http://alertmanager:9093` |
| `SIEM_PORTAL_GRAFANA_URL` | Базовый URL Grafana (health) | `http://grafana:3000` |
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
| `GET` | `/api/v1/ui/config` | JSON с объектом `links` (публичные URL Grafana, Prometheus, Alertmanager, case-management, дашборд overview) |

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
| `GET` | `/api/v1/proxy/cases/:id/investigate` | `GET {case-management}/api/v1/cases/:id/investigate` |

См. исходные маршруты: `case-management-rs/src/main.rs`.

## Grafana и ClickHouse

- **Дашборды и Explore по ClickHouse** остаются в **Grafana** (как в архитектуре проекта).
- На главной странице портала есть **iframe** с дашбордом Overview; если сессия не передалась, откройте ссылку в новой вкладке. Для осознанного встраивания в доверенной среде можно включить `GF_SECURITY_ALLOW_EMBEDDING` в Grafana (см. документацию Grafana).

## Отличие от `siem-admin`

| Компонент | Назначение |
|-----------|------------|
| `siem-portal` | Всегда в основном compose: «операторская консоль» + прокси API. |
| `siem-admin` | Профиль `admin`, Docker socket, сиды — админка стека. |
