#!/usr/bin/env python3
"""
SIEM-Lite Seed Data Generator
Генерирует реалистичные логи 4 типов и отправляет их в Vector HTTP endpoint.

Использование:
    python generate_logs.py --eps 100 --duration 60
    python generate_logs.py --eps 1000 --duration 300 --threat-ratio 0.1
    python generate_logs.py --attack brute_force
    python generate_logs.py --attack all
"""

from __future__ import annotations

import json
import logging
import random
import time
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from ipaddress import IPv4Network
from typing import Any

import click
import httpx
import yaml
from faker import Faker
from rich.console import Console
from rich.live import Live
from rich.table import Table

fake = Faker()
console = Console()
logger = logging.getLogger("seed-data")


# ── Конфигурация ──────────────────────────────────────────────────────────────

@dataclass
class Config:
    vector_url: str = "http://localhost:8080/logs"
    batch_size: int = 100
    request_timeout_sec: int = 10
    source_weights: dict[str, float] = field(default_factory=lambda: {
        "dotnet": 0.50, "postgresql": 0.20, "redis": 0.15, "nginx": 0.15,
    })
    error_ratio: float = 0.05
    ip_pools: dict[str, list[str]] = field(default_factory=dict)
    api_endpoints: list[str] = field(default_factory=list)
    attacks: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_file(cls, path: str) -> "Config":
        with open(path) as f:
            data = yaml.safe_load(f)
        return cls(**{k: v for k, v in data.items() if k in cls.__dataclass_fields__})


# ── IP Pool Helper ────────────────────────────────────────────────────────────

class IPPool:
    def __init__(self, cidrs: list[str]) -> None:
        self._networks = [IPv4Network(c) for c in cidrs]
        self._hosts: list[str] = []
        for net in self._networks:
            hosts = list(net.hosts())
            self._hosts.extend(str(h) for h in random.sample(hosts, min(50, len(hosts))))

    def random(self) -> str:
        return random.choice(self._hosts)


# ── Генераторы логов ──────────────────────────────────────────────────────────

DOTNET_LEVELS = ["Verbose", "Debug", "Information", "Warning", "Error", "Fatal"]
SEVERITY_WEIGHTS = [0.01, 0.04, 0.70, 0.15, 0.08, 0.02]

HTTP_METHODS = ["GET", "POST", "PUT", "DELETE", "PATCH"]
HTTP_METHOD_WEIGHTS = [0.55, 0.20, 0.10, 0.10, 0.05]

STATUS_CODES_NORMAL = [200, 200, 200, 201, 204, 301, 304, 400, 404, 422]
STATUS_CODES_THREAT = [401, 401, 403, 403, 429, 500, 503]

PG_COMMANDS = [
    "SELECT",
    "INSERT INTO",
    "UPDATE",
    "DELETE FROM",
    "SELECT COUNT(*) FROM",
]

REDIS_OPS = ["GET", "SET", "DEL", "EXPIRE", "HGET", "HSET", "LPUSH", "LRANGE", "SADD", "SCARD"]


def _ts() -> str:
    return datetime.now(timezone.utc).isoformat()


def generate_dotnet_log(
    ip_pool: IPPool,
    attacker_pool: IPPool,
    is_threat: bool,
    endpoint_override: str | None = None,
    status_override: int | None = None,
    level_override: str | None = None,
) -> dict[str, Any]:
    level = level_override or random.choices(DOTNET_LEVELS, weights=SEVERITY_WEIGHTS)[0]
    if is_threat and not level_override:
        level = random.choice(["Warning", "Error", "Fatal"])

    method = random.choices(HTTP_METHODS, weights=HTTP_METHOD_WEIGHTS)[0]
    endpoint = endpoint_override or random.choice([
        "/api/auth/login", "/api/users", "/api/products",
        "/api/orders", "/api/search", "/hubs/notifications",
    ])
    status_code = status_override or (
        random.choice(STATUS_CODES_THREAT) if is_threat
        else random.choice(STATUS_CODES_NORMAL)
    )
    source_ip = attacker_pool.random() if is_threat else ip_pool.random()
    duration = round(random.lognormvariate(3, 1), 2)  # lognormal ~ realistic response times

    messages = {
        "Information": f"HTTP {method} {endpoint} responded {status_code} in {duration}ms",
        "Warning": f"Slow response: {method} {endpoint} took {duration}ms (threshold: 500ms)",
        "Error": f"Unhandled exception on {method} {endpoint}: {fake.sentence()}",
        "Fatal": f"Critical failure on {method} {endpoint}: service unavailable",
        "Debug": f"Processing request {method} {endpoint} for user {fake.user_name()}",
        "Verbose": f"Middleware pipeline: {method} {endpoint} → handler invoked",
    }

    host = f"api-{random.randint(1, 4):02d}"
    return {
        "Timestamp": _ts(),
        "Level": level,
        "Message": messages.get(level, f"Event on {endpoint}"),
        "SourceType": "dotnet",
        "Host": host,
        "Properties": {
            "ClientIp": source_ip,
            "RequestMethod": method,
            "RequestPath": endpoint,
            "StatusCode": status_code,
            "Elapsed": duration,
            "UserId": fake.uuid4() if random.random() > 0.3 else None,
            "CorrelationId": str(uuid.uuid4()),
            "MachineName": host,
        },
    }


