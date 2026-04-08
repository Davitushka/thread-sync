# SIEM-Lite Operator (десктоп)

Нативное окно на **Rust + egui**: список кейсов через API **case-management**. Карточка и рабочий стол расследования по кнопке открываются в **системном браузере**.

## Сборка

Из каталога `siem-operator/`:

```bash
cargo run --release
```

**Windows:** нужны [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (компонент «Desktop development with C++»), иначе не будет `link.exe`.

**Linux:** установите зависимости OpenGL/winit вашего дистрибутива (пакеты вроде `libxcb-render0`, `libwayland-client0` и т.п., см. документацию `winit`/`eframe`).

## Переменные окружения

| Переменная | Смысл |
|------------|--------|
| `SIEM_OPERATOR_API` | Базовый URL case-management (по умолчанию `http://127.0.0.1:8088`) |

Сначала поднимите стек (`docker compose … up`), чтобы API отвечал.
