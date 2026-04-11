"""Проверка наличия сервиса threat intel (Phase 2)."""

from __future__ import annotations

from pathlib import Path


def test_intel_connector_package_layout(repo_root: Path) -> None:
    main_py = repo_root / "intel-connector" / "intel_connector" / "main.py"
    assert main_py.is_file(), "intel-connector: ожидается intel_connector/main.py"
    text = main_py.read_text(encoding="utf-8")
    assert "fetch_misp_iocs" in text and "upsert_iocs" in text, (
        "intel-connector: нужны функции MISP и записи в ClickHouse"
    )
    df = repo_root / "deploy" / "docker" / "Dockerfile.intel"
    assert df.is_file(), "нужен deploy/docker/Dockerfile.intel"
    assert "intel-connector" in df.read_text(encoding="utf-8")
