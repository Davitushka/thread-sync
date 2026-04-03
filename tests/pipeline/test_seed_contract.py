"""
Контракт генератора сидов: структура событий, NDJSON, заголовки HTTP — без живого Vector.

Явные сообщения об ошибках упрощают поддержку.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

# Импорт generate_logs из scripts/seed-data
_SEED_DIR = Path(__file__).resolve().parents[2] / "scripts" / "seed-data"


@pytest.fixture(scope="module")
def seed_module():
    sys.path.insert(0, str(_SEED_DIR))
    import generate_logs as gl  # noqa: PLC0415

    return gl


def test_config_loads_default_yaml(seed_module, repo_root: Path) -> None:
    cfg_path = _SEED_DIR / "config.yaml"
    assert cfg_path.is_file(), f"Ожидался config.yaml: {cfg_path}"
    cfg = seed_module.Config.from_file(str(cfg_path))
    assert cfg.vector_url, "vector_url в config.yaml не должен быть пустым"
    assert "/logs" in cfg.vector_url, (
        "vector_url должен указывать на путь Vector http_server (по проекту это /logs); "
        f"сейчас: {cfg.vector_url!r}"
    )
    w = cfg.source_weights
    assert abs(sum(w.values()) - 1.0) < 0.01, f"source_weights должны суммироваться в ~1.0, получили {w}"


@pytest.mark.parametrize(
    "generator,source_type",
    [
        ("generate_dotnet_log", "dotnet"),
        ("generate_postgresql_log", "postgresql"),
        ("generate_redis_log", "redis"),
        ("generate_nginx_log", "nginx"),
    ],
)
def test_each_source_shape(seed_module, generator: str, source_type: str) -> None:
    """Каждый тип события обязан нести поля, которые нормализует Vector (SourceType, Level, Message)."""
    gen = getattr(seed_module, generator)
    gl = seed_module
    ip_pool = gl.IPPool(["192.168.1.0/24"])
    atk_pool = gl.IPPool(["203.0.113.0/24"])

    if generator in ("generate_postgresql_log", "generate_redis_log"):
        evt = gen(is_threat=False)
    else:
        evt = gen(ip_pool, atk_pool, is_threat=False)

    assert isinstance(evt, dict), f"{generator} должен вернуть dict, получили {type(evt)}"
    assert evt.get("SourceType") == source_type, (
        f"{generator}: ожидали SourceType={source_type!r}, получили {evt.get('SourceType')!r}. "
        "Иначе в Vector неверно выставится source_type после маппинга."
    )
    assert evt.get("Level"), f"{generator}: поле Level обязательно для маппинга severity в Vector"
    assert evt.get("Message"), f"{generator}: пустое Message — в SIEM нечего показывать"
    line = json.dumps(evt, ensure_ascii=False)
    json.loads(line)  # round-trip


def test_ndjson_batch_format(seed_module) -> None:
    gl = seed_module
    ip_pool = gl.IPPool(["10.0.0.0/24"])
    atk = gl.IPPool(["198.51.100.0/24"])
    batch = [gl.generate_dotnet_log(ip_pool, atk, False) for _ in range(3)]
    payload = "\n".join(json.dumps(e) for e in batch)
    lines = payload.split("\n")
    assert len(lines) == 3, "NDJSON: одна JSON-строка на событие, разделитель \\n"
    for i, line in enumerate(lines):
        assert line.strip(), f"Строка {i} не должна быть пустой"
        obj = json.loads(line)
        assert "Message" in obj, f"Строка {i}: ожидался JSON с полем Message"


def test_send_batch_sets_ndjson_headers(seed_module) -> None:
    gl = seed_module
    recorded: list[dict] = []

    class FakeResponse:
        status_code = 200

        def raise_for_status(self) -> None:
            return None

    class FakeClient:
        def post(self, url, content="", headers=None, timeout=10):
            recorded.append({"url": url, "headers": dict(headers or {}), "content": content})
            return FakeResponse()

    client = FakeClient()
    stats = gl.Stats()
    batch = [{"Timestamp": gl._ts(), "Level": "Information", "Message": "ping", "SourceType": "dotnet"}]
    gl.send_batch(client, "http://example.test/logs", batch, stats, timeout=5)  # type: ignore[arg-type]
    assert recorded, "send_batch должен выполнить HTTP POST"
    assert recorded[0]["headers"].get("Content-Type") == "application/x-ndjson", (
        "Vector http_server с framing newline_delimited ожидает тело NDJSON; "
        "заголовок должен быть application/x-ndjson, иначе приём может отличаться от ожиданий."
    )
    assert stats.sent == 1 and stats.errors == 0, "После успешного POST счётчик sent должен увеличиться"
