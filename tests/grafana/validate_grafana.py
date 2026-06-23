#!/usr/bin/env python3
"""
SIEM-Lite Grafana Validation Suite

Проверяет работоспособность Grafana, datasource подключения,
дашборды, SQL/PromQL запросы и HTTP endpoints всех сервисов.

Usage:
    python validate_grafana.py --url http://localhost:3000 --user admin --password changeme
    python validate_grafana.py --output report.json
    python validate_grafana.py --verbose --skip-panel-queries
"""

import argparse
import base64
import json
import logging
import sys
import time
from datetime import datetime, timezone
from typing import Any, Optional

import requests
import yaml
from colorama import Fore, Style, init as colorama_init
from tabulate import tabulate

# ── Colorama init (Windows support) ──────────────────────────────────────────
colorama_init(autoreset=True)

# ── Logging ──────────────────────────────────────────────────────────────────
logger = logging.getLogger("siem-validator")


def setup_logging(verbose: bool = False) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    handler = logging.StreamHandler(sys.stdout)
    handler.setFormatter(logging.Formatter("%(levelname)-8s %(message)s"))
    logger.setLevel(level)
    logger.addHandler(handler)


# ── Symbols ──────────────────────────────────────────────────────────────────
OK = f"{Fore.GREEN}✓{Style.RESET_ALL}"
FAIL = f"{Fore.RED}✗{Style.RESET_ALL}"
WARN = f"{Fore.YELLOW}⚠{Style.RESET_ALL}"
INFO = f"{Fore.CYAN}ℹ{Style.RESET_ALL}"


# ── Result collector ─────────────────────────────────────────────────────────
class ValidationResult:
    def __init__(self):
        self.passed = 0
        self.failed = 0
        self.warnings = 0
        self.details: list[dict[str, Any]] = []

    def record(self, category: str, name: str, status: str, detail: str = "", latency_ms: float = 0):
        entry = {"category": category, "name": name, "status": status, "detail": detail, "latency_ms": round(latency_ms, 1)}
        self.details.append(entry)
        if status == "OK":
            self.passed += 1
        elif status == "FAIL":
            self.failed += 1
        else:
            self.warnings += 1

    @property
    def total(self) -> int:
        return self.passed + self.failed + self.warnings

    def to_dict(self) -> dict:
        return {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "summary": {"total_checks": self.total, "passed": self.passed, "failed": self.failed, "warnings": self.warnings},
            "checks": self.details,
        }


# ── HTTP helper with retry ───────────────────────────────────────────────────
def http_get(url: str, auth: tuple | None = None, timeout: int = 10, retries: int = 3) -> requests.Response:
    """GET с retry и замером latency."""
    last_err = None
    for attempt in range(1, retries + 1):
        try:
            start = time.monotonic()
            resp = requests.get(url, auth=auth, timeout=timeout)
            latency = (time.monotonic() - start) * 1000
            resp._latency_ms = latency  # type: ignore[attr-defined]
            return resp
        except requests.RequestException as e:
            last_err = e
            if attempt < retries:
                logger.debug(f"Retry {attempt}/{retries} for {url}: {e}")
                time.sleep(1)
    raise last_err  # type: ignore[misc]


def http_post(url: str, json_data: Any = None, auth: tuple | None = None, timeout: int = 10) -> requests.Response:
    start = time.monotonic()
    resp = requests.post(url, json=json_data, auth=auth, timeout=timeout)
    resp._latency_ms = (time.monotonic() - start) * 1000  # type: ignore[attr-defined]
    return resp


