"""
Threat intelligence connector: pull IoC from MISP and/or HTTP JSON feed,
upsert into siem.threat_intel, optionally mirror IPv4 into Redis sets for siem-parser.
"""

from __future__ import annotations

import json
import logging
import os
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import clickhouse_connect
import httpx

LOG = logging.getLogger("intel-connector")


@dataclass(frozen=True)
class Ioc:
    ioc_type: str
    ioc_value: str
    threat_label: str
    tags: list[str]
    confidence: int


def _read_secret(path: str | None) -> str:
    if not path:
        return ""
    p = Path(path)
    if not p.is_file():
        return ""
    return p.read_text(encoding="utf-8").strip()


def _env_bool(name: str, default: bool = False) -> bool:
    v = os.environ.get(name, "").strip().lower()
    if not v:
        return default
    return v in ("1", "true", "yes", "on")


def _clickhouse_password() -> str:
    pw = os.environ.get("CLICKHOUSE_PASSWORD", "").strip()
    if pw:
        return pw
    return _read_secret(os.environ.get("CLICKHOUSE_PASSWORD_FILE", ""))


def _ch_client():
    host = os.environ.get("CLICKHOUSE_HOST", "clickhouse").strip()
    port = int(os.environ.get("CLICKHOUSE_PORT", "8123"))
    user = os.environ.get("CLICKHOUSE_USER", "siem").strip()
    password = _clickhouse_password()
    database = os.environ.get("CLICKHOUSE_DATABASE", "siem").strip()
    if not password:
        LOG.warning("CLICKHOUSE_PASSWORD empty — check env or CLICKHOUSE_PASSWORD_FILE")
    return clickhouse_connect.get_client(
        host=host, port=port, username=user, password=password, database=database
    )


def _normalize_ioc_type(misp_type: str) -> str | None:
    t = (misp_type or "").lower().strip()
    if t in ("ip-src", "ip-dst"):
        return "ipv4"
    if t in ("domain", "domain|ip", "hostname", "url"):
        return "domain"
    if t == "sha256":
        return "sha256"
    if t == "ipv6":
        return "ipv6"
    return None


def _normalize_value(ioc_type: str, value: str) -> str | None:
    v = (value or "").strip()
    if not v:
        return None
    if ioc_type == "sha256":
        v = v.lower()
        if len(v) != 64 or any(c not in "0123456789abcdef" for c in v):
            return None
        return v
    if ioc_type == "ipv4":
        parts = v.split(".")
        if len(parts) != 4:
            return None
        try:
            nums = [int(p) for p in parts]
        except ValueError:
            return None
        if not all(0 <= n <= 255 for n in nums):
            return None
        return v
    if ioc_type == "domain":
        v = v.lower().rstrip(".")
        if not v or " " in v:
            return None
        return v
    if ioc_type == "ipv6":
        return v.lower()
    return v


def fetch_misp_iocs(
    base_url: str, api_key: str, verify_ssl: bool, limit: int
) -> list[Ioc]:
    base = base_url.rstrip("/")
    url = f"{base}/attributes/restSearch"
    headers = {
        "Authorization": api_key,
        "Content-Type": "application/json",
        "Accept": "application/json",
    }
    body: dict[str, Any] = {
        "returnFormat": "json",
        "limit": limit,
        "type": [
            "ip-src",
            "ip-dst",
            "domain",
            "hostname",
            "domain|ip",
            "sha256",
            "ipv6",
        ],
    }
    out: list[Ioc] = []
    transport = httpx.Transport(retries=3)
    with httpx.Client(timeout=120.0, verify=verify_ssl, transport=transport) as client:
        r = client.post(url, headers=headers, json=body)
        r.raise_for_status()
        data = r.json()
    resp = data.get("response") or data
    attrs = resp.get("Attribute") if isinstance(resp, dict) else None
    if attrs is None:
        return out
    if isinstance(attrs, dict):
        attrs = [attrs]
    for a in attrs:
        if not isinstance(a, dict):
            continue
        raw_type = str(a.get("type", ""))
        it = _normalize_ioc_type(raw_type)
        if not it:
            continue
        val = _normalize_value(it, str(a.get("value", "")))
        if not val:
            continue
        comment = str(a.get("comment") or "").strip()
        tags: list[str] = ["misp"]
        for tag in a.get("Tag") or []:
            if isinstance(tag, dict) and tag.get("name"):
                tags.append(str(tag["name"]))
            elif isinstance(tag, str):
                tags.append(tag)
        conf = a.get("confidence")
        try:
            c = int(conf) if conf is not None else 60
        except (TypeError, ValueError):
            c = 60
        c = max(0, min(100, c))
        label = comment[:512] if comment else f"MISP {raw_type}"
        out.append(
            Ioc(
                ioc_type=it,
                ioc_value=val,
                threat_label=label,
                tags=tags[:32],
                confidence=c,
            )
        )
    return out


