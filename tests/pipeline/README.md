# Контрактные тесты пайплайна

Проверяют, что конфиги и артефакты **согласованы** между собой (Grafana ↔ ClickHouse, сиды ↔ Vector, SQL-файлы), без обязательного запущенного Docker-стека.

## Запуск

```bash
pip install -r tests/requirements.txt
pytest tests/pipeline -v --tb=short
```

`vector validate` вызывается с **`--skip-healthchecks`**, иначе на машине без сети `redpanda:9092` тест падает с ошибкой брокера.

Без Docker (пропустится только `test_vector_config`):

```bash
pytest tests/pipeline -v -k "not vector"
```

Проверка `vector validate` (нужен Docker):

```bash
pytest tests/pipeline/test_vector_config.py -v
```

## Что ловят тесты

| Область | Пример |
|--------|--------|
| Grafana JSON | Панель HTTP status содержит bucket `non-HTTP`, нет `toUInt16OrZero(status_code)` |
| Seed-data | У всех типов логов есть `SourceType`, `Level`, `Message`; NDJSON и заголовок POST |
| ClickHouse | В `init.sql` / `02-kafka_ingest.sql` есть ожидаемые конструкции |
| Vector | В CI: `docker run … validate` для `aggregator.yaml` |

Сообщения в `assert` сформулированы так, чтобы по логу CI было ясно, **что откатилось** и зачем это важно для работы SIEM-Lite.