# ── Grafana API client ───────────────────────────────────────────────────────
class GrafanaClient:
    def __init__(self, base_url: str, user: str, password: str, timeout: int = 10):
        self.base_url = base_url.rstrip("/")
        self.auth = (user, password)
        self.timeout = timeout
        self.session = requests.Session()
        self.session.auth = self.auth
        self.session.headers.update({"Content-Type": "application/json"})

    def get(self, path: str, retries: int = 3) -> requests.Response:
        url = f"{self.base_url}{path}"
        last_err = None
        for attempt in range(1, retries + 1):
            try:
                start = time.monotonic()
                resp = self.session.get(url, timeout=self.timeout)
                resp._latency_ms = (time.monotonic() - start) * 1000  # type: ignore[attr-defined]
                return resp
            except requests.RequestException as e:
                last_err = e
                if attempt < retries:
                    time.sleep(1)
        raise last_err  # type: ignore[misc]

    def post(self, path: str, json_data: Any = None) -> requests.Response:
        url = f"{self.base_url}{path}"
        start = time.monotonic()
        resp = self.session.post(url, json=json_data, timeout=self.timeout)
        resp._latency_ms = (time.monotonic() - start) * 1000  # type: ignore[attr-defined]
        return resp


# ── Validation functions ─────────────────────────────────────────────────────

EXPECTED_DASHBOARDS = [
    "siem-overview",
    "siem-detection",
    "siem-alerts",
    "siem-validation",
    "siem-operations",
    "siem-infrastructure",
]

EXPECTED_DATASOURCES = {
    "clickhouse-siem": "ClickHouse",
    "prometheus-siem": "Prometheus",
    "loki-siem": "Loki",
    "alertmanager-siem": "Alertmanager",
}

# Grafana API endpoint для дашбордов (версия 10.x vs 11.x)
DASHBOARD_LIST_ENDPOINTS = ["/api/search?query=&type=dash-db", "/api/dashboards/uids"]

SERVICE_ENDPOINTS = [
    ("Grafana", "http://localhost:3000/api/health"),
    ("Prometheus", "http://localhost:9090/-/healthy"),
    ("ClickHouse", "http://localhost:8123/ping"),
    ("Loki", "http://localhost:3100/ready"),
    ("Alertmanager", "http://localhost:9093/-/healthy"),
    ("Rust Parser", "http://localhost:7000/health"),
    ("Vector Agg", "http://localhost:9598/metrics"),  # Prometheus metrics endpoint
    ("Redpanda", "http://localhost:9644/v1/status/ready"),
]


def check_grafana_api(client: GrafanaClient, result: ValidationResult) -> bool:
    """Проверка что Grafana API доступна."""
    try:
        resp = client.get("/api/health")
        if resp.status_code == 200:
            result.record("Grafana API", "Health endpoint", "OK", f"HTTP {resp.status_code}", resp._latency_ms)
            return True
        else:
            result.record("Grafana API", "Health endpoint", "FAIL", f"HTTP {resp.status_code}", resp._latency_ms)
            return False
    except Exception as e:
        result.record("Grafana API", "Health endpoint", "FAIL", str(e))
        return False


def check_datasources(client: GrafanaClient, result: ValidationResult, skip_health: bool = False) -> dict:
    """Проверка datasource подключений и health."""
    ds_map: dict[str, dict] = {}
    try:
        resp = client.get("/api/datasources")
        if resp.status_code != 200:
            result.record("Datasources", "List datasources", "FAIL", f"HTTP {resp.status_code}")
            return ds_map

        for ds in resp.json():
            ds_map[ds["uid"]] = ds

        for expected_uid, expected_name in EXPECTED_DATASOURCES.items():
            if expected_uid in ds_map:
                ds = ds_map[expected_uid]
                latency = getattr(resp, "_latency_ms", 0)

                if skip_health:
                    result.record("Datasources", f"{expected_name} ({expected_uid})", "OK", "listed", latency)
                else:
                    # Health check
                    try:
                        h_resp = client.post(f"/api/datasources/uid/{expected_uid}/health")
                        if h_resp.status_code == 200:
                            result.record("Datasources", f"{expected_name} ({expected_uid})", "OK", f"healthy ({h_resp._latency_ms:.0f}ms)", latency)
                        else:
                            result.record("Datasources", f"{expected_name} ({expected_uid})", "WARN", f"health={h_resp.status_code}", latency)
                    except Exception as e:
                        result.record("Datasources", f"{expected_name} ({expected_uid})", "WARN", f"health check failed: {e}", latency)
            else:
                result.record("Datasources", f"{expected_name} ({expected_uid})", "FAIL", "not found")

    except Exception as e:
        result.record("Datasources", "List datasources", "FAIL", str(e))

    return ds_map


