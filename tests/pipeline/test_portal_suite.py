"""Контракты Unified Suite в siem-portal без запуска стека."""

from __future__ import annotations

from pathlib import Path


def test_portal_web_bundle_exists(repo_root: Path) -> None:
    index_html = repo_root / "siem-portal" / "static" / "index.html"
    assert index_html.is_file(), "siem-portal/static/index.html должен существовать после сборки web suite"
    text = index_html.read_text(encoding="utf-8")
    assert "/assets/" in text, (
        "siem-portal/static/index.html должен ссылаться на Vite assets bundle, "
        "иначе portal не хостит новый Unified Suite frontend."
    )


def test_portal_handlers_expose_suite_endpoints(repo_root: Path) -> None:
    handlers = (repo_root / "siem-portal" / "src" / "handlers.rs").read_text(encoding="utf-8")
    required = [
        "search_events",
        "get_event",
        "entity_context",
        "proxy_create_case",
        "proxy_patch_case",
        "proxy_case_timeline",
        "proxy_case_event_link",
        "proxy_case_alert_link",
        "proxy_correlator_stats",
        "proxy_correlator_rules",
    ]
    missing = [name for name in required if name not in handlers]
    assert not missing, (
        "siem-portal handlers должны содержать suite endpoints для analyst-facing приложения. "
        f"Не найдены: {missing}"
    )


def test_portal_web_app_has_core_routes(repo_root: Path) -> None:
    app_tsx = (repo_root / "siem-portal" / "web" / "src" / "App.tsx").read_text(encoding="utf-8")
    for route in ["/dashboards", "/alerts", "/detections", "/events", "/cases"]:
        assert route in app_tsx, (
            f"Unified Suite должен включать маршрут {route!r} в основном app shell."
        )
