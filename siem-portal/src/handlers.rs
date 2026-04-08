use std::time::Instant;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::Json;
use reqwest::Url;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

pub async fn health() -> Json<Value> {
    Json(json!({"status": "ok", "service": "siem-portal"}))
}

pub async fn ui_config(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"links": state.cfg.public}))
}

#[derive(Debug, Deserialize)]
pub struct PromInstantParams {
    pub query: String,
    pub time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PromRangeParams {
    pub query: String,
    pub start: String,
    pub end: String,
    pub step: String,
}

/// Parallel health checks against existing services (HTTP only — no new DB).
pub async fn stack_status(State(state): State<AppState>) -> Json<Value> {
    let client = &state.http;
    let c = &state.cfg;

    let url_cm = format!("{}/health", c.case_management);
    let url_pr = format!("{}/-/healthy", c.prometheus);
    let url_am = format!("{}/-/healthy", c.alertmanager);
    let url_gr = format!("{}/api/health", c.grafana);

    let t0 = Instant::now();
    let (hc, hp, ha, hg) = tokio::join!(
        ping_health(client, &url_cm, c.http_timeout),
        ping_simple(client, &url_pr, c.http_timeout),
        ping_simple(client, &url_am, c.http_timeout),
        ping_simple(client, &url_gr, c.http_timeout),
    );

    Json(json!({
        "elapsed_ms": t0.elapsed().as_millis() as u64,
        "components": {
            "case_management": hc,
            "prometheus": hp,
            "alertmanager": ha,
            "grafana": hg,
        }
    }))
}

async fn ping_health(client: &reqwest::Client, url: &str, timeout: std::time::Duration) -> Value {
    let start = Instant::now();
    match client.get(url).timeout(timeout).send().await {
        Ok(r) if r.status().is_success() => match r.json::<Value>().await {
            Ok(j) => json!({"ok": true, "latency_ms": start.elapsed().as_millis() as u64, "detail": j}),
            Err(_) => json!({"ok": true, "latency_ms": start.elapsed().as_millis() as u64, "detail": "non-json body"}),
        },
        Ok(r) => json!({"ok": false, "latency_ms": start.elapsed().as_millis() as u64, "status": r.status().as_u16()}),
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    }
}

async fn ping_simple(client: &reqwest::Client, url: &str, timeout: std::time::Duration) -> Value {
    let start = Instant::now();
    match client.get(url).timeout(timeout).send().await {
        Ok(r) if r.status().is_success() => {
            json!({"ok": true, "latency_ms": start.elapsed().as_millis() as u64})
        }
        Ok(r) => json!({"ok": false, "latency_ms": start.elapsed().as_millis() as u64, "status": r.status().as_u16()}),
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    }
}

/// Proxy: Prometheus GET /api/v1/query — https://prometheus.io/docs/prometheus/latest/querying/api/
pub async fn proxy_prometheus_query(
    State(state): State<AppState>,
    Query(q): Query<PromInstantParams>,
) -> Result<Response, StatusCode> {
    let base: Url = state.cfg.prometheus.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut url = base.join("/api/v1/query").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    url.query_pairs_mut().append_pair("query", &q.query);
    if let Some(t) = &q.time {
        url.query_pairs_mut().append_pair("time", t);
    }
    proxy_get_json(&state.http, url, state.cfg.http_timeout).await
}

pub async fn proxy_prometheus_query_range(
    State(state): State<AppState>,
    Query(q): Query<PromRangeParams>,
) -> Result<Response, StatusCode> {
    let base: Url = state.cfg.prometheus.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut url = base
        .join("/api/v1/query_range")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    url.query_pairs_mut()
        .append_pair("query", &q.query)
        .append_pair("start", &q.start)
        .append_pair("end", &q.end)
        .append_pair("step", &q.step);
    proxy_get_json(&state.http, url, state.cfg.http_timeout).await
}

/// Proxy: Alertmanager GET /api/v2/alerts — https://prometheus.io/docs/alerting/latest/clients/
pub async fn proxy_alertmanager_alerts(State(state): State<AppState>) -> Result<Response, StatusCode> {
    let url = format!("{}/api/v2/alerts", state.cfg.alertmanager);
    let u: Url = url.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    proxy_get_json(&state.http, u, state.cfg.http_timeout).await
}

pub async fn proxy_alertmanager_status(State(state): State<AppState>) -> Result<Response, StatusCode> {
    let url = format!("{}/api/v2/status", state.cfg.alertmanager);
    let u: Url = url.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    proxy_get_json(&state.http, u, state.cfg.http_timeout).await
}

#[derive(Debug, Deserialize, Default)]
pub struct CasesQuery {
    pub status: Option<String>,
    pub severity: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub q: Option<String>,
}

/// Proxy: case-management-rs GET /api/v1/cases
pub async fn proxy_cases(
    State(state): State<AppState>,
    Query(q): Query<CasesQuery>,
) -> Result<Response, StatusCode> {
    let base: Url = state
        .cfg
        .case_management
        .parse()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut url = base.join("/api/v1/cases").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(s) = &q.status {
            pairs.append_pair("status", s);
        }
        if let Some(s) = &q.severity {
            pairs.append_pair("severity", s);
        }
        if let Some(l) = q.limit {
            pairs.append_pair("limit", &l.to_string());
        }
        if let Some(o) = q.offset {
            pairs.append_pair("offset", &o.to_string());
        }
        if let Some(s) = &q.q {
            pairs.append_pair("q", s);
        }
    }
    proxy_get_json(&state.http, url, state.cfg.http_timeout).await
}

/// Proxy: case-management-rs GET /api/v1/cases/:id/investigate
pub async fn proxy_investigate(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    let url = format!(
        "{}/api/v1/cases/{}/investigate",
        state.cfg.case_management, id
    );
    let u: Url = url.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    proxy_get_json(&state.http, u, state.cfg.http_timeout).await
}

async fn proxy_get_json(
    client: &reqwest::Client,
    url: Url,
    timeout: std::time::Duration,
) -> Result<Response, StatusCode> {
    let r = client
        .get(url.clone())
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, %url, "proxy request failed");
            StatusCode::BAD_GATEWAY
        })?;
    let status = StatusCode::from_u16(r.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let bytes = r.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok(Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn ui_root() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(include_str!("../static/index.html")))
        .unwrap()
}

/// Avoid 404 for favicon in logs.
pub async fn favicon_noop() -> StatusCode {
    StatusCode::NO_CONTENT
}