def check_dashboards(client: GrafanaClient, result: ValidationResult) -> list[dict]:
    """Проверка что все ожидаемые дашборды существуют."""
    dashboards = []
    resp = None

    # Пробуем разные endpoints для разных версий Grafana
    for endpoint in DASHBOARD_LIST_ENDPOINTS:
        try:
            resp = client.get(endpoint)
            if resp.status_code == 200:
                break
        except Exception:
            continue

    if resp is None or resp.status_code != 200:
        result.record("Dashboards", "List dashboards", "FAIL", f"HTTP {resp.status_code if resp else 'no response'}")
        return dashboards

    # Разные форматы ответа для разных endpoints
    raw_data = resp.json()
    if isinstance(raw_data, list) and raw_data and "uid" in raw_data[0]:
        # /api/search format: [{"uid": "...", "title": "...", "type": "dash-db"}]
        existing = {d["uid"]: d for d in raw_data if d.get("type") == "dash-db"}
    elif isinstance(raw_data, list):
        # /api/dashboards/uids format: [{"uid": "...", "title": "..."}]
        existing = {d["uid"]: d for d in raw_data}
    else:
        result.record("Dashboards", "List dashboards", "FAIL", "Unexpected response format")
        return dashboards

    for uid in EXPECTED_DASHBOARDS:
        if uid in existing:
            d = existing[uid]
            # Get full dashboard to count panels
            full_resp = client.get(f"/api/dashboards/uid/{uid}")
            panel_count = 0
            if full_resp.status_code == 200:
                dashboard_data = full_resp.json().get("dashboard", {})
                panel_count = len(dashboard_data.get("panels", []))

            result.record("Dashboards", f"{d['title']} ({uid})", "OK", f"{panel_count} panels")
            dashboards.append({"uid": uid, "title": d["title"], "panel_count": panel_count, "data": full_resp.json().get("dashboard", {}) if full_resp.status_code == 200 else {}})
        else:
            result.record("Dashboards", uid, "FAIL", "not found")

    return dashboards


