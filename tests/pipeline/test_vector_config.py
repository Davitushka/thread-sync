"""
Проверка синтаксиса Vector: `vector validate` в контейнере (как в CI).

На локали без Docker тест пропускается с понятным сообщением.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest


@pytest.mark.skipif(not shutil.which("docker"), reason="Нужен docker для vector validate")
def test_aggregator_yaml_validates(repo_root: Path) -> None:
    cfg = repo_root / "vector" / "aggregator.yaml"
    assert cfg.is_file(), f"Не найден {cfg}"
    proc = subprocess.run(
        [
            "docker",
            "run",
            "--rm",
            "-v",
            f"{repo_root / 'vector'}:/etc/vector:ro",
            "timberio/vector:0.43.0-debian",
            "validate",
            "--skip-healthchecks",
            "--config-yaml",
            "/etc/vector/aggregator.yaml",
        ],
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert proc.returncode == 0, (
        "vector validate (со схемой топологии, без healthcheck к Redpanda) завершился с ошибкой. "
        "На хосте нет DNS redpanda:9092 — без --skip-healthchecks проверка всегда падает вне compose. "
        f"stdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
    )
