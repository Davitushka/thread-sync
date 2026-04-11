# SIEM-Lite Operator (десктоп)

Десктоп-клиент SOC на **Rust (egui)**. Рекомендуемый upstream — **[siem-portal](http://localhost:8091)** (см. [`docs/SIEM_PORTAL.md`](../docs/SIEM_PORTAL.md)). В **GitHub Actions** этот крейт отдельным job не собирается — перед PR полезно локально: `cargo fmt`, `cargo clippy`, `cargo test` в каталоге `siem-operator/`.

## Запуск (рекомендуется — всегда должен подниматься)

Нативный клиент на **egui** (все вкладки, API):

```bash
cd siem-operator
cargo run --release
```

Один бинарь **`siem-operator`**: по умолчанию **egui**; режим портала в WebView — флаг **`--web`** (см. ниже).

**Windows:** [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (C++, `link.exe`).

**Если `cargo` пишет `Blocking waiting for file lock`** — закрой второй терминал с `cargo run`/`cargo build` или подожди окончания сборки.

---

## Окно с порталом (WebView → кастомный веб-UI)

Тот же exe: нужен **WebView2** (Windows 10/11 обычно уже есть) и запущенный **siem-portal** на `8091`.

```bash
cargo run --release -- --web
```

Альтернатива без флага: **`SIEM_OPERATOR_MODE=portal`** (или `web` / `webview`).

Переменная **`SIEM_OPERATOR_PORTAL_URL`** — другой URL портала (по умолчанию `http://127.0.0.1:8091/`). Если в браузере не открывается `localhost`, в URL используй **`127.0.0.1`**.

**Linux:** для WebView нужны зависимости **webkit2gtk** (см. документацию `wry`).

---

## Переменные окружения

| Переменная | Смысл |
|------------|--------|
| `SIEM_OPERATOR_API` | Базовый URL для **egui** (по умолчанию **`http://127.0.0.1:8091`** — портал; для прямого case-management — `http://127.0.0.1:8088`) |
| `SIEM_OPERATOR_ALERTMANAGER_URL` | Прямой Alertmanager, если портал не отвечает (по умолчанию тот же хост, что у API, порт **9093**) |
| `SIEM_OPERATOR_PORTAL_URL` | URL портала для режима **`--web`** (WebView) |
| `SIEM_OPERATOR_MODE` | `portal` / `web` / `webview` — как **`--web`** |

Подними стек или хотя бы те сервисы, к которым клиент ходит.
