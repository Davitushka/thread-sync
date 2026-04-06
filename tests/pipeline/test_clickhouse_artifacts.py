"""Сводные проверки SQL-артефактов ClickHouse без запуска сервера."""

from __future__ import annotations

from pathlib import Path

import pytest


def test_init_sql_defines_events_table(repo_root: Path) -> None:
    p = repo_root / "clickhouse" / "init.sql"
    text = p.read_text(encoding="utf-8")
    assert "CREATE TABLE" in text and "siem.events" in text, (
        "init.sql должен создавать siem.events — иначе Grafana и Kafka-consumer некуда писать."
    )


def test_kafka_ingest_defines_consumer(repo_root: Path) -> None:
    p = repo_root / "clickhouse" / "02-kafka_ingest.sql"
    text = p.read_text(encoding="utf-8")
    assert "ENGINE = Kafka" in text, "02-kafka_ingest.sql: нужна таблица ENGINE = Kafka"
    assert "siem.events_kafka_mv" in text, "Должно быть MV в siem.events"
    assert "siem.events" in text, "MV должна писать в siem.events"


def test_init_sql_defines_threat_intel(repo_root: Path) -> None:
    text = (repo_root / "clickhouse" / "init.sql").read_text(encoding="utf-8")
    assert "siem.threat_intel" in text and "CREATE TABLE" in text, (
        "init.sql должен создавать siem.threat_intel — иначе SOC Workbench и сиды IoC не работают."
    )


def test_seed_sql_includes_threat_intel_and_ioc_checks(repo_root: Path) -> None:
    text = (repo_root / "scripts" / "seed-data" / "seed_test_events.sql").read_text(encoding="utf-8")
    assert "INSERT INTO siem.threat_intel" in text, "seed_test_events.sql: нужен INSERT в siem.threat_intel"
    assert "feed = 'seed'" in text or "feed='seed'" in text, (
        "Сид должен помечать IoC feed='seed' для отличия от продовых данных"
    )
    assert "Events hitting IOC" in text or "пересекающиеся с IoC" in text, (
        "В конце seed должна быть проверка пересечения events × threat_intel"
    )
