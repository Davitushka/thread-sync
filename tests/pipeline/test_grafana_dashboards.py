"""
Контракты дашбордов Grafana: валидный JSON, осмысленные SQL, отсутствие известных анти-паттернов.

Сообщения ассертов намеренно явные — при падении CI сразу видно, что починить.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

import pytest


def _load_dashboard(path: Path) -> dict:
    assert path.is_file(), f"Файл дашборда не найден: {path}"
    with path.open(encoding="utf-8") as f:
        return json.load(f)


def _iter_raw_sql_panels(dash: dict):
    for panel in dash.get("panels", []):
        for t in panel.get("targets", []):
            sql = t.get("rawSql")
            if sql:
                yield panel.get("title", "?"), panel.get("id"), sql


def _all_dashboard_files(repo_root: Path) -> list[str]:
    d = repo_root / "grafana" / "dashboards"
    return sorted(p.name for p in d.glob("*.json"))


def test_all_dashboard_json_files_load(repo_root: Path) -> None:
    names = _all_dashboard_files(repo_root)
    assert len(names) == 18, f"Ожидается 18 дашбордов в grafana/dashboards, найдено {len(names)}"
    for name in names:
        path = repo_root / "grafana" / "dashboards" / name
        data = _load_dashboard(path)
        assert data.get("title"), f"{name}: у дашборда должно быть поле title"
        assert data.get("panels"), f"{name}: список panels не должен быть пустым"


def test_siem_overview_has_http_status_panel_with_non_http_bucket(repo_root: Path) -> None:
    """Панель HTTP Status не должна отфильтровывать все строки (раньше было пусто в UI)."""
    path = repo_root / "grafana" / "dashboards" / "siem-overview.json"
    dash = _load_dashboard(path)
    titles_sql = {title: sql for title, _pid, sql in _iter_raw_sql_panels(dash)}
    http_sql = next((sql for t, sql in titles_sql.items() if "HTTP Status" in t), None)
    assert http_sql, "Должна быть панель с «HTTP Status» в названии"
    assert "non-HTTP" in http_sql or "non_http" in http_sql.lower(), (
        "В SQL панели HTTP статусов должен быть bucket «non-HTTP / N/A», иначе график снова станет пустым "
        "при отсутствии кодов у PG/Redis."
    )
    assert "toUInt16OrZero(status_code)" not in http_sql, (
        "toUInt16OrZero(status_code) для Nullable(UInt16) ломает запрос в ClickHouse 24 — не возвращайте этот паттерн."
    )


def test_siem_overview_error_rate_uses_float_division(repo_root: Path) -> None:
    path = repo_root / "grafana" / "dashboards" / "siem-overview.json"
    dash = _load_dashboard(path)
    for title, _pid, sql in _iter_raw_sql_panels(dash):
        if "Error Rate" not in title:
            continue
        assert "toFloat64" in sql or "toFloat32" in sql, (
            f"Панель «{title}»: доля ошибок должна считаться через float (toFloat64/toFloat32), "
            "иначе целочисленное деление в ClickHouse даёт 0%."
        )
        assert (
            "nullIf(count()" in sql
            or "if(count()" in sql.lower()
            or "nullIf(countMerge(" in sql
        ), (
            f"Панель «{title}»: защититесь от деления на ноль (nullIf(count(),0) или nullIf(countMerge(...),0))."
        )
        return
    pytest.fail("Не найдена панель Error Rate с rawSql")


def test_no_broken_ch_sql_patterns(repo_root: Path) -> None:
    """Сканируем rawSql дашбордов на известные ошибки CH."""
    bad = re.compile(r"toUInt16OrZero\s*\(\s*status_code\s*\)", re.IGNORECASE)
    for fname in _all_dashboard_files(repo_root):
        dash = _load_dashboard(repo_root / "grafana" / "dashboards" / fname)
        for title, pid, sql in _iter_raw_sql_panels(dash):
            assert not bad.search(sql), (
                f"{fname} panel id={pid} «{title}»: запрещён toUInt16OrZero(status_code) — "
                "в ClickHouse 24 это ILLEGAL_TYPE для Nullable(UInt16). "
                "Сравнивайте status_code IS NULL OR status_code = 0."
            )


def test_siem_operations_links_and_uid(repo_root: Path) -> None:
    dash = _load_dashboard(repo_root / "grafana" / "dashboards" / "siem-operations.json")
    assert dash.get("uid") == "siem-operations"
    links = dash.get("links") or []
    assert any("/d/siem-overview" in str(L.get("url", "")) for L in links), (
        "siem-operations: нужна ссылка на Overview"
    )
    assert any("/d/siem-validation" in str(L.get("url", "")) for L in links), (
        "siem-operations: нужна ссылка на проверки"
    )
    prom_panels = [
        p for p in dash.get("panels", []) if p.get("datasource", {}).get("uid") == "prometheus-siem"
    ]
    assert len(prom_panels) >= 5, "siem-operations: ожидается несколько Prometheus-панелей"


def test_validation_dashboard_has_instruction_text(repo_root: Path) -> None:
    dash = _load_dashboard(repo_root / "grafana" / "dashboards" / "siem-validation.json")
    text_panels = [p for p in dash.get("panels", []) if p.get("type") == "text"]
    assert text_panels, "siem-validation: нужна текстовая панель с инструкцией прогона"
    content = text_panels[0].get("options", {}).get("content", "")
    assert "seed-data" in content.lower() or "scripts/seed-data" in content, (
        "Инструкция на дашборде проверок должна упоминать seed-data, иначе новым людям непонятно, как нагрузить стек."
    )
