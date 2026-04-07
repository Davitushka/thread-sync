# SIEM-Lite Seed Data Generator

Генерирует реалистичные логи 4 типов и отправляет их в Vector HTTP endpoint для тестирования SIEM-Lite.

## ClickHouse: события + алерты + IoC (дашборды не пустые)

Одним скриптом в уже запущенный контейнер `siem-clickhouse`:

```bash
# из корня репозитория (нужен Docker)
bash scripts/seed-data/bootstrap_clickhouse.sh
```

Или через Compose (профиль `seed`):

```bash
docker compose -f deploy/docker/docker-compose.yml --profile seed up soc-seed
```

Выполняется `seed_test_events.sql`: ~1000+ строк в `siem.events`, демо `siem.alerts`, `siem.threat_intel` (feed=`seed`) для **SOC Workbench**.

В файле после маркера `-- END_SEED_DATA` идут только **проверочные SELECT** (для ручного прогона `clickhouse-client < seed_test_events.sql`). **Fill All Data** в SIEM Admin отправляет в ClickHouse только фрагмент **до** этой метки.

Если таблицы `siem.threat_intel` ещё нет (старый volume), сид сам создаёт её (`CREATE TABLE IF NOT EXISTS` в начале файла).

То же самое из UI: **SIEM Admin** (профиль `admin`, порт 8089) → **Fill All Data** (пароль ClickHouse должен совпадать с `CLICKHOUSE_PASSWORD` в compose — в `siem-admin` он проброшен из `.env`).

Связка Grafana (ClickHouse vs Prometheus): [`docs/DATA_PROMETHEUS_GRAFANA.md`](../../docs/DATA_PROMETHEUS_GRAFANA.md).

## Установка

```bash
cd scripts/seed-data
pip install -r requirements.txt
```

## Использование

```bash
# Базовая генерация: 100 EPS в течение 60 секунд
python generate_logs.py

# 1000 EPS, 5 минут, 10% атак
python generate_logs.py --eps 1000 --duration 300 --threat-ratio 0.1

# Только атака brute-force (15 событий)
python generate_logs.py --attack brute_force

# Все атаки последовательно
python generate_logs.py --attack all

# Просмотр без отправки (dry-run)
python generate_logs.py --attack sql_injection --dry-run
```

## Типы событий

| Тип | Вес | Описание |
|-----|-----|---------|
| dotnet | 50% | .NET 9 Serilog JSON логи |
| postgresql | 20% | pg_audit CSV, slow query |
| redis | 15% | slowlog, keyspace events |
| nginx | 15% | access log format |

## Атаки

| Attack | Rule ID | Events | Threshold |
|--------|---------|--------|-----------|
| brute_force | brute_force_api | 15 × 401 на /api/auth/login | 10 |
| sql_injection | sql_injection_attempt | 10 × SQL паттерны | 1 (stateless) |
| rate_limit | rate_limit_evasion | 600 req/min | 500 |
| privilege_escalation | privilege_escalation_attempt | 12 × 403 на /api/admin | 3 |
