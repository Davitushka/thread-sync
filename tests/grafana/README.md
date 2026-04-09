# SIEM-Lite Grafana Validation Suite

Инструмент для проверки работоспособности Grafana и всего SIEM стека.

## Что проверяется

1. **Datasource health** — ClickHouse, Prometheus, Loki, Alertmanager подключены
2. **Dashboards exist** — все 6 дашбордов загружены в Grafana
3. **Panel queries** — SQL/PromQL запросы в панелях валидны и возвращают данные
4. **Service endpoints** — HTTP health endpoints всех сервисов отвечают
5. **Prometheus alerts** — активные алерты в корректном формате
6. **Provisioning** — datasources.yaml и dashboards.yaml корректны

## Установка

```bash
cd tests/grafana
pip install -r requirements.txt
```

## Использование

### Базовый запуск

```bash
python validate_grafana.py --url http://localhost:3000 --user admin --password ClickHousePass123!
```

### С выводом отчёта в JSON

```bash
python validate_grafana.py --url http://localhost:3000 --user admin --password ClickHousePass123! --output report.json
```

### Подробный лог

```bash
python validate_grafana.py --verbose
```

### Пропустить тяжёлые проверки

```bash
# Не выполнять SQL/PromQL запросы панелей
python validate_grafana.py --skip-panel-queries

# Не проверять health datasource
python validate_grafana.py --skip-datasource-health
```

### Все опции

```
--url GRAFANA_URL       URL Grafana (default: http://localhost:3000)
--user GRAFANA_USER     Пользователь Grafana (default: admin)
--password GRAFANA_PASS Пароль Grafana
--output REPORT_FILE    Путь к JSON отчёту
--verbose               Подробный лог
--skip-datasource-health Пропустить проверку datasource health
--skip-panel-queries    Пропустить выполнение запросов панелей
--timeout TIMEOUT       Timeout для HTTP запросов в секундах (default: 10)
```

## Windows .exe запуск

```cmd
cd scripts\grafana-validator
grafana-validator.exe --url http://localhost:3000 --user admin --password ClickHousePass123!
```

## Пример вывода

```
╔══════════════════════════════════════════════════════════════╗
║        SIEM-Lite Grafana Validation Report                  ║
╚══════════════════════════════════════════════════════════════╝

[✓] Grafana API: http://localhost:3000 — OK (5ms)

── Datasources ──────────────────────────────────────────────
[✓] ClickHouse (clickhouse-siem) — OK (12ms)
[✓] Prometheus (prometheus-siem) — OK (8ms)
[✓] Loki (loki-siem) — OK (3ms)
[✓] Alertmanager (alertmanager-siem) — OK (5ms)

── Dashboards ───────────────────────────────────────────────
[✓] siem-overview — 15 panels
[✓] siem-detection — 14 panels
[✓] siem-alerts — 10 panels
[✓] siem-validation — 11 panels
[✓] siem-operations — 19 panels
[✓] siem-infrastructure — 30 panels

── Service Endpoints ────────────────────────────────────────
[✓] Grafana:3000 — 200 (5ms)
[✓] Prometheus:9090 — 200 (3ms)
[✓] ClickHouse:8123 — 200 (2ms)
[✓] Loki:3100 — 200 (4ms)
[✓] Alertmanager:9093 — 200 (3ms)
[✓] Rust Parser:7000 — 200 (8ms)
[✓] Vector:8080 — 200 (6ms)
[✓] Redpanda:9644 — 200 (5ms)

── Summary ──────────────────────────────────────────────────
Total checks: 45  |  Passed: 43  |  Failed: 0  |  Warnings: 2
```

## Troubleshooting

### Grafana не доступна
- Проверьте: `docker compose ps | grep grafana`
- Grafana может загружаться 30-60 сек при первом запуске (установка плагина ClickHouse)

### Datasource не подключается
- Проверьте что ClickHouse/Prometheus запущены: `docker compose ps`
- Проверьте сеть: `docker network inspect siem-lite_siem-internal`

### Панели пустые
- Это нормально если нет данных. Поток событий: убедитесь, что в compose запущен **`siem-log-generator`**, или локально `cargo run --manifest-path log-generator/Cargo.toml` (переменные `SIEM_LOGGEN_*`). Сид таблиц ClickHouse: `bash scripts/seed-data/bootstrap_clickhouse.sh`. Подробнее: [`scripts/seed-data/README.md`](../../scripts/seed-data/README.md).

### PyInstaller .exe не работает
- Убедитесь что Python 3.10+ установлен
- Пересоберите: `cd scripts/grafana-validator && build.bat`
