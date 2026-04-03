"""
SIEM-Lite Grafana Integration Tests (pytest)

Запуск:
    pytest tests/grafana/test_grafana_integration.py -v --tb=short

Маркеры:
    @pytest.mark.grafana    — требует запущенную Grafana
    @pytest.mark.integration — интеграционный тест (не unit)

Переменные окружения:
    GRAFANA_URL       — URL Grafana (default: http://localhost:3000)
    GRAFANA_USER      — пользователь (default: admin)
    GRAFANA_PASSWORD  — пароль (required)
"""

import os
import time

import pytest
import requests

# ── Fixtures ─────────────────────────────────────────────────────────────────

GRAFANA_URL = os.environ.get("GRAFANA_URL", "http://localhost:3000")
GRAFANA_USER = os.environ.get("GRAFANA_USER", "admin")
GRAFANA_PASSWORD = os.environ.get("GRAFANA_PASSWORD", "")


@pytest.fixture(scope="session")
def grafana_session():
    """HTTP session с авторизацией в Grafana."""
    session = requests.Session()
    session.auth = (GRAFANA_USER, GRAFANA_PASSWORD)
    session.headers.update({"Content-Type": "application/json"})
    return session


@pytest.fixture(scope="session")
def grafana_base_url():
    return GRAFANA_URL.rstrip("/")


# ── Tests ────────────────────────────────────────────────────────────────────


@pytest.mark.grafana
@pytest.mark.integration
def test_grafana_health(grafana_session, grafana_base_url):
    """Grafana API health endpoint отвечает."""
    resp = grafana_session.get(f"{grafana_base_url}/api/health", timeout=10)
    assert resp.status_code == 200, f"Grafana health: HTTP {resp.status_code}"
    data = resp.json()
    assert data.get("commit") is not None, "Health response missing 'commit' field"
    assert data.get("database") == "ok", f"Database status: {data.get('database')}"


@pytest.mark.grafana
@pytest.mark.integration
def test_datasources_exist(grafana_session, grafana_base_url):
    """Все ожидаемые datasource существуют."""
    resp = grafana_session.get(f"{grafana_base_url}/api/datasources", timeout=10)
    assert resp.status_code == 200
    datasources = {ds["uid"]: ds for ds in resp.json()}

    expected = ["clickhouse-siem", "prometheus-siem", "loki-siem", "alertmanager-siem"]
    for uid in expected:
        assert uid in datasources, f"Datasource {uid} not found"


@pytest.mark.grafana
@pytest.mark.integration
def test_clickhouse_datasource_health(grafana_session, grafana_base_url):
    """ClickHouse datasource здоров."""
    resp = grafana_session.post(f"{grafana_base_url}/api/datasources/uid/clickhouse-siem/health", timeout=10)
    assert resp.status_code == 200, f"ClickHouse datasource health: HTTP {resp.status_code}"


@pytest.mark.grafana
@pytest.mark.integration
def test_prometheus_datasource_health(grafana_session, grafana_base_url):
    """Prometheus datasource здоров."""
    resp = grafana_session.post(f"{grafana_base_url}/api/datasources/uid/prometheus-siem/health", timeout=10)
    assert resp.status_code == 200, f"Prometheus datasource health: HTTP {resp.status_code}"


@pytest.mark.grafana
@pytest.mark.integration
def test_dashboards_exist(grafana_session, grafana_base_url):
    """Все ожидаемые дашборды загружены."""
    resp = grafana_session.get(f"{grafana_base_url}/api/dashboards/uids", timeout=10)
    assert resp.status_code == 200
    dashboards = {d["uid"] for d in resp.json()}

    expected = ["siem-overview", "siem-detection", "siem-alerts", "siem-validation", "siem-operations", "siem-infrastructure"]
    for uid in expected:
        assert uid in dashboards, f"Dashboard {uid} not found in Grafana"


