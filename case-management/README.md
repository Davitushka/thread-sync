# case-management — React UI

Каталог содержит **только фронтенд** (`web/`) для case management. Бэкенд и Docker-образ: **`case-management-rs`** (см. `deploy/docker/Dockerfile.casemgmt`). Исторический Go-сервис из репозитория удалён; вся серверная логика — в **Rust** (`case-management-rs/`).

Сборка UI и API в CI: job **case-management** в `.github/workflows/ci.yml`.