def generate_postgresql_log(is_threat: bool) -> dict[str, Any]:
    command = random.choice(PG_COMMANDS)
    table = random.choice(["users", "orders", "products", "sessions", "audit_logs"])
    duration_ms = round(random.lognormvariate(4, 1.5), 3)
    rows = random.randint(0, 10000)

    if is_threat:
        # SQL injection attempt в query
        injection = random.choice([
            "' OR '1'='1",
            "'; DROP TABLE users;--",
            "UNION SELECT null, username, password FROM users",
            "1 AND SLEEP(5)",
        ])
        msg = f"ERROR: syntax error at or near '{injection}' in query: {command} FROM {table} WHERE id={injection}"
        level = "Error"
    else:
        msg = f"duration: {duration_ms} ms  statement: {command} {table} WHERE id=$1"
        level = "Warning" if duration_ms > 1000 else "Information"

    return {
        "Timestamp": _ts(),
        "Level": level,
        "Message": msg,
        "SourceType": "postgresql",
        "Host": f"db-{random.randint(1, 2):02d}",
        "Properties": {
            "duration_ms": duration_ms,
            "rows_affected": rows,
            "command": command,
            "table": table,
        },
    }


def generate_redis_log(is_threat: bool) -> dict[str, Any]:
    op = random.choice(REDIS_OPS)
    key_pattern = random.choice(["session:", "cache:", "rate:", "user:", "lock:"])
    key = f"{key_pattern}{fake.uuid4()[:8]}"
    latency_us = random.randint(50, 50000)

    if is_threat:
        msg = f"SLOWLOG: {op} {key} took {latency_us}us — possible enumeration"
        level = "Warning"
    else:
        msg = f"{op} {key} — {latency_us}us"
        level = "Debug" if latency_us < 1000 else "Warning"

    return {
        "Timestamp": _ts(),
        "Level": level,
        "Message": msg,
        "SourceType": "redis",
        "Host": f"redis-{random.randint(1, 2):02d}",
        "Properties": {
            "operation": op,
            "key": key,
            "latency_us": latency_us,
        },
    }


def generate_nginx_log(
    ip_pool: IPPool,
    attacker_pool: IPPool,
    is_threat: bool,
) -> dict[str, Any]:
    source_ip = attacker_pool.random() if is_threat else ip_pool.random()
    method = random.choices(HTTP_METHODS, weights=HTTP_METHOD_WEIGHTS)[0]
    path = random.choice([
        "/", "/index.html", "/api/v1/health", "/static/app.js",
        "/.env", "/.git/config", "/admin", "/wp-admin",
    ])
    status = random.choice(STATUS_CODES_THREAT) if is_threat else random.choice(STATUS_CODES_NORMAL)
    bytes_sent = random.randint(200, 50000)
    duration = round(random.uniform(0.001, 2.5), 3)

    user_agents = [
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        "curl/7.88.0",
        "Python-urllib/3.11",
        "sqlmap/1.7.8",
        "Googlebot/2.1 (+http://www.google.com/bot.html)",
    ]
    ua = user_agents[4] if not is_threat else random.choice(user_agents[:4])

    return {
        "Timestamp": _ts(),
        "Level": "Warning" if status >= 400 else "Information",
        "Message": f'{source_ip} - - [{_ts()}] "{method} {path} HTTP/1.1" {status} {bytes_sent}',
        "SourceType": "nginx",
        "Host": f"nginx-{random.randint(1, 2):02d}",
        "Properties": {
            "remote_addr": source_ip,
            "method": method,
            "path": path,
            "status": status,
            "bytes_sent": bytes_sent,
            "request_time": duration,
            "user_agent": ua,
        },
    }


# ── Генератор атак ────────────────────────────────────────────────────────────