def check_panel_queries(client: GrafanaClient, dashboards: list[dict], ds_map: dict, result: ValidationResult) -> None:
    """Выполнение запросов панелей для проверки что они возвращают данные."""
    for dash in dashboards:
        dashboard_data = dash["data"]
        if not dashboard_data:
            continue

        for panel in dashboard_data.get("panels", []):
            panel_title = panel.get("title", "untitled")
            targets = panel.get("targets", [])
            if not targets:
                continue

            for target in targets:
                ds_uid = ""
                if "datasource" in target and isinstance(target["datasource"], dict):
                    ds_uid = target["datasource"].get("uid", "")
                elif panel.get("datasource") and isinstance(panel.get("datasource"), dict):
                    ds_uid = panel["datasource"].get("uid", "")

                query = ""
                query_type = ""
                if "expr" in target:
                    query = target["expr"]
                    query_type = "PromQL"
                elif "rawSql" in target:
                    query = target["rawSql"]
                    query_type = "ClickHouse SQL"

                if not query or not ds_uid:
                    continue

                panel_name = f"{dash['title']} → {panel_title}"

                if ds_uid == "prometheus-siem" and query_type == "PromQL":
                    try:
                        # Используем requests напрямую с params
                        url = f"{client.base_url}/api/datasources/proxy/uid/{ds_uid}/api/v1/query"
                        start = time.monotonic()
                        resp = client.session.get(url, params={"query": query, "time": str(int(time.time()))}, timeout=client.timeout)
                        resp._latency_ms = (time.monotonic() - start) * 1000  # type: ignore[attr-defined]
                        if resp.status_code == 200:
                            data = resp.json()
                            if data.get("status") == "success":
                                result_count = len(data.get("data", {}).get("result", []))
                                if result_count > 0:
                                    result.record("Panel Queries", panel_name, "OK", f"PromQL → {result_count} results ({resp._latency_ms:.0f}ms)")
                                else:
                                    result.record("Panel Queries", panel_name, "WARN", f"PromQL → empty result (no data yet)")
                            else:
                                result.record("Panel Queries", panel_name, "FAIL", f"PromQL error: {data.get('error', 'unknown')}")
                        else:
                            result.record("Panel Queries", panel_name, "FAIL", f"HTTP {resp.status_code}")
                    except Exception as e:
                        result.record("Panel Queries", panel_name, "FAIL", f"PromQL exception: {e}")

                elif ds_uid == "clickhouse-siem" and query_type == "ClickHouse SQL":
                    try:
                        payload = {
                            "queries": [
                                {
                                    "refId": "A",
                                    "rawSql": query,
                                    "format": 1,
                                    "datasource": {"type": "grafana-clickhouse-datasource", "uid": ds_uid},
                                }
                            ]
                        }
                        resp = client.post("/api/ds/query", json_data=payload)
                        if resp.status_code == 200:
                            data = resp.json()
                            results = data.get("results", {})
                            has_data = any(r.get("frames", []) for r in results.values())
                            if has_data:
                                result.record("Panel Queries", panel_name, "OK", f"SQL → data returned ({resp._latency_ms:.0f}ms)")
                            else:
                                result.record("Panel Queries", panel_name, "WARN", "SQL → empty result (no data yet)")
                        else:
                            result.record("Panel Queries", panel_name, "FAIL", f"SQL HTTP {resp.status_code}: {resp.text[:200]}")
                    except Exception as e:
                        result.record("Panel Queries", panel_name, "FAIL", f"SQL exception: {e}")


def check_service_endpoints(result: ValidationResult, timeout: int = 10) -> None:
    """Проверка HTTP endpoints всех сервисов."""
    for name, url in SERVICE_ENDPOINTS:
        try:
            resp = http_get(url, timeout=timeout)
            # Для /metrics endpoints принимаем любой 2xx
            if "/metrics" in url:
                if 200 <= resp.status_code < 300:
                    result.record("Service Endpoints", f"{name} ({url.split(':')[-1].split('/')[0]})", "OK", f"HTTP {resp.status_code} ({resp._latency_ms:.0f}ms)")
                else:
                    result.record("Service Endpoints", f"{name} ({url.split(':')[-1].split('/')[0]})", "WARN", f"HTTP {resp.status_code}")
            elif resp.status_code in (200, 204):
                result.record("Service Endpoints", f"{name} ({url.split(':')[-1].split('/')[0]})", "OK", f"HTTP {resp.status_code} ({resp._latency_ms:.0f}ms)")
            else:
                result.record("Service Endpoints", f"{name} ({url.split(':')[-1].split('/')[0]})", "WARN", f"HTTP {resp.status_code}")
        except Exception as e:
            result.record("Service Endpoints", f"{name} ({url.split(':')[-1].split('/')[0]})", "FAIL", str(e))


def check_prometheus_alerts(client: GrafanaClient, result: ValidationResult) -> None:
    """Проверка активных Prometheus алертов."""
    try:
        resp = client.get("/api/datasources/proxy/uid/prometheus-siem/api/v1/alerts")
        if resp.status_code == 200:
            data = resp.json()
            if data.get("status") == "success":
                alerts = data.get("data", {}).get("alerts", [])
                firing = [a for a in alerts if a.get("state") == "firing"]
                result.record("Prometheus Alerts", "Firing alerts count", "OK", f"{len(firing)} firing, {len(alerts)} total")
                for alert in firing[:5]:  # Показать первые 5
                    alert_name = alert.get("labels", {}).get("alertname", "unknown")
                    severity = alert.get("labels", {}).get("severity", "unknown")
                    result.record("Prometheus Alerts", f"  → {alert_name}", "OK", f"severity={severity}")
            else:
                result.record("Prometheus Alerts", "Alerts query", "WARN", f"status={data.get('status')}")
        else:
            result.record("Prometheus Alerts", "Alerts query", "FAIL", f"HTTP {resp.status_code}")
    except Exception as e:
        result.record("Prometheus Alerts", "Alerts query", "WARN", str(e))


