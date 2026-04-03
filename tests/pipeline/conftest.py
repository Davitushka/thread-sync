"""Общие фикстуры: корень репозитория SIEM-Lite."""

from __future__ import annotations

from pathlib import Path

import pytest


@pytest.fixture(scope="session")
def repo_root() -> Path:
    """Каталог siem-lite (родитель tests/)."""
    return Path(__file__).resolve().parents[2]
