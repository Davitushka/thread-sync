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