def generate_attack_brute_force(attacker_pool: IPPool, config: Config) -> list[dict[str, Any]]:
    """15 запросов с одного IP на /api/auth/login с кодом 401."""
    ip = attacker_pool.random()
    cfg = config.attacks.get("brute_force", {})
    count = cfg.get("requests", 15)
    return [
        generate_dotnet_log(
            ip_pool=IPPool(["10.0.0.1/32"]),
            attacker_pool=IPPool([f"{ip}/32"]),
            is_threat=True,
            endpoint_override=cfg.get("path", "/api/auth/login"),
            status_override=cfg.get("status_code", 401),
            level_override="Warning",
        )
        for _ in range(count)
    ]


def generate_attack_sql_injection(ip_pool: IPPool) -> list[dict[str, Any]]:
    """Попытки SQL injection через PostgreSQL и dotnet endpoint."""
    logs = []
    for _ in range(5):
        logs.append(generate_postgresql_log(is_threat=True))
        logs.append(generate_dotnet_log(
            ip_pool=ip_pool,
            attacker_pool=ip_pool,
            is_threat=True,
            endpoint_override="/api/search",
            status_override=500,
        ))
    return logs


def generate_attack_rate_limit(attacker_pool: IPPool, config: Config) -> list[dict[str, Any]]:
    """600 запросов с одного IP за короткое время."""
    cfg = config.attacks.get("rate_limit", {})
    count = cfg.get("requests", 600)
    ip = attacker_pool.random()
    return [
        generate_dotnet_log(
            ip_pool=IPPool(["10.0.0.1/32"]),
            attacker_pool=IPPool([f"{ip}/32"]),
            is_threat=False,
            endpoint_override="/api/products",
            status_override=200,
        )
        for _ in range(count)
    ]


def generate_attack_privilege_escalation(attacker_pool: IPPool, config: Config) -> list[dict[str, Any]]:
    """Попытки доступа к /api/admin с кодом 403."""
    ip = attacker_pool.random()
    paths = config.attacks.get("privilege_escalation", {}).get(
        "paths", ["/api/admin/users", "/api/admin/config"]
    )
    return [
        generate_dotnet_log(
            ip_pool=IPPool(["10.0.0.1/32"]),
            attacker_pool=IPPool([f"{ip}/32"]),
            is_threat=True,
            endpoint_override=path,
            status_override=403,
            level_override="Warning",
        )
        for path in paths
        for _ in range(4)  # 4 попытки на каждый path
    ]


# ── HTTP отправка ─────────────────────────────────────────────────────────────

@dataclass
class Stats:
    sent: int = 0
    errors: int = 0
    start_time: float = field(default_factory=time.monotonic)

    def elapsed(self) -> float:
        return time.monotonic() - self.start_time

    def eps(self) -> float:
        elapsed = self.elapsed()
        return self.sent / elapsed if elapsed > 0 else 0.0


def send_batch(
    client: httpx.Client,
    url: str,
    batch: list[dict[str, Any]],
    stats: Stats,
    timeout: int = 10,
) -> None:
    payload = "\n".join(json.dumps(e) for e in batch)
    try:
        resp = client.post(
            url,
            content=payload,
            headers={"Content-Type": "application/x-ndjson"},
            timeout=timeout,
        )
        resp.raise_for_status()
        stats.sent += len(batch)
    except httpx.HTTPStatusError as e:
        logger.warning("HTTP error %s for batch of %d", e.response.status_code, len(batch))
        stats.errors += len(batch)
    except httpx.RequestError as e:
        logger.warning("Request error: %s", e)
        stats.errors += len(batch)


# ── CLI ───────────────────────────────────────────────────────────────────────

@click.command()
@click.option("--eps", default=100, type=int, show_default=True, help="Events per second")
@click.option("--duration", default=60, type=int, show_default=True, help="Duration in seconds")
@click.option("--threat-ratio", default=0.05, type=float, show_default=True,
              help="Fraction of events simulating attacks (0.0–1.0)")
@click.option("--attack", type=click.Choice(["brute_force", "sql_injection", "rate_limit",
                                              "privilege_escalation", "all", "none"]),
              default="none", show_default=True, help="Generate specific attack scenario")
@click.option("--config", "config_path", default="config.yaml", type=click.Path(),
              show_default=True, help="Path to config.yaml")