def fetch_feed_iocs(feed_url: str, verify_ssl: bool) -> list[Ioc]:
    transport = httpx.Transport(retries=3)
    with httpx.Client(timeout=60.0, verify=verify_ssl, transport=transport) as client:
        r = client.get(feed_url)
        r.raise_for_status()
        data = r.json()
    raw_list: list[Any]
    if isinstance(data, dict) and "iocs" in data:
        raw_list = data["iocs"]
    elif isinstance(data, list):
        raw_list = data
    else:
        raw_list = []
    out: list[Ioc] = []
    for item in raw_list:
        if not isinstance(item, dict):
            continue
        it = str(item.get("ioc_type", "")).lower().strip()
        if it not in ("ipv4", "domain", "sha256", "ipv6"):
            continue
        val = _normalize_value(it, str(item.get("ioc_value", "")))
        if not val:
            continue
        label = str(item.get("threat_label", "") or "feed")[:512]
        tags = item.get("tags") or []
        if not isinstance(tags, list):
            tags = []
        tags_s = [str(t) for t in tags if t][:32]
        conf = item.get("confidence", 50)
        try:
            c = int(conf)
        except (TypeError, ValueError):
            c = 50
        c = max(0, min(100, c))
        out.append(
            Ioc(
                ioc_type=it,
                ioc_value=val,
                threat_label=label,
                tags=tags_s,
                confidence=c,
            )
        )
    return out


def load_local_feed(path: str) -> list[Ioc]:
    p = Path(path)
    if not p.is_file():
        return []
    data = json.loads(p.read_text(encoding="utf-8"))
    if isinstance(data, dict) and "iocs" in data:
        raw_list = data["iocs"]
    elif isinstance(data, list):
        raw_list = data
    else:
        raw_list = []
    out: list[Ioc] = []
    for item in raw_list:
        if not isinstance(item, dict):
            continue
        it = str(item.get("ioc_type", "")).lower().strip()
        if it not in ("ipv4", "domain", "sha256", "ipv6"):
            continue
        val = _normalize_value(it, str(item.get("ioc_value", "")))
        if not val:
            continue
        try:
            c = int(item.get("confidence", 50))
        except (TypeError, ValueError):
            c = 50
        c = max(0, min(100, c))
        tags = [str(t) for t in (item.get("tags") or []) if t][:32] or ["local"]
        out.append(
            Ioc(
                ioc_type=it,
                ioc_value=val,
                threat_label=str(item.get("threat_label", "local"))[:512],
                tags=tags,
                confidence=c,
            )
        )
    return out


def load_existing_first_seen(client, feed: str) -> dict[tuple[str, str, str], datetime]:
    q = """
    SELECT ioc_type, ioc_value, feed, min(first_seen) AS fs
    FROM threat_intel
    WHERE feed = {feed:String}
    GROUP BY ioc_type, ioc_value, feed
    """
    result = client.query(q, parameters={"feed": feed})
    m: dict[tuple[str, str, str], datetime] = {}
    for row in result.result_rows:
        k = (str(row[0]), str(row[1]), str(row[2]))
        m[k] = row[3]
    return m


def upsert_iocs(
    client,
    iocs: list[Ioc],
    feed: str,
    existing: dict[tuple[str, str, str], datetime],
) -> int:
    if not iocs:
        return 0
    now = datetime.now(timezone.utc)
    rows: list[tuple] = []
    for i in iocs:
        key = (i.ioc_type, i.ioc_value, feed)
        first = existing.get(key) or now
        rows.append(
            (
                i.ioc_type,
                i.ioc_value,
                feed,
                i.threat_label,
                i.tags,
                i.confidence,
                first,
                now,
            )
        )
    client.insert(
        "threat_intel",
        rows,
        column_names=[
            "ioc_type",
            "ioc_value",
            "feed",
            "threat_label",
            "tags",
            "confidence",
            "first_seen",
            "last_seen",
        ],
    )
    return len(rows)


_redis_client = None


def _get_redis(redis_url: str):
    """Lazy singleton Redis connection pool — reused across sync cycles."""
    global _redis_client
    if _redis_client is None:
        import redis
        _redis_client = redis.from_url(redis_url, decode_responses=True)
    return _redis_client