def check_provisioning_files(result: ValidationResult, base_path: str = "../../grafana/provisioning") -> None:
    """Проверка что provisioning YAML файлы валидны."""
    import os

    for filename in ["datasources.yaml", "dashboards.yaml"]:
        filepath = os.path.join(base_path, filename)
        try:
            with open(filepath, "r") as f:
                data = yaml.safe_load(f)
            if data:
                result.record("Provisioning", filename, "OK", f"valid YAML ({len(str(data))} chars)")
            else:
                result.record("Provisioning", filename, "WARN", "empty file")
        except FileNotFoundError:
            result.record("Provisioning", filename, "WARN", f"not found at {filepath}")
        except Exception as e:
            result.record("Provisioning", filename, "FAIL", str(e))


# ── Console output ────────────────────────────────────────────────────────────
def print_header(title: str) -> None:
    print(f"\n{Fore.CYAN}{'─' * 60}")
    print(f"  {title}")
    print(f"{'─' * 60}{Style.RESET_ALL}")


def print_result_entry(entry: dict[str, Any]) -> None:
    symbol = OK if entry["status"] == "OK" else (FAIL if entry["status"] == "FAIL" else WARN)
    status_color = Fore.GREEN if entry["status"] == "OK" else (Fore.RED if entry["status"] == "FAIL" else Fore.YELLOW)
    detail = f" — {entry['detail']}" if entry["detail"] else ""
    latency = f" ({entry['latency_ms']}ms)" if entry.get("latency_ms", 0) > 0 else ""
    print(f"  {symbol} {Fore.WHITE}{entry['name']}{Style.RESET_ALL}{status_color} [{entry['status']}]{Style.RESET_ALL}{detail}{Style.DIM}{latency}{Style.RESET_ALL}")


def print_summary(result: ValidationResult) -> None:
    print(f"\n{Fore.CYAN}{'═' * 60}")
    print(f"  SUMMARY")
    print(f"{'═' * 60}{Style.RESET_ALL}")

    status_str = Fore.GREEN if result.failed == 0 else Fore.RED
    print(f"  Total checks : {result.total}")
    print(f"  {Fore.GREEN}Passed{Style.RESET_ALL}       : {result.passed}")
    print(f"  {Fore.RED}Failed{Style.RESET_ALL}       : {result.failed}")
    print(f"  {Fore.YELLOW}Warnings{Style.RESET_ALL}     : {result.warnings}")
    print(f"\n  Status : {status_str}{'ALL PASSED ✓' if result.failed == 0 else f'{result.failed} ISSUE(S) ✗'}{Style.RESET_ALL}")
    print(f"{Fore.CYAN}{'═' * 60}{Style.RESET_ALL}\n")