@click.option("--url", default=None, help="Override Vector HTTP endpoint URL")
@click.option("--dry-run", is_flag=True, help="Print logs to stdout instead of sending")
@click.option("--verbose", "-v", is_flag=True, help="Verbose logging")
def main(
    eps: int,
    duration: int,
    threat_ratio: float,
    attack: str,
    config_path: str,
    url: str | None,
    dry_run: bool,
    verbose: bool,
) -> None:
    logging.basicConfig(
        level=logging.DEBUG if verbose else logging.WARNING,
        format="%(asctime)s %(levelname)s %(message)s",
    )

    cfg = Config.from_file(config_path)
    if url:
        cfg.vector_url = url

    normal_pool = IPPool(cfg.ip_pools.get("normal", ["192.168.1.0/24"]))
    attacker_pool = IPPool(cfg.ip_pools.get("attacker", ["203.0.113.0/24"]))

    sources = list(cfg.source_weights.keys())
    weights = list(cfg.source_weights.values())

    # Режим атаки
    if attack != "none":
        attack_batches = {
            "brute_force": lambda: generate_attack_brute_force(attacker_pool, cfg),
            "sql_injection": lambda: generate_attack_sql_injection(normal_pool),
            "rate_limit": lambda: generate_attack_rate_limit(attacker_pool, cfg),
            "privilege_escalation": lambda: generate_attack_privilege_escalation(attacker_pool, cfg),
        }
        attacks_to_run = list(attack_batches.keys()) if attack == "all" else [attack]

        with httpx.Client() as client:
            for atk in attacks_to_run:
                logs = attack_batches[atk]()
                console.print(f"[bold yellow]Sending {len(logs)} events for attack: {atk}[/]")
                if dry_run:
                    for log in logs[:3]:
                        console.print_json(json.dumps(log))
                    console.print(f"  ... ({len(logs)} total, dry-run)")
                else:
                    stats = Stats()
                    for i in range(0, len(logs), cfg.batch_size):
                        send_batch(client, cfg.vector_url, logs[i:i + cfg.batch_size], stats)
                    console.print(f"  [green]✓ Sent {stats.sent} events[/]")
        return

    # Режим непрерывной генерации
    interval = 1.0 / eps if eps > 0 else 0
    total_events = eps * duration
    stats = Stats()

    def _gen_event() -> dict[str, Any]:
        src = random.choices(sources, weights=weights)[0]
        is_threat = random.random() < threat_ratio
        if src == "dotnet":
            return generate_dotnet_log(normal_pool, attacker_pool, is_threat)
        elif src == "postgresql":
            return generate_postgresql_log(is_threat)
        elif src == "redis":
            return generate_redis_log(is_threat)
        else:
            return generate_nginx_log(normal_pool, attacker_pool, is_threat)

    def make_table() -> Table:
        t = Table(title="SIEM-Lite Seed Data Generator")
        t.add_column("Metric", style="cyan")
        t.add_column("Value", style="green")
        t.add_row("Target EPS", str(eps))
        t.add_row("Actual EPS", f"{stats.eps():.1f}")
        t.add_row("Sent", str(stats.sent))
        t.add_row("Errors", str(stats.errors))
        t.add_row("Elapsed", f"{stats.elapsed():.1f}s")
        t.add_row("Progress", f"{stats.sent}/{total_events} ({stats.sent * 100 // max(total_events, 1)}%)")
        return t

    console.print(f"[bold]Generating {total_events:,} events at {eps} EPS for {duration}s[/]")
    console.print(f"[dim]Target: {cfg.vector_url} | threat_ratio={threat_ratio:.1%}[/]")

    if dry_run:
        for _ in range(min(5, total_events)):
            event = _gen_event()
            console.print_json(json.dumps(event))
        console.print(f"[dim]... (dry-run, showed 5 of {total_events})[/]")
        return

    batch: list[dict[str, Any]] = []
    deadline = time.monotonic() + duration

    with httpx.Client() as client:
        with Live(make_table(), refresh_per_second=2, console=console) as live:
            while time.monotonic() < deadline and stats.sent < total_events:
                tick = time.monotonic()
                batch.append(_gen_event())

                if len(batch) >= cfg.batch_size:
                    send_batch(client, cfg.vector_url, batch, stats, cfg.request_timeout_sec)
                    batch.clear()
                    live.update(make_table())

                elapsed_tick = time.monotonic() - tick
                sleep = interval - elapsed_tick
                if sleep > 0:
                    time.sleep(sleep)

            # Flush остатка
            if batch:
                send_batch(client, cfg.vector_url, batch, stats, cfg.request_timeout_sec)

    console.print(f"\n[bold green]Done! Sent {stats.sent:,} events in {stats.elapsed():.1f}s "
                  f"({stats.eps():.1f} EPS avg)[/]")
    if stats.errors:
        console.print(f"[yellow]Errors: {stats.errors}[/]")


if __name__ == "__main__":
    main()