@pytest.mark.grafana
@pytest.mark.integration
def test_panel_datasources_valid(grafana_session, grafana_base_url):
    """Все панели ссылаются на существующие datasource."""
    resp = grafana_session.get(f"{grafana_base_url}/api/dashboards/uids", timeout=10)
    all_uids = {d["uid"] for d in resp.json()}

    # Получить список datasource
    ds_resp = grafana_session.get(f"{grafana_base_url}/api/datasources", timeout=10)
    valid_ds_uids = {ds["uid"] for ds in ds_resp.json()}

    issues = []
    for uid in all_uids:
        dash_resp = grafana_session.get(f"{grafana_base_url}/api/dashboards/uid/{uid}", timeout=10)
        if dash_resp.status_code != 200:
            continue
        dashboard = dash_resp.json().get("dashboard", {})
        for panel in dashboard.get("panels", []):
            for target in panel.get("targets", []):
                ds_ref = target.get("datasource", {})
                if isinstance(ds_ref, dict):
                    ds_uid = ds_ref.get("uid", "")
                    if ds_uid and ds_uid not in valid_ds_uids:
                        issues.append(f"Panel '{panel.get('title')}' in {uid}: invalid datasource '{ds_uid}'")

    assert not issues, f"Invalid datasource references:\n" + "\n".join(issues)


@pytest.mark.grafana
@pytest.mark.integration
def test_clickhouse_query_basic(grafana_session, grafana_base_url):
    """Базовый ClickHouse SQL запрос возвращает данные."""
    payload = {
        "queries": [
            {
                "refId": "A",
                "rawSql": "SELECT 1 AS test",
                "format": 1,
                "datasource": {"type": "grafana-clickhouse-datasource", "uid": "clickhouse-siem"},
            }
        ]
    }
    resp = grafana_session.post(f"{grafana_base_url}/api/ds/query", json=payload, timeout=10)
    assert resp.status_code == 200, f"ClickHouse query: HTTP {resp.status_code} — {resp.text[:200]}"
    data = resp.json()
    assert "results" in data, "Query response missing 'results'"


@pytest.mark.grafana
@pytest.mark.integration
def test_prometheus_query_basic(grafana_session, grafana_base_url):
    """Базовый PromQL запрос возвращает данные."""
    resp = grafana_session.get(
        f"{grafana_base_url}/api/datasources/proxy/uid/prometheus-siem/api/v1/query",
        params={"query": "up", "time": str(int(time.time()))},
        timeout=10,
    )
    assert resp.status_code == 200, f"Prometheus query: HTTP {resp.status_code}"
    data = resp.json()
    assert data.get("status") == "success", f"Prometheus query status: {data.get('status')}"


@pytest.mark.grafana
@pytest.mark.integration
def test_service_endpoints():
    """Ключевые HTTP endpoints сервисов отвечают."""
    endpoints = {
        "Grafana": ("http://localhost:3000/api/health", [200]),
        "Prometheus": ("http://localhost:9090/-/healthy", [200]),
        "ClickHouse": ("http://localhost:8123/ping", [200]),
        "Loki": ("http://localhost:3100/ready", [200]),
        "Alertmanager": ("http://localhost:9093/-/healthy", [200]),
    }

    failures = []
    for name, (url, expected_codes) in endpoints.items():
        try:
            resp = requests.get(url, timeout=5)
            if resp.status_code not in expected_codes:
                failures.append(f"{name}: HTTP {resp.status_code} (expected {expected_codes})")
        except requests.RequestException as e:
            failures.append(f"{name}: {e}")

    assert not failures, f"Service endpoints down:\n" + "\n".join(failures)


@pytest.mark.grafana
@pytest.mark.integration
def test_provisioning_files_consistent():
    """Provisioning YAML файлы валидны и содержат ожидаемые datasource."""
    import yaml

    provisioning_dir = os.path.join(os.path.dirname(__file__), "..", "..", "grafana", "provisioning")
    ds_file = os.path.join(provisioning_dir, "datasources.yaml")

    assert os.path.exists(ds_file), f"datasources.yaml not found at {ds_file}"

    with open(ds_file, "r") as f:
        data = yaml.safe_load(f)

    assert "datasources" in data, "datasources.yaml missing 'datasources' key"
    ds_names = {ds["name"] for ds in data["datasources"]}
    assert "ClickHouse" in ds_names, "ClickHouse datasource not in provisioning"
    assert "Prometheus" in ds_names, "Prometheus datasource not in provisioning"