# ── Main ─────────────────────────────────────────────────────────────────────
def main():
    parser = argparse.ArgumentParser(description="SIEM-Lite Grafana Validation Suite")
    parser.add_argument("--url", default="http://localhost:3000", help="Grafana URL (default: http://localhost:3000)")
    parser.add_argument("--user", default="admin", help="Grafana username (default: admin)")
    parser.add_argument("--password", required=True, help="Grafana password")
    parser.add_argument("--output", help="Path to JSON report file")
    parser.add_argument("--verbose", action="store_true", help="Verbose logging")
    parser.add_argument("--skip-datasource-health", action="store_true", help="Skip datasource health checks")
    parser.add_argument("--skip-panel-queries", action="store_true", help="Skip panel query execution")
    parser.add_argument("--timeout", type=int, default=10, help="HTTP request timeout in seconds (default: 10)")
    args = parser.parse_args()

    setup_logging(args.verbose)

    result = ValidationResult()

    print(f"\n{Fore.CYAN}╔{'═' * 58}╗")
    print(f"║{'SIEM-Lite Grafana Validation Report':^58}║")
    print(f"╚{'═' * 58}╝{Style.RESET_ALL}")
    print(f"  URL      : {args.url}")
    print(f"  User     : {args.user}")
    print(f"  Time     : {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")

    # ── Grafana API ──────────────────────────────────────────────────────────
    client = GrafanaClient(args.url, args.user, args.password, args.timeout)

    print_header("Grafana API")
    api_ok = check_grafana_api(client, result)
    if not api_ok:
        print(f"\n{Fore.RED}Grafana API недоступна. Проверьте что Grafana запущена.{Style.RESET_ALL}")
        print_summary(result)
        sys.exit(1)
    print_result_entry(result.details[-1])

    # ── Datasources ──────────────────────────────────────────────────────────
    print_header("Datasources")
    ds_map = check_datasources(client, result, skip_health=args.skip_datasource_health)
    for entry in result.details[-len(EXPECTED_DATASOURCES):]:
        print_result_entry(entry)

    # ── Dashboards ───────────────────────────────────────────────────────────
    print_header("Dashboards")
    dashboards = check_dashboards(client, result)
    for entry in result.details[-len(EXPECTED_DASHBOARDS):]:
        print_result_entry(entry)

    # ── Panel Queries ────────────────────────────────────────────────────────
    if not args.skip_panel_queries:
        print_header("Panel Queries (sample)")
        # Проверяем по 2 запроса из каждого дашборда чтобы не перегружать
        query_count = 0
        for dash in dashboards:
            dashboard_data = dash["data"]
            if not dashboard_data:
                continue
            for panel in dashboard_data.get("panels", [])[:2]:  # 2 панели на дашборд
                targets = panel.get("targets", [])
                if targets:
                    panel_title = panel.get("title", "untitled")
                    for target in targets[:1]:  # 1 query на панель
                        ds_uid = ""
                        if "datasource" in target and isinstance(target["datasource"], dict):
                            ds_uid = target["datasource"].get("uid", "")
                        elif panel.get("datasource") and isinstance(panel.get("datasource"), dict):
                            ds_uid = panel["datasource"].get("uid", "")
                        query = target.get("expr") or target.get("rawSql", "")
                        if query and ds_uid:
                            query_count += 1
                            if query_count > 12:  # Максимум 12 запросов
                                break

        check_panel_queries(client, dashboards, ds_map, result)
        # Показать результаты
        panel_entries = [e for e in result.details if e["category"] == "Panel Queries"]
        for entry in panel_entries:
            print_result_entry(entry)

    # ── Service Endpoints ────────────────────────────────────────────────────
    print_header("Service Endpoints")
    check_service_endpoints(result, args.timeout)
    endpoint_entries = [e for e in result.details if e["category"] == "Service Endpoints"]
    for entry in endpoint_entries:
        print_result_entry(entry)

    # ── Prometheus Alerts ────────────────────────────────────────────────────
    print_header("Prometheus Alerts")
    check_prometheus_alerts(client, result)
    alert_entries = [e for e in result.details if e["category"] == "Prometheus Alerts"]
    for entry in alert_entries:
        print_result_entry(entry)

    # ── Provisioning ─────────────────────────────────────────────────────────
    print_header("Provisioning Files")
    check_provisioning_files(result)
    prov_entries = [e for e in result.details if e["category"] == "Provisioning"]
    for entry in prov_entries:
        print_result_entry(entry)

    # ── Summary ──────────────────────────────────────────────────────────────
    print_summary(result)

    # ── Save report ──────────────────────────────────────────────────────────
    if args.output:
        report_data = result.to_dict()
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(report_data, f, indent=2, ensure_ascii=False)
        print(f"{INFO} Report saved to {Fore.WHITE}{args.output}{Style.RESET_ALL}\n")

    sys.exit(1 if result.failed > 0 else 0)


if __name__ == "__main__":
    main()
