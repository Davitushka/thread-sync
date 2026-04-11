# SIEM-Lite Alerting Tests

Интеграционные тесты для проверки pipeline: генерация события → **correlator** (образ **detection-engine-rs**, Rust) → алерт в **Alertmanager**. Запуск вручную на поднятом `docker compose`; в едином CI см. job **`grafana-validation`** и скрипты в этом каталоге.

## Предварительные требования

- Запущен полный стек: `docker compose -f deploy/docker/docker-compose.yml up -d`
- `python3` доступен в PATH

## Запуск

```bash
# Все тесты с JUnit отчётом
bash run-all-tests.sh

# Один тест
bash test-brute-force.sh
bash test-sql-injection.sh
bash test-rate-limit.sh
bash test-privilege-escalation.sh

# С другими endpoints
VECTOR_URL=http://localhost:8080/logs \
ALERTMANAGER_URL=http://localhost:9093 \
bash run-all-tests.sh
```

## Переменные окружения

| Переменная | Default | Описание |
|-----------|---------|---------|
| VECTOR_URL | http://localhost:8080/logs | Vector HTTP ingest |
| ALERTMANAGER_URL | http://localhost:9093 | Alertmanager API |
| DETECTION_URL | http://localhost:9110 | Detection Engine metrics |
| MAX_WAIT_SEC | 30 | Таймаут ожидания алерта |

## JUnit отчёт

Отчёт сохраняется в `tests/alerting/results/junit.xml` и совместим с Jenkins, GitLab CI, GitHub Actions.
