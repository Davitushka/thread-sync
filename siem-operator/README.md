# SIEM-Lite Operator (десктоп)

Гибридная десктоп-оболочка над **Unified Suite**, который хостится через **[siem-portal](http://localhost:8091)** (см. [`docs/SIEM_PORTAL.md`](../docs/SIEM_PORTAL.md)). Нативный **egui**-режим остаётся как fallback и для отдельных operator-сценариев. В **GitHub Actions** этот крейт отдельным job не собирается — перед PR полезно локально: `cargo fmt`, `cargo clippy`, `cargo test` в каталоге `siem-operator/`.

## Запуск

Рекомендуемый режим: WebView с единым web-first приложением.

```bash
cd siem-operator
cargo run --release
```

Один бинарь **`siem-operator`**: по умолчанию открывает Unified Suite в WebView; нативный **egui** можно включить флагом **`--native`**.

**Windows:** [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (C++, `link.exe`).

**Если `cargo` пишет `Blocking waiting for file lock`** — закрой второй терминал с `cargo run`/`cargo build` или подожди окончания сборки.

---

## Окно с Unified Suite (WebView)

Тот же exe: нужен **WebView2** (Windows 10/11 обычно уже есть). Если `SIEM_OPERATOR_PORTAL_URL` указывает на локальный `127.0.0.1` / `localhost`, оператор сначала попробует сам поднять **`siem-portal`**, дождётся `GET /health`, а затем откроет Unified Suite внутри окна.

```bash
cargo run --release
```

Альтернатива: **`SIEM_OPERATOR_MODE=portal`** (или `web` / `webview`).

Переменная **`SIEM_OPERATOR_PORTAL_URL`** — другой URL портала / Unified Suite (по умолчанию `http://127.0.0.1:8091/`). Если в браузере не открывается `localhost`, в URL используй **`127.0.0.1`**.

**Linux:** для WebView нужны зависимости **webkit2gtk** (см. документацию `wry`).

---

## Нативный fallback (egui)

```bash
cargo run --release -- --native
```

Или через окружение: **`SIEM_OPERATOR_MODE=native`**.

Этот режим полезен как запасной вариант и для тех operator-фич, которые ещё не перенесены в web suite.

---

## Переменные окружения

| Переменная | Смысл |
|------------|--------|
| `SIEM_OPERATOR_API` | Базовый URL для **egui** (по умолчанию **`http://127.0.0.1:8091`** — портал) |
| `SIEM_OPERATOR_ALERTMANAGER_URL` | Прямой Alertmanager, если портал не отвечает (по умолчанию тот же хост, что у API, порт **9093**) |
| `SIEM_OPERATOR_PORTAL_URL` | URL Unified Suite для режима WebView |
| `SIEM_OPERATOR_AUTOSTART_PORTAL` | Автозапуск локального `siem-portal` (`true` по умолчанию; `false`/`0` выключает) |
| `SIEM_OPERATOR_PORTAL_BIN` | Явный путь к бинарнику `siem-portal`, если нужно не искать его рядом с репозиторием |
| `SIEM_OPERATOR_MODE` | `portal` / `web` / `webview` или `native` |

Подними стек или хотя бы те сервисы, к которым клиент ходит.
