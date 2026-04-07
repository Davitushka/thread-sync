#!/usr/bin/env python3
"""
Добавляет в каждый JSON-дашборд в grafana/dashboards панель «Loki: логи контейнеров SIEM»,
если её ещё нет. Идемпотентно (повторный запуск пропускает дашборд).

Запуск из корня репозитория:
  python scripts/add_loki_logs_panels.py
"""

from __future__ import annotations

import json
from pathlib import Path

LOKI_TITLE = "Loki: логи контейнеров SIEM"
LOKI_MARKDOWN_HINT = (
    "\n\n---\n**Логи Docker:** внизу дашборда панель **«"
    + LOKI_TITLE
    + "»** (Promtail → Loki). Фильтр по `container` именам контейнеров."
)


def _grid_bottom(panel: dict) -> int:
    g = panel.get("gridPos") or {}
    return int(g.get("y", 0)) + int(g.get("h", 0))


def _max_panel_id(panels: list) -> int:
    ids = [int(p["id"]) for p in panels if isinstance(p.get("id"), int)]
    return max(ids) if ids else 0


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    dash_dir = root / "grafana" / "dashboards"
    for path in sorted(dash_dir.glob("*.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        panels: list = data.get("panels") or []
        if any(p.get("title") == LOKI_TITLE for p in panels):
            print(f"skip (already has Loki): {path.name}")
            continue

        # Подсказка в первой текстовой панели сверху (один раз)
        for p in panels:
            if p.get("type") != "text":
                continue
            opts = p.setdefault("options", {})
            content = opts.get("content") or ""
            if "**Логи Docker:**" in content:
                break
            opts["content"] = content.rstrip() + LOKI_MARKDOWN_HINT
            break

        bottom = max((_grid_bottom(p) for p in panels), default=0)
        new_id = _max_panel_id(panels) + 1
        log_panel = {
            "id": new_id,
            "title": LOKI_TITLE,
            "description": "Стрим логов ключевых сервисов из Loki (docker_sd → label container).",
            "type": "logs",
            "gridPos": {"x": 0, "y": bottom, "w": 24, "h": 10},
            "datasource": {"type": "loki", "uid": "loki-siem"},
            "targets": [
                {
                    "refId": "A",
                    "datasource": {"type": "loki", "uid": "loki-siem"},
                    "editorMode": "code",
                    "expr": '{container=~"siem-clickhouse|siem-parser|siem-correlator|siem-vector-aggregator|siem-admin|siem-grafana|siem-prometheus|siem-redpanda"}',
                    "queryType": "range",
                }
            ],
            "options": {
                "dedupStrategy": "none",
                "enableLogDetails": True,
                "prettifyLogMessage": False,
                "showCommonLabels": False,
                "showLabels": True,
                "showTime": True,
                "sortOrder": "Descending",
                "wrapLogMessage": True,
            },
        }
        panels.append(log_panel)
        data["panels"] = panels
        path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"updated: {path.name} (panel id={new_id}, y={bottom})")


if __name__ == "__main__":
    main()
