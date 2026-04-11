# Документация SIEM-Lite

Указатель файлов в этом каталоге. Быстрый старт, Docker Compose и эндпоинты — в корневом [`README.md`](../README.md).

| Документ | Содержание |
|----------|------------|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Слои, Mermaid-потоки, гарантии доставки, Rust-компоненты |
| [`STACK.md`](STACK.md) | Таблица стека: версии из Compose, обоснование, ресурсы |
| [`SCHEMA.md`](SCHEMA.md) | Нормализованное событие (ECS-подобное), примеры |
| [`RUNBOOK.md`](RUNBOOK.md) | Операции, проверки пайплайна, расширение источников |
| [`RISKS_AND_ROADMAP.md`](RISKS_AND_ROADMAP.md) | Упрощения vs enterprise SIEM, масштабирование, Phase 1–3 |
| [`Idea.md`](Idea.md) | Позиционирование SIEM-Lite относительно вендорских SIEM |
| [`DATA_PROMETHEUS_GRAFANA.md`](DATA_PROMETHEUS_GRAFANA.md) | Почему панели пустые: ClickHouse vs Prometheus |
| [`INTEL_CONNECTOR.md`](INTEL_CONNECTOR.md) | Threat intel: MISP/фид → ClickHouse + Redis, связка с парсером |
| [`SIEM_PORTAL.md`](SIEM_PORTAL.md) | Веб-портал SOC (`siem-portal`, порт 8091), API-прокси |

См. также: [`deploy/docker/secrets/README.md`](../deploy/docker/secrets/README.md), [`scripts/seed-data/README.md`](../scripts/seed-data/README.md).