def sync_redis(iocs: list[Ioc], redis_url: str) -> None:
    r = _get_redis(redis_url)
    pipe = r.pipeline()
    # Очищаем старые IPv4 для атомарной замены набора (только ключ коннектора)
    key_v4 = os.environ.get("INTEL_REDIS_SET_IPV4", "siem:intel:ipv4")
    key_dom = os.environ.get("INTEL_REDIS_SET_DOMAIN", "siem:intel:domain")
    key_sha = os.environ.get("INTEL_REDIS_SET_SHA256", "siem:intel:sha256")
    pipe.delete(key_v4, key_dom, key_sha)
    for i in iocs:
        if i.ioc_type == "ipv4":
            pipe.sadd(key_v4, i.ioc_value)
        elif i.ioc_type == "domain":
            pipe.sadd(key_dom, i.ioc_value)
        elif i.ioc_type == "sha256":
            pipe.sadd(key_sha, i.ioc_value)
    pipe.execute()
    LOG.info("redis: synced sets %s / %s / %s", key_v4, key_dom, key_sha)


def run_once() -> None:
    misp_url = os.environ.get("INTEL_MISP_URL", "").strip()
    misp_key = os.environ.get("INTEL_MISP_API_KEY", "").strip()
    feed_url = os.environ.get("INTEL_FEED_URL", "").strip()
    http_feed_name = os.environ.get("INTEL_HTTP_FEED_NAME", "http_feed").strip() or "http_feed"
    local_path = os.environ.get("INTEL_LOCAL_FEED_PATH", "").strip()
    verify = not _env_bool("INTEL_INSECURE_SKIP_VERIFY", False)
    limit = int(os.environ.get("INTEL_MISP_LIMIT", "5000"))

    batches: list[tuple[str, list[Ioc]]] = []

    if misp_url and misp_key:
        LOG.info("fetching MISP %s", misp_url)
        batch = fetch_misp_iocs(misp_url, misp_key, verify, limit)
        batches.append(("misp", batch))
        LOG.info("MISP attributes normalized: %d", len(batch))

    if feed_url:
        LOG.info("fetching feed %s", feed_url)
        batch = fetch_feed_iocs(feed_url, verify)
        batches.append((http_feed_name, batch))
        LOG.info("HTTP feed IoC: %d", len(batch))

    if local_path:
        batch = load_local_feed(local_path)
        batches.append(("local_feed", batch))
        LOG.info("local feed IoC: %d", len(batch))

    if not batches or all(not b for _, b in batches):
        LOG.warning(
            "no IoC loaded — configure INTEL_MISP_URL+INTEL_MISP_API_KEY and/or INTEL_FEED_URL / INTEL_LOCAL_FEED_PATH"
        )
        return

    client = _ch_client()
    all_for_redis: list[Ioc] = []

    for feed_name, batch in batches:
        if not batch:
            continue
        all_for_redis.extend(batch)
        existing = load_existing_first_seen(client, feed_name)
        n = upsert_iocs(client, batch, feed_name, existing)
        LOG.info("clickhouse insert feed=%s rows=%d", feed_name, n)

    if _env_bool("INTEL_SYNC_REDIS") and all_for_redis:
        redis_url = os.environ.get("INTEL_REDIS_URL", "redis://redis:6379/0").strip()
        sync_redis(all_for_redis, redis_url)


def _health_server(port: int) -> None:
    """Minimal health endpoint on /health for k8s liveness probes."""
    from http.server import HTTPServer, BaseHTTPRequestHandler

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path == "/health":
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b"ok")
            else:
                self.send_response(404)
                self.end_headers()

        def log_message(self, *_args):
            pass  # silence request logs

    HTTPServer(("0.0.0.0", port), Handler).serve_forever()


def main() -> None:
    logging.basicConfig(
        level=os.environ.get("LOG_LEVEL", "INFO").upper(),
        format="%(asctime)s %(levelname)s %(message)s",
        stream=sys.stdout,
    )
    interval = int(os.environ.get("INTEL_POLL_INTERVAL_SEC", "3600"))
    run_immediately = _env_bool("INTEL_RUN_ONCE", False)
    health_port = int(os.environ.get("INTEL_HEALTH_PORT", "8080"))

    LOG.info("intel-connector starting poll_interval=%ss run_once=%s health_port=%s", interval, run_immediately, health_port)

    # Start health server in background thread
    import threading
    threading.Thread(target=_health_server, args=(health_port,), daemon=True).start()

    while True:
        try:
            run_once()
        except Exception:
            LOG.exception("sync failed")
        if run_immediately:
            break
        time.sleep(max(60, interval))


if __name__ == "__main__":
    main()
