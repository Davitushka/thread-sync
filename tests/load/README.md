# SIEM-Lite Load Tests

Ручные сценарии нагрузки на парсер/ingest. Общая архитектура и масштаб: [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md), [`docs/RISKS_AND_ROADMAP.md`](../../docs/RISKS_AND_ROADMAP.md).

## Инструменты

- **k6** — основной инструмент нагрузочного тестирования
- **vegeta** — альтернатива для simple HTTP load

## Сценарии

| Сценарий | VUs | EPS | Длительность | SLA p99 |
|----------|-----|-----|-------------|---------|
| 1k       | 10  | 1 000 | 60s | <5ms |
| 10k      | 50  | 10 000 | 120s | <5ms |
| 50k      | 200 | 50 000 | 180s | <5ms |

## Запуск

```bash
# k6 — сценарий 1k EPS
bash run-test.sh --scenario 1k

# k6 — 10k EPS
bash run-test.sh --scenario 10k --tool k6

# vegeta — быстрый smoke test
bash run-test.sh --quick --tool vegeta

# Прямой запуск k6 с JSON output
k6 run --env SCENARIO=10k --out json=results.json k6-script.js
```

## Установка k6

```bash
# Linux/macOS (Homebrew)
brew install k6

# Linux (apt)
sudo gpg -k && sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg \
  --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" \
  | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update && sudo apt-get install k6
```
