use std::time::Instant;

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::Response;
use axum::Json;
use reqwest::Url;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    event_search::EventSearchParams,
    infrastructure::InfrastructureRequest,
    overview::OverviewRequest,
    AppState,
};

#[derive(RustEmbed)]
#[folder = "static"]
struct PortalAsset;

pub async fn health() -> Json<Value> {
    Json(json!({"status": "ok", "service": "siem-portal"}))
}

pub async fn ui_config(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "links": state.cfg.public,
        "suite": {
            "api_base": "/api/v1",
            "modules": ["overview", "infrastructure", "dashboards", "alerts", "detections", "events", "cases", "investigations"]
        }
    }))
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

#[derive(Debug, Deserialize, Default)]
pub struct CasesQuery {
    pub status: Option<String>,
    pub severity: Option<String>,
    pub assignee: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub q: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DashboardRangeQuery {
    pub hours: Option<u16>,
}

pub async fn stack_status(State(state): State<AppState>) -> Json<Value> {
    let client = &state.http;
    let c = &state.cfg;

    let url_cm = format!("{}/health", c.case_management);
    let url_pr = format!("{}/-/healthy", c.prometheus);
    let url_am = format!("{}/-/healthy", c.alertmanager);
    let url_gr = format!("{}/api/health", c.grafana);
    let url_corr = format!("{}/health", c.correlator);

    let t0 = Instant::now();
    let (hc, hp, ha, hg, hcorr) = tokio::join!(
        ping_health(client, &url_cm, c.http_timeout),
        ping_simple(client, &url_pr, c.http_timeout),
        ping_simple(client, &url_am, c.http_timeout),
        ping_simple(client, &url_gr, c.http_timeout),
        ping_health(client, &url_corr, c.http_timeout),
    );

    Json(json!({
        "elapsed_ms": t0.elapsed().as_millis() as u64,
        "components": {
            "case_management": hc,
            "prometheus": hp,
            "alertmanager": ha,
            "grafana": hg,
            "correlator": hcorr,
        }
    }))
}

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

pub async fn proxy_cases(
    State(state): State<AppState>,
    Query(q): Query<CasesQuery>,
) -> Result<Response, StatusCode> {
    let mut url = join_case_management(&state, "/api/v1/cases")?;
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(s) = &q.status {
            pairs.append_pair("status", s);
        }
        if let Some(s) = &q.severity {
            pairs.append_pair("severity", s);
        }
        if let Some(s) = &q.assignee {
            pairs.append_pair("assignee", s);
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

pub async fn proxy_create_case(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let url = join_case_management(&state, "/api/v1/cases")?;
    proxy_json_request(
        &state.http,
        Method::POST,
        url,
        state.cfg.http_timeout,
        forwarded_actor(&headers),
        Some(body),
    )
    .await
}

pub async fn proxy_case_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    let url = join_case_management(&state, &format!("/api/v1/cases/{id}"))?;
    proxy_get_json(&state.http, url, state.cfg.http_timeout).await
}

pub async fn proxy_patch_case(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let url = join_case_management(&state, &format!("/api/v1/cases/{id}"))?;
    proxy_json_request(
        &state.http,
        Method::PATCH,
        url,
        state.cfg.http_timeout,
        forwarded_actor(&headers),
        Some(body),
    )
    .await
}

pub async fn proxy_case_timeline(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let url = join_case_management(&state, &format!("/api/v1/cases/{id}/timeline"))?;
    proxy_json_request(
        &state.http,
        Method::POST,
        url,
        state.cfg.http_timeout,
        forwarded_actor(&headers),
        Some(body),
    )
    .await
}

pub async fn proxy_case_event_link(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let url = join_case_management(&state, &format!("/api/v1/cases/{id}/events"))?;
    proxy_json_request(
        &state.http,
        Method::POST,
        url,
        state.cfg.http_timeout,
        forwarded_actor(&headers),
        Some(body),
    )
    .await
}

pub async fn proxy_case_alert_link(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let url = join_case_management(&state, &format!("/api/v1/cases/{id}/alerts"))?;
    proxy_json_request(
        &state.http,
        Method::POST,
        url,
        state.cfg.http_timeout,
        forwarded_actor(&headers),
        Some(body),
    )
    .await
}

pub async fn proxy_investigate(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    let url = join_case_management(&state, &format!("/api/v1/cases/{id}/investigate"))?;
    proxy_get_json(&state.http, url, state.cfg.http_timeout).await
}

pub async fn proxy_correlator_stats(State(state): State<AppState>) -> Result<Response, StatusCode> {
    let base: Url = state.cfg.correlator.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let url = base
        .join("/api/v1/stats")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    proxy_get_json(&state.http, url, state.cfg.http_timeout).await
}

pub async fn proxy_correlator_rules(State(state): State<AppState>) -> Result<Response, StatusCode> {
    let base: Url = state.cfg.correlator.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let url = base
        .join("/api/v1/rules")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    proxy_get_json(&state.http, url, state.cfg.http_timeout).await
}

pub async fn overview_dashboard(
    State(state): State<AppState>,
    Query(range): Query<DashboardRangeQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .overview
        .dashboard(OverviewRequest::from_query(range.hours), state.cfg.http_timeout)
        .await
        .map(|payload| Json(json!(payload)))
        .map_err(service_error)
}

pub async fn infrastructure_dashboard(
    State(state): State<AppState>,
    Query(range): Query<DashboardRangeQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .infrastructure
        .dashboard(InfrastructureRequest::from_query(range.hours), state.cfg.http_timeout)
        .await
        .map(|payload| Json(json!(payload)))
        .map_err(service_error)
}

pub async fn search_events(
    State(state): State<AppState>,
    Query(params): Query<EventSearchParams>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .event_search
        .search(&params, state.cfg.http_timeout)
        .await
        .map(|payload| Json(json!(payload)))
        .map_err(service_error)
}

pub async fn get_event(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match state
        .event_search
        .get_event(&id, state.cfg.http_timeout)
        .await
        .map_err(service_error)?
    {
        Some(payload) => Ok(Json(json!(payload))),
        None => Err(not_found("event not found")),
    }
}

pub async fn entity_context(
    State(state): State<AppState>,
    Path((kind, value)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .event_search
        .entity_context(&kind, &value, state.cfg.http_timeout)
        .await
        .map(|payload| Json(json!(payload)))
        .map_err(service_error)
}

pub async fn ui_root() -> Response {
    embedded_asset_response("index.html")
}

pub async fn asset_path(Path(path): Path<String>) -> Response {
    embedded_asset_response(&format!("assets/{path}"))
}

pub async fn favicon_noop() -> StatusCode {
    StatusCode::NO_CONTENT
}

pub async fn spa_fallback() -> Response {
    ui_root().await
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

async fn proxy_get_json(
    client: &reqwest::Client,
    url: Url,
    timeout: std::time::Duration,
) -> Result<Response, StatusCode> {
    proxy_json_request(client, Method::GET, url, timeout, None, None).await
}

async fn proxy_json_request(
    client: &reqwest::Client,
    method: Method,
    url: Url,
    timeout: std::time::Duration,
    actor: Option<String>,
    body: Option<Bytes>,
) -> Result<Response, StatusCode> {
    let mut req = client.request(reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap(), url.clone());
    req = req.timeout(timeout);
    if let Some(actor) = actor.filter(|v| !v.trim().is_empty()) {
        req = req.header("X-SOC-Actor", actor);
    }
    if let Some(body) = body {
        req = req.header(reqwest::header::CONTENT_TYPE, "application/json").body(body);
    }
    let r = req.send().await.map_err(|e| {
        tracing::warn!(error = %e, %url, "proxy request failed");
        StatusCode::BAD_GATEWAY
    })?;
    let status = StatusCode::from_u16(r.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = r
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let bytes = r.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok(Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(bytes))
        .unwrap())
}

fn join_case_management(state: &AppState, path: &str) -> Result<Url, StatusCode> {
    let base: Url = state
        .cfg
        .case_management
        .parse()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    base.join(path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn forwarded_actor(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-soc-actor")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

fn service_error(err: anyhow::Error) -> (StatusCode, Json<Value>) {
    tracing::warn!(error = %err, "suite service request failed");
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({
            "error": err.to_string()
        })),
    )
}

fn not_found(message: &str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": message
        })),
    )
}

fn embedded_asset_response(path: &str) -> Response {
    let lookup = if path == "index.html" {
        "index.html"
    } else {
        path.trim_start_matches('/')
    };
    match PortalAsset::get(lookup) {
        Some(file) => {
            let mime = mime_guess::from_path(lookup)
                .first_or_octet_stream()
                .to_string();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "public, max-age=3600")
                .body(Body::from(file.data.to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("asset not found"))
            .unwrap(),
    }
}
