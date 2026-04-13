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
        "overview_dashboard",
        "infrastructure_dashboard",
        "operations_dashboard",
        "data_quality_dashboard",
        "alerts_overview",
        "detections_overview",
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
    for route in ["/infrastructure", "/operations", "/data-quality", "/dashboards", "/alerts", "/detections", "/events", "/cases"]:
        assert route in app_tsx, (
            f"Unified Suite должен включать маршрут {route!r} в основном app shell."
        )


def test_overview_page_uses_native_overview_api(repo_root: Path) -> None:
    overview = (repo_root / "siem-portal" / "web" / "src" / "pages" / "OverviewPage.tsx").read_text(
        encoding="utf-8"
    )
    assert "getOverviewDashboard" in overview, (
        "OverviewPage должен строиться на собственном portal API, а не только на внешних ссылках Grafana."
    )


def test_infrastructure_page_uses_native_infrastructure_api(repo_root: Path) -> None:
    infra = (repo_root / "siem-portal" / "web" / "src" / "pages" / "InfrastructurePage.tsx").read_text(
        encoding="utf-8"
    )
    assert "getInfrastructureDashboard" in infra, (
        "InfrastructurePage должен строиться на собственном portal API, а не только на встраивании Grafana."
    )


def test_operations_page_uses_native_operations_api(repo_root: Path) -> None:
    operations = (repo_root / "siem-portal" / "web" / "src" / "pages" / "OperationsPage.tsx").read_text(
        encoding="utf-8"
    )
    assert "getOperationsDashboard" in operations, (
        "OperationsPage должен строиться на native portal API, а не оставаться только Grafana deep-dive экраном."
    )


def test_data_quality_page_uses_native_data_quality_api(repo_root: Path) -> None:
    quality = (repo_root / "siem-portal" / "web" / "src" / "pages" / "DataQualityPage.tsx").read_text(
        encoding="utf-8"
    )
    assert "getDataQualityDashboard" in quality, (
        "DataQualityPage должен строиться на собственном portal API, а не только на Grafana dashboard."
    )


def test_alerts_page_uses_native_alerts_overview_api(repo_root: Path) -> None:
    alerts = (repo_root / "siem-portal" / "web" / "src" / "pages" / "AlertsPage.tsx").read_text(
        encoding="utf-8"
    )
    assert "getAlertsOverview" in alerts, (
        "AlertsPage должен строиться на aggregated portal API, а не на сыром alert stack без triage summary."
    )


def test_detections_page_uses_native_detections_overview_api(repo_root: Path) -> None:
    detections = (repo_root / "siem-portal" / "web" / "src" / "pages" / "DetectionsPage.tsx").read_text(
        encoding="utf-8"
    )
    assert "getDetectionsOverview" in detections, (
        "DetectionsPage должен строиться на aggregated portal API, а не на разрозненных correlator/prometheus вызовах."
    )
