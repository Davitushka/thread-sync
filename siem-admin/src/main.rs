use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bollard::container::{
    ListContainersOptions, RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::Docker;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    docker: Arc<Docker>,
    http: reqwest::Client,
    red_alert: Arc<tokio::sync::Mutex<RedAlertRuntime>>,
}

#[derive(Serialize)]
struct ServiceInfo {
    name: String,
    container_id: String,
    state: String,
    status: String,
    health: Option<String>,
    image: String,
    ports: Vec<String>,
    created: i64,
}

#[derive(Serialize)]
struct PipelineCheck {
    name: String,
    ok: bool,
    detail: String,
}

#[derive(Serialize)]
struct PipelineStatus {
    checks: Vec<PipelineCheck>,
    healthy: bool,
}

#[derive(Serialize)]
struct FillAllDataResult {
    ok: bool,
    /// Размер применённого `seed_test_events.sql` (ClickHouse: events, alerts, threat_intel).
    soc_seed_bytes: usize,
    stress_action: String,
    seeded_alerts: usize,
    details: Vec<String>,
}

#[derive(Serialize)]
struct DataStatus {
    clickhouse_ok: bool,
    events_24h: u64,
    alerts_7d: u64,
    /// Строки IoC с feed=seed (SOC Workbench).
    threat_intel_seed: u64,
    events_per_minute_24h: u64,
    top_ips_24h: u64,
    source_types_24h: u64,
    error: Option<String>,
}

/// Срез метрик из Prometheus (дашборды с datasource prometheus-siem).
#[derive(Serialize)]
struct PrometheusStatus {
    prometheus_ok: bool,
    /// `sum(siem_events_total)` — накапливается только при обработке в **siem-parser** (/parse и ingest), не при прямом INSERT в CH.
    siem_events_total: f64,
    /// `sum(siem_parser_events_parsed_total)` — всё принятое парсером.
    siem_parser_parsed_total: f64,
    /// `sum(siem_parser_events_parsed_total{status="error"})`
    siem_parser_parse_errors: f64,
    /// `sum(detection_events_processed_total)` — движок детекции.
    detection_events_processed_total: f64,
    /// `sum(vector_component_received_events_total{component_id="http_ingest"})` — события на HTTP /logs.
    vector_http_ingest_events_total: f64,
    error: Option<String>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Deserialize)]
struct ParserSeedResponse {
    processed: usize,
    errors: usize,
}

#[derive(Default)]
struct RedAlertRuntime {
    running: bool,
    started_at: Option<String>,
    last_update: Option<String>,
    last_result: Option<String>,
    details: Vec<String>,
    abort: Option<tokio::task::AbortHandle>,
}

#[derive(Serialize)]
struct RedAlertStatus {
    running: bool,
    started_at: Option<String>,
    last_update: Option<String>,
    last_result: Option<String>,
    details: Vec<String>,
}

#[derive(Serialize)]
struct RedAlertActionResult {
    ok: bool,
    message: String,
    status: RedAlertStatus,
}

#[derive(Default)]
struct RedAlertRunStats {
    cycles: usize,
    events_sent: usize,
    processed: usize,
    parse_errors: usize,
}

const SIEM_PREFIX: &str = "siem-";
const RED_ALERT_DURATION_SEC: u64 = 300;
const RED_ALERT_INTERVAL: Duration = Duration::from_secs(1);

/// Имена без префикса `siem-` (как в docker-compose `container_name`).
const SIEM_CONTAINER_EXCEPTIONS: &[&str] = &["detection-engine", "siem-stress"];

fn is_siem_container(name: &str) -> bool {
    name.starts_with(SIEM_PREFIX) || SIEM_CONTAINER_EXCEPTIONS.contains(&name)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let docker = Docker::connect_with_socket_defaults()
        .expect("failed to connect to Docker socket");

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();

    let state = AppState {
        docker: Arc::new(docker),
        http,
        red_alert: Arc::new(tokio::sync::Mutex::new(RedAlertRuntime::default())),
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/services", get(list_services))
        .route("/api/services/:name/stop", post(stop_service))
        .route("/api/services/:name/start", post(start_service))
        .route("/api/services/:name/restart", post(restart_service))
        .route("/api/fill-all-data", post(fill_all_data))
        .route("/api/red-alert", post(start_red_alert))
        .route("/api/red-alert/stop", post(stop_red_alert))
        .route("/api/red-alert/status", get(red_alert_status))
        .route("/api/data-status", get(data_status))
        .route("/api/prometheus-status", get(prometheus_status))
        .route("/api/pipeline", get(pipeline_status))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = std::env::var("ADMIN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8089".into());

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!(addr = %addr, "SIEM Admin Panel starting");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    ctrl_c.await.ok();
}

async fn list_services(
    State(state): State<AppState>,
) -> Result<Json<Vec<ServiceInfo>>, StatusCode> {
    // Не используем filters.name=siem-: на части Docker Engine API фильтр даёт пустой список.
    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };

    let containers = state
        .docker
        .list_containers(Some(opts))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut services: Vec<ServiceInfo> = containers
        .into_iter()
        .filter_map(|c| {
            let name = c
                .names
                .as_ref()
                .and_then(|n| n.first())
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_default();

            if !is_siem_container(&name) {
                return None;
            }

            let ports = c
                .ports
                .unwrap_or_default()
                .iter()
                .filter_map(|p| {
                    let public = p.public_port?;
                    let private = p.private_port;
                    Some(format!("{}:{}", public, private))
                })
                .collect();

            let health = c
                .status
                .as_deref()
                .and_then(|s| {
                    if s.contains("healthy") {
                        Some("healthy".to_string())
                    } else if s.contains("unhealthy") {
                        Some("unhealthy".to_string())
                    } else if s.contains("starting") {
                        Some("starting".to_string())
                    } else {
                        None
                    }
                });

            Some(ServiceInfo {
                name,
                container_id: c.id.unwrap_or_default().chars().take(12).collect(),
                state: c.state.unwrap_or_default(),
                status: c.status.unwrap_or_default(),
                health,
                image: c.image.unwrap_or_default(),
                ports,
                created: c.created.unwrap_or(0),
            })
        })
        .collect();

    services.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(services))
}

async fn stop_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let container_name = resolve_container_name(&state.docker, &name)
        .await
        .ok_or_else(|| json_error(StatusCode::NOT_FOUND, format!("service not found: {}", name)))?;
    state
        .docker
        .stop_container(&container_name, Some(StopContainerOptions { t: 15 }))
        .await
        .map_err(|e| json_error(StatusCode::BAD_GATEWAY, format!("stop failed for {}: {}", container_name, e)))?;

    Ok(Json(serde_json::json!({"status": "stopped", "name": container_name})))
}

async fn start_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let container_name = resolve_container_name(&state.docker, &name)
        .await
        .ok_or_else(|| json_error(StatusCode::NOT_FOUND, format!("service not found: {}", name)))?;
    state
        .docker
        .start_container(&container_name, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| json_error(StatusCode::BAD_GATEWAY, format!("start failed for {}: {}", container_name, e)))?;

    Ok(Json(serde_json::json!({"status": "started", "name": container_name})))
}

async fn restart_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let container_name = resolve_container_name(&state.docker, &name)
        .await
        .ok_or_else(|| json_error(StatusCode::NOT_FOUND, format!("service not found: {}", name)))?;
    state
        .docker
        .restart_container(&container_name, Some(RestartContainerOptions { t: 15 }))
        .await
        .map_err(|e| json_error(StatusCode::BAD_GATEWAY, format!("restart failed for {}: {}", container_name, e)))?;

    Ok(Json(serde_json::json!({"status": "restarted", "name": container_name})))
}

fn normalize_container_name(name: &str) -> String {
    if SIEM_CONTAINER_EXCEPTIONS.contains(&name) || name.starts_with(SIEM_PREFIX) {
        name.to_string()
    } else {
        format!("{SIEM_PREFIX}{name}")
    }
}

async fn resolve_container_name(docker: &Docker, name: &str) -> Option<String> {
    let target = normalize_container_name(name);
    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };
    let containers = docker.list_containers(Some(opts)).await.ok()?;
    for c in containers {
        let names = c.names.unwrap_or_default();
        if names
            .iter()
            .any(|n| n.trim_start_matches('/') == target || n.trim_start_matches('/') == name)
        {
            return names.first().map(|n| n.trim_start_matches('/').to_string());
        }
        if c.id
            .as_deref()
            .map(|id| id.starts_with(name) || id.starts_with(&target))
            .unwrap_or(false)
        {
            if let Some(first_name) = names.first() {
                return Some(first_name.trim_start_matches('/').to_string());
            }
        }
    }
    None
}

fn soc_seed_sql_path() -> String {
    std::env::var("SOC_SEED_SQL_PATH").unwrap_or_else(|_| "/app/seed/seed_test_events.sql".into())
}

fn load_soc_seed_sql() -> Result<String, String> {
    let path = soc_seed_sql_path();
    std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "cannot read SOC seed SQL from {}: {} (set SOC_SEED_SQL_PATH or mount seed_test_events.sql)",
            path, e
        )
    })
}

async fn execute_clickhouse_multiquery(
    http: &reqwest::Client,
    sql: &str,
) -> Result<String, String> {
    let (url, user, password) = clickhouse_config();
    let base = url.trim_end_matches('/').to_string();
    let sep = if base.contains('?') { '&' } else { '?' };
    let url = format!("{base}{sep}allow_multiquery=1");
    let resp = http
        .post(url)
        .basic_auth(user, Some(password))
        .timeout(Duration::from_secs(120))
        .body(sql.to_string())
        .send()
        .await
        .map_err(|e| format!("clickhouse multiquery request failed: {}", e))?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("clickhouse returned {}: {}", status, body));
    }
    Ok(body)
}

async fn fill_all_data(
    State(state): State<AppState>,
) -> Result<Json<FillAllDataResult>, (StatusCode, Json<ErrorBody>)> {
    let mut details = Vec::new();

    let seed_sql = load_soc_seed_sql().map_err(|e| json_error(StatusCode::BAD_GATEWAY, e))?;
    execute_clickhouse_multiquery(&state.http, &seed_sql)
        .await
        .map_err(|e| json_error(StatusCode::BAD_GATEWAY, e))?;
    let soc_bytes = seed_sql.len();
    details.push(format!(
        "ClickHouse SOC seed ({} bytes): siem.events + siem.alerts + siem.threat_intel (see seed_test_events.sql)",
        soc_bytes
    ));

    let stress_name = resolve_container_name(&state.docker, "siem-stress")
        .await
        .ok_or_else(|| json_error(StatusCode::NOT_FOUND, "service not found: siem-stress"))?;

    let stress_action = match state
        .docker
        .restart_container(&stress_name, Some(RestartContainerOptions { t: 15 }))
        .await
    {
        Ok(_) => {
            details.push("siem-stress restarted".to_string());
            "restarted".to_string()
        }
        Err(_) => {
            state
                .docker
                .start_container(&stress_name, None::<StartContainerOptions<String>>)
                .await
                .map_err(|e| json_error(StatusCode::BAD_GATEWAY, format!("start failed for {}: {}", stress_name, e)))?;
            details.push("siem-stress started".to_string());
            "started".to_string()
        }
    };

    let seeded_alerts = seed_alerts_into_clickhouse(&state.http)
        .await
        .map_err(|e| json_error(StatusCode::BAD_GATEWAY, e))?;
    details.push(format!("seeded {} alerts into siem.alerts", seeded_alerts));

    let parser_seed = seed_parser_metrics_and_events(&state.http)
        .await
        .map_err(|e| json_error(StatusCode::BAD_GATEWAY, e))?;
    details.push(format!(
        "seeded parser path: processed={}, errors={}",
        parser_seed.processed, parser_seed.errors
    ));
    details.push(
        "parser seed also injects critical + export/download events for Grafana".to_string(),
    );

    Ok(Json(FillAllDataResult {
        ok: true,
        soc_seed_bytes: soc_bytes,
        stress_action,
        seeded_alerts,
        details,
    }))
}

async fn start_red_alert(
    State(state): State<AppState>,
) -> Result<Json<RedAlertActionResult>, (StatusCode, Json<ErrorBody>)> {
    {
        let runtime = state.red_alert.lock().await;
        if runtime.running {
            return Ok(Json(RedAlertActionResult {
                ok: true,
                message: "red alert mode already running".to_string(),
                status: snapshot_red_alert(&runtime),
            }));
        }
    }

    let stress_name = resolve_container_name(&state.docker, "siem-stress")
        .await
        .ok_or_else(|| json_error(StatusCode::NOT_FOUND, "service not found: siem-stress"))?;

    let stress_action = restart_stress_container(&state, &stress_name).await?;
    let seeded_alerts = seed_alerts_into_clickhouse(&state.http)
        .await
        .map_err(|e| json_error(StatusCode::BAD_GATEWAY, e))?;

    let started_at = now_iso();
    {
        let mut runtime = state.red_alert.lock().await;
        runtime.running = true;
        runtime.started_at = Some(started_at.clone());
        runtime.last_update = Some(started_at.clone());
        runtime.last_result = Some(format!(
            "red alert started: siem-stress {}, seeded {} alerts",
            stress_action, seeded_alerts
        ));
        runtime.details = vec![
            format!("siem-stress {}", stress_action),
            format!("seeded {} alerts into siem.alerts", seeded_alerts),
            format!(
                "running sustained parser attack traffic for {} seconds",
                RED_ALERT_DURATION_SEC
            ),
        ];
    }

    let task_state = state.clone();
    let join = tokio::spawn(async move {
        let result = run_red_alert_mode(task_state.clone()).await;
        let mut runtime = task_state.red_alert.lock().await;
        runtime.running = false;
        runtime.abort = None;
        runtime.last_update = Some(now_iso());
        match result {
            Ok(stats) => {
                runtime.last_result = Some(format!(
                    "completed: cycles={}, sent={}, processed={}, parse_errors={}",
                    stats.cycles, stats.events_sent, stats.processed, stats.parse_errors
                ));
                runtime.details.push(format!(
                    "completed red alert: {} cycles, {} parser events sent",
                    stats.cycles, stats.events_sent
                ));
                runtime.details.push(format!(
                    "parser accepted {}, parser errors {}",
                    stats.processed, stats.parse_errors
                ));
            }
            Err(error) => {
                runtime.last_result = Some(format!("failed: {}", error));
                runtime.details.push(format!("red alert failed: {}", error));
            }
        }
    });

    {
        let mut runtime = state.red_alert.lock().await;
        runtime.abort = Some(join.abort_handle());
        runtime.last_update = Some(now_iso());
        runtime.details.push("red alert task spawned".to_string());
        return Ok(Json(RedAlertActionResult {
            ok: true,
            message: "red alert mode started".to_string(),
            status: snapshot_red_alert(&runtime),
        }));
    }
}

async fn stop_red_alert(
    State(state): State<AppState>,
) -> Result<Json<RedAlertActionResult>, (StatusCode, Json<ErrorBody>)> {
    let mut runtime = state.red_alert.lock().await;
    if !runtime.running {
        return Ok(Json(RedAlertActionResult {
            ok: true,
            message: "red alert mode is not running".to_string(),
            status: snapshot_red_alert(&runtime),
        }));
    }

    if let Some(handle) = runtime.abort.take() {
        handle.abort();
    }
    runtime.running = false;
    runtime.last_update = Some(now_iso());
    runtime.last_result = Some("stopped by user".to_string());
    runtime.details.push("red alert stopped by user".to_string());

    Ok(Json(RedAlertActionResult {
        ok: true,
        message: "red alert mode stopped".to_string(),
        status: snapshot_red_alert(&runtime),
    }))
}

async fn red_alert_status(State(state): State<AppState>) -> Json<RedAlertStatus> {
    let runtime = state.red_alert.lock().await;
    Json(snapshot_red_alert(&runtime))
}

async fn data_status(State(state): State<AppState>) -> Json<DataStatus> {
    let queries = [
        (
            "events_24h",
            "SELECT count() FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR FORMAT TabSeparatedRaw",
        ),
        (
            "alerts_7d",
            "SELECT count() FROM siem.alerts WHERE triggered_at >= now() - INTERVAL 7 DAY FORMAT TabSeparatedRaw",
        ),
        (
            "threat_intel_seed",
            "SELECT count() FROM siem.threat_intel WHERE feed = 'seed' FORMAT TabSeparatedRaw",
        ),
        (
            "events_per_minute_24h",
            "SELECT count() FROM siem.events_per_minute_agg WHERE minute >= now() - INTERVAL 24 HOUR FORMAT TabSeparatedRaw",
        ),
        (
            "top_ips_24h",
            "SELECT count() FROM siem.top_ips_agg WHERE hour >= now() - INTERVAL 24 HOUR FORMAT TabSeparatedRaw",
        ),
        (
            "source_types_24h",
            "SELECT uniqExact(source_type) FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR FORMAT TabSeparatedRaw",
        ),
    ];

    let mut values = std::collections::HashMap::new();
    for (key, query) in queries {
        match clickhouse_scalar_u64(&state.http, query).await {
            Ok(value) => {
                values.insert(key, value);
            }
            Err(error) => {
                return Json(DataStatus {
                    clickhouse_ok: false,
                    events_24h: 0,
                    alerts_7d: 0,
                    threat_intel_seed: 0,
                    events_per_minute_24h: 0,
                    top_ips_24h: 0,
                    source_types_24h: 0,
                    error: Some(error),
                });
            }
        }
    }

    Json(DataStatus {
        clickhouse_ok: true,
        events_24h: *values.get("events_24h").unwrap_or(&0),
        alerts_7d: *values.get("alerts_7d").unwrap_or(&0),
        threat_intel_seed: *values.get("threat_intel_seed").unwrap_or(&0),
        events_per_minute_24h: *values.get("events_per_minute_24h").unwrap_or(&0),
        top_ips_24h: *values.get("top_ips_24h").unwrap_or(&0),
        source_types_24h: *values.get("source_types_24h").unwrap_or(&0),
        error: None,
    })
}

async fn seed_parser_metrics_and_events(http: &reqwest::Client) -> Result<ParserSeedResponse, String> {
    let export_ts = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let download_ts = (Utc::now() + chrono::Duration::seconds(1))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    let sqli_ts = (Utc::now() + chrono::Duration::seconds(2))
        .to_rfc3339_opts(SecondsFormat::Millis, true);

    let body = serde_json::json!({
        "events": [
            parser_seed_event(
                build_parser_seed_http_event(
                    &export_ts,
                    "Fatal",
                    "Export completed for /api/reports/export.csv",
                    "203.0.113.200",
                    "GET",
                    "/api/reports/export.csv",
                    200,
                    182.4,
                    "export_user",
                ),
                "dotnet",
                "api-01",
            ),
            parser_seed_event(
                build_parser_seed_http_event(
                    &download_ts,
                    "Fatal",
                    "Large download from /api/downloads/dump.zip",
                    "203.0.113.200",
                    "GET",
                    "/api/downloads/dump.zip",
                    206,
                    245.7,
                    "export_user",
                ),
                "dotnet",
                "api-02",
            ),
            parser_seed_event(
                build_parser_seed_http_event(
                    &sqli_ts,
                    "Fatal",
                    "Detected SQL injection payload in request",
                    "198.51.100.20",
                    "GET",
                    "/api/search/union/select/users",
                    500,
                    355.9,
                    "api_guest",
                ),
                "dotnet",
                "api-03",
            ),
            parser_seed_event("{bad json".to_string(), "dotnet", "api-04"),
        ]
    });

    let resp = http
        .post("http://siem-parser:7000/parse")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("parser request failed: {}", e))?;

    let status = resp.status();
    let response_body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("siem-parser returned {}: {}", status, response_body));
    }

    serde_json::from_str::<ParserSeedResponse>(&response_body)
        .map_err(|e| format!("failed to parse siem-parser response: {}", e))
}

fn parser_seed_event(raw: String, source_type: &str, host: &str) -> serde_json::Value {
    serde_json::json!({
        "raw": raw,
        "source_type": source_type,
        "host": host,
    })
}

fn build_parser_seed_http_event(
    timestamp: &str,
    level: &str,
    message: &str,
    ip: &str,
    method: &str,
    path: &str,
    status_code: u16,
    elapsed_ms: f64,
    user_id: &str,
) -> String {
    serde_json::json!({
        "Timestamp": timestamp,
        "Level": level,
        "Message": message,
        "ClientIp": ip,
        "RequestMethod": method,
        "RequestPath": path,
        "StatusCode": status_code,
        "Elapsed": elapsed_ms,
        "UserId": user_id,
        "Properties": {
            "ClientIp": ip,
            "RequestMethod": method,
            "RequestPath": path,
            "StatusCode": status_code,
            "Elapsed": elapsed_ms,
            "UserId": user_id,
        }
    })
    .to_string()
}

fn build_parser_seed_http_event_with_role(
    timestamp: &str,
    level: &str,
    message: &str,
    ip: &str,
    method: &str,
    path: &str,
    status_code: u16,
    elapsed_ms: f64,
    user_id: &str,
    user_role: &str,
    user_agent: &str,
) -> String {
    serde_json::json!({
        "Timestamp": timestamp,
        "Level": level,
        "Message": message,
        "SourceType": "dotnet",
        "Host": "api-critical-01",
        "ClientIp": ip,
        "RequestMethod": method,
        "RequestPath": path,
        "StatusCode": status_code,
        "Elapsed": elapsed_ms,
        "UserId": user_id,
        "Properties": {
            "ClientIp": ip,
            "RequestMethod": method,
            "RequestPath": path,
            "StatusCode": status_code,
            "Elapsed": elapsed_ms,
            "UserId": user_id,
            "UserRole": user_role,
            "UserAgent": user_agent,
            "CorrelationId": Uuid::new_v4().to_string(),
        }
    })
    .to_string()
}

async fn seed_alerts_into_clickhouse(http: &reqwest::Client) -> Result<usize, String> {
    let total = 50usize;
    let columns = clickhouse_table_columns(http, "siem.alerts").await?;
    let sql = build_seed_alerts_sql(total, &columns);
    execute_clickhouse_query(http, sql).await?;
    Ok(total)
}

async fn restart_stress_container(
    state: &AppState,
    stress_name: &str,
) -> Result<String, (StatusCode, Json<ErrorBody>)> {
    match state
        .docker
        .restart_container(stress_name, Some(RestartContainerOptions { t: 15 }))
        .await
    {
        Ok(_) => Ok("restarted".to_string()),
        Err(_) => {
            state
                .docker
                .start_container(stress_name, None::<StartContainerOptions<String>>)
                .await
                .map_err(|e| {
                    json_error(
                        StatusCode::BAD_GATEWAY,
                        format!("start failed for {}: {}", stress_name, e),
                    )
                })?;
            Ok("started".to_string())
        }
    }
}

fn snapshot_red_alert(runtime: &RedAlertRuntime) -> RedAlertStatus {
    RedAlertStatus {
        running: runtime.running,
        started_at: runtime.started_at.clone(),
        last_update: runtime.last_update.clone(),
        last_result: runtime.last_result.clone(),
        details: runtime.details.clone(),
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

async fn run_red_alert_mode(state: AppState) -> Result<RedAlertRunStats, String> {
    let deadline = Instant::now() + Duration::from_secs(RED_ALERT_DURATION_SEC);
    let mut stats = RedAlertRunStats::default();

    while Instant::now() < deadline {
        let response = send_red_alert_batch(&state.http).await?;
        stats.cycles += 1;
        stats.events_sent += 69;
        stats.processed += response.processed;
        stats.parse_errors += response.errors;

        let mut runtime = state.red_alert.lock().await;
        runtime.last_update = Some(now_iso());
        runtime.last_result = Some(format!(
            "running: cycles={}, sent={}, processed={}, parse_errors={}",
            stats.cycles, stats.events_sent, stats.processed, stats.parse_errors
        ));
        runtime.details.retain(|line| {
            !line.starts_with("live counters:")
                && !line.starts_with("last parser batch:")
        });
        runtime.details.push(format!(
            "last parser batch: processed={}, errors={}",
            response.processed, response.errors
        ));
        runtime.details.push(format!(
            "live counters: cycles={}, sent={}, processed={}, parse_errors={}",
            stats.cycles, stats.events_sent, stats.processed, stats.parse_errors
        ));
        drop(runtime);

        tokio::time::sleep(RED_ALERT_INTERVAL).await;
    }

    Ok(stats)
}

async fn send_red_alert_batch(http: &reqwest::Client) -> Result<ParserSeedResponse, String> {
    let body = serde_json::json!({
        "events": build_red_alert_events()
            .into_iter()
            .map(|raw| parser_seed_event(raw, "dotnet", "api-critical-01"))
            .collect::<Vec<_>>()
    });

    let resp = http
        .post("http://siem-parser:7000/parse")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("parser request failed: {}", e))?;

    let status = resp.status();
    let response_body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("siem-parser returned {}: {}", status, response_body));
    }

    serde_json::from_str::<ParserSeedResponse>(&response_body)
        .map_err(|e| format!("failed to parse siem-parser response: {}", e))
}

fn build_red_alert_events() -> Vec<String> {
    let mut events = Vec::with_capacity(69);
    let base = Utc::now();
    let attacker = "203.0.113.5";
    let takeover_ip = "203.0.113.12";
    let admin_ip = "192.168.1.10";
    let exfil_ip = "198.51.100.20";

    for idx in 0..12 {
        let ts = (base + chrono::Duration::milliseconds(idx * 25))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Error",
            &format!("Authentication failed for user admin from {}", attacker),
            attacker,
            "POST",
            "/api/auth/login",
            401,
            32.0 + idx as f64,
            "admin",
            "anonymous",
            "Hydra/9.5",
        ));
    }

    for idx in 0..6 {
        let ts = (base + chrono::Duration::milliseconds(500 + idx * 25))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Error",
            &format!("HTTP GET /api/search responded 500 in {:.2}ms; detected payload UNION SELECT username,password FROM users", 280.0 + idx as f64),
            attacker,
            "GET",
            "/api/search/union/select/users",
            500,
            280.0 + idx as f64,
            "guest_user",
            "user",
            "sqlmap/1.7.8",
        ));
    }

    for idx in 0..8 {
        let ts = (base + chrono::Duration::milliseconds(800 + idx * 25))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Error",
            &format!("Unauthorized access attempt to /api/admin/config by user user_{:03}", idx + 1),
            admin_ip,
            "GET",
            "/api/admin/config",
            403,
            75.0 + idx as f64,
            &format!("user_{:03}", idx + 1),
            "user",
            "Mozilla/5.0",
        ));
    }

    for idx in 0..4 {
        let ts = (base + chrono::Duration::milliseconds(1050 + idx * 25))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Information",
            "HTTP GET /api/admin/config responded 200 in 28.10ms",
            admin_ip,
            "GET",
            "/api/admin/config",
            200,
            28.1 + idx as f64,
            "svc-admin",
            "admin",
            "Mozilla/5.0",
        ));
    }

    for idx in 0..10 {
        let ts = (base + chrono::Duration::milliseconds(1200 + idx * 25))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Fatal",
            &format!("Large download from /api/downloads/dump-{}.zip", idx),
            exfil_ip,
            "GET",
            &format!("/api/downloads/dump-{}.zip", idx),
            206,
            420.0 + idx as f64,
            "export_user",
            "analyst",
            "curl/8.6.0",
        ));
    }

    for idx in 0..5 {
        let ts = (base + chrono::Duration::milliseconds(1500 + idx * 25))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Fatal",
            &format!("Export completed for /api/reports/export-{}.csv", idx),
            exfil_ip,
            "GET",
            &format!("/api/reports/export-{}.csv", idx),
            200,
            180.0 + idx as f64,
            "export_user",
            "analyst",
            "curl/8.6.0",
        ));
    }

    for idx in 0..9 {
        let ts = (base + chrono::Duration::milliseconds(1700 + idx * 20))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Warning",
            &format!("Authentication failed for user admin from {}", takeover_ip),
            takeover_ip,
            "POST",
            "/api/auth/token",
            403,
            27.0 + idx as f64,
            "admin",
            "anonymous",
            "Hydra/9.5",
        ));
    }

    for idx in 0..3 {
        let ts = (base + chrono::Duration::milliseconds(1900 + idx * 20))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Information",
            "HTTP POST /api/auth/login responded 200 in 18.30ms",
            takeover_ip,
            "POST",
            "/api/auth/login",
            200,
            18.3 + idx as f64,
            "admin",
            "admin",
            "Mozilla/5.0",
        ));
    }

    for idx in 0..8 {
        let ts = (base + chrono::Duration::milliseconds(2050 + idx * 15))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Warning",
            &format!("Rate limit exceeded: {} requests in 60s from 203.0.113.88", 550 + idx),
            "203.0.113.88",
            "GET",
            "/api/search",
            429,
            9.0 + idx as f64,
            "none",
            "anonymous",
            "attack-bot/1.0",
        ));
    }

    for idx in 0..3 {
        let ts = (base + chrono::Duration::milliseconds(2200 + idx * 20))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        events.push(build_parser_seed_http_event_with_role(
            &ts,
            "Error",
            &format!("HTTP GET /api/orders responded 500 in {:.2}ms", 12_500.0 + idx as f64 * 1000.0),
            attacker,
            "GET",
            "/api/orders",
            500,
            12_500.0 + idx as f64 * 1000.0,
            "svc-orders",
            "service",
            "Mozilla/5.0",
        ));
    }

    events.push("{bad json".to_string());
    assert_eq!(events.len(), 69);
    events
}

fn build_seed_alerts_sql(total: usize, columns: &[String]) -> String {
    let mut rows = Vec::with_capacity(total);
    let rule_specs = [
        (
            "brute_force_api",
            "API / SignalR Brute-Force Authentication Attempts",
            "high",
            vec!["T1110", "T1110.001"],
        ),
        (
            "sql_injection_attempt",
            "SQL/NoSQL Injection Attempt Detected in Application Logs",
            "critical",
            vec!["T1190", "T1059.007"],
        ),
        (
            "privilege_escalation_attempt",
            "Privilege Escalation or Unauthorized Admin Access Attempt",
            "critical",
            vec!["T1068", "T1078.003", "T1548"],
        ),
        (
            "rate_limit_evasion",
            "Rate Limit Evasion - Anomalous Request Volume from Single IP",
            "medium",
            vec!["T1595", "T1595.002"],
        ),
    ];
    let attacker_ips = ["203.0.113.5", "203.0.113.12", "198.51.100.20", "203.0.113.88"];
    let supported_columns: Vec<String> = [
        "alert_id",
        "fingerprint",
        "triggered_at",
        "rule_id",
        "rule_title",
        "severity",
        "description",
        "source_ip",
        "user_id",
        "event_ids",
        "mitre_tags",
        "status",
        "acknowledged_by",
        "acknowledged_at",
        "notes",
    ]
    .iter()
    .filter(|c| columns.iter().any(|actual| actual == **c))
    .map(|c| c.to_string())
    .collect();

    for idx in 0..total {
        let (status, severity) = if idx < 20 {
            ("new", if idx < 10 { "critical" } else { "high" })
        } else if idx < 35 {
            ("acknowledged", "medium")
        } else if idx < 45 {
            ("resolved", if idx % 2 == 0 { "low" } else { "medium" })
        } else {
            ("false_positive", "low")
        };

        let spec = &rule_specs[idx % rule_specs.len()];
        let triggered_at = Utc::now() - chrono::Duration::hours((idx * 3) as i64);
        let ack_at = if matches!(status, "acknowledged" | "resolved") {
            Some(triggered_at + chrono::Duration::minutes(30))
        } else {
            None
        };

        let event_ids = format!("['{}']", Uuid::new_v4());
        let mitre_tags = format!(
            "[{}]",
            spec.3
                .iter()
                .map(|t| format!("'{}'", t))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let ack_by = if ack_at.is_some() {
            "'soc_analyst_1'".to_string()
        } else {
            "NULL".to_string()
        };
        let ack_ts = if let Some(ts) = ack_at {
            format!("toDateTime64('{}', 3, 'UTC')", ts.format("%Y-%m-%d %H:%M:%S%.3f"))
        } else {
            "NULL".to_string()
        };
        let mut values = std::collections::HashMap::new();
        values.insert("alert_id", format!("toUUID('{}')", Uuid::new_v4()));
        values.insert("fingerprint", format!("'{}'", Uuid::new_v4()));
        values.insert(
            "triggered_at",
            format!(
                "toDateTime64('{}', 3, 'UTC')",
                triggered_at.format("%Y-%m-%d %H:%M:%S%.3f")
            ),
        );
        values.insert("rule_id", format!("'{}'", spec.0));
        values.insert("rule_title", format!("'{}'", spec.1.replace('\'', "\\'")));
        values.insert("severity", format!("'{}'", severity));
        values.insert(
            "description",
            format!(
                "'{}'",
                format!("Synthetic {} alert for dashboard population", spec.0)
                    .replace('\'', "\\'")
            ),
        );
        values.insert(
            "source_ip",
            format!("toIPv4('{}')", attacker_ips[idx % attacker_ips.len()]),
        );
        values.insert("user_id", "'seed_user'".to_string());
        values.insert("event_ids", event_ids);
        values.insert("mitre_tags", mitre_tags);
        values.insert("status", format!("'{}'", status));
        values.insert("acknowledged_by", ack_by);
        values.insert("acknowledged_at", ack_ts);
        values.insert("notes", "'Seeded via SIEM Admin fill-all-data'".to_string());

        rows.push(format!(
            "({})",
            supported_columns
                .iter()
                .filter_map(|col| values.get(col.as_str()).cloned())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    format!(
        "INSERT INTO siem.alerts ({}) VALUES {}",
        supported_columns.join(", "),
        rows.join(", ")
    )
}

fn clickhouse_config() -> (String, String, String) {
    let url = std::env::var("CLICKHOUSE_URL")
        .unwrap_or_else(|_| "http://siem-clickhouse:8123".to_string());
    let user = std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "siem".to_string());
    let password = if let Ok(path) = std::env::var("CLICKHOUSE_PASSWORD_FILE") {
        std::fs::read_to_string(path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| {
                std::env::var("CLICKHOUSE_PASSWORD")
                    .unwrap_or_else(|_| "ClickHousePass123!".to_string())
            })
    } else {
        std::env::var("CLICKHOUSE_PASSWORD")
            .unwrap_or_else(|_| "ClickHousePass123!".to_string())
    };
    (url, user, password)
}

fn prometheus_base_url() -> String {
    std::env::var("PROMETHEUS_URL").unwrap_or_else(|_| "http://siem-prometheus:9090".into())
}

/// Instant query (`/api/v1/query`); пустой `result` → `0.0`.
async fn prometheus_instant_scalar(http: &reqwest::Client, query: &str) -> Result<f64, String> {
    let url = format!("{}/api/v1/query", prometheus_base_url().trim_end_matches('/'));
    let resp = http
        .get(url)
        .query(&[("query", query)])
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("prometheus: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("prometheus HTTP {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if v.get("status").and_then(|s| s.as_str()) != Some("success") {
        return Err("prometheus: status != success".into());
    }
    let Some(rows) = v["data"]["result"].as_array() else {
        return Ok(0.0);
    };
    if rows.is_empty() {
        return Ok(0.0);
    }
    let s = rows[0]["value"]
        .get(1)
        .and_then(|x| x.as_str())
        .unwrap_or("0");
    s.parse::<f64>()
        .map_err(|_| format!("prometheus: bad scalar {:?}", s))
}

async fn prometheus_status(State(state): State<AppState>) -> Json<PrometheusStatus> {
    if let Err(e) = prometheus_instant_scalar(&state.http, "time()").await {
        return Json(PrometheusStatus {
            prometheus_ok: false,
            siem_events_total: 0.0,
            siem_parser_parsed_total: 0.0,
            siem_parser_parse_errors: 0.0,
            detection_events_processed_total: 0.0,
            vector_http_ingest_events_total: 0.0,
            error: Some(e),
        });
    }

    let siem_events_total = prometheus_instant_scalar(&state.http, "sum(siem_events_total)")
        .await
        .unwrap_or(0.0);
    let siem_parser_parsed_total =
        prometheus_instant_scalar(&state.http, "sum(siem_parser_events_parsed_total)")
            .await
            .unwrap_or(0.0);
    let siem_parser_parse_errors = prometheus_instant_scalar(
        &state.http,
        r#"sum(siem_parser_events_parsed_total{status="error"})"#,
    )
    .await
    .unwrap_or(0.0);
    let detection_events_processed_total =
        prometheus_instant_scalar(&state.http, "sum(detection_events_processed_total)")
            .await
            .unwrap_or(0.0);
    let vector_http_ingest_events_total = prometheus_instant_scalar(
        &state.http,
        r#"sum(vector_component_received_events_total{component_id="http_ingest"})"#,
    )
    .await
    .unwrap_or(0.0);

    Json(PrometheusStatus {
        prometheus_ok: true,
        siem_events_total,
        siem_parser_parsed_total,
        siem_parser_parse_errors,
        detection_events_processed_total,
        vector_http_ingest_events_total,
        error: None,
    })
}

async fn execute_clickhouse_query(http: &reqwest::Client, query: String) -> Result<String, String> {
    let (url, user, password) = clickhouse_config();
    let resp = http
        .post(url)
        .basic_auth(user, Some(password))
        .body(query)
        .send()
        .await
        .map_err(|e| format!("clickhouse request failed: {}", e))?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("clickhouse returned {}: {}", status, body));
    }
    Ok(body)
}

async fn clickhouse_scalar_u64(http: &reqwest::Client, query: &str) -> Result<u64, String> {
    let body = execute_clickhouse_query(http, query.to_string()).await?;
    body.trim()
        .parse::<u64>()
        .map_err(|e| format!("failed to parse ClickHouse scalar '{}': {}", body.trim(), e))
}

async fn clickhouse_table_columns(
    http: &reqwest::Client,
    table_name: &str,
) -> Result<Vec<String>, String> {
    let body = execute_clickhouse_query(
        http,
        format!("DESCRIBE TABLE {} FORMAT TabSeparatedRaw", table_name),
    )
    .await?;
    let columns = body
        .lines()
        .filter_map(|line| line.split('\t').next())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err(format!("no columns returned for table {}", table_name));
    }
    Ok(columns)
}

async fn pipeline_status(
    State(state): State<AppState>,
) -> Json<PipelineStatus> {
    // Метрики надёжнее «готовности» API: /health у Vector и /v1/status/ready у Redpanda
    // на части версий/сборок отвечают иначе; :9598 и :9644 /metrics стабильны.
    let endpoints: Vec<(&str, &str)> = vec![
        ("Vector Aggregator", "http://siem-vector-aggregator:9598/metrics"),
        ("Redpanda", "http://siem-redpanda:9644/metrics"),
        ("ClickHouse", "http://siem-clickhouse:8123/ping"),
        ("siem-parser", "http://siem-parser:7000/health"),
        ("Detection Engine", "http://detection-engine:9110/health"),
        ("Correlator", "http://siem-correlator:9111/health"),
        ("Case Management", "http://siem-case-management:8088/health"),
        ("Grafana", "http://siem-grafana:3000/api/health"),
        ("Prometheus", "http://siem-prometheus:9090/-/healthy"),
        ("Alertmanager", "http://siem-alertmanager:9093/-/healthy"),
    ];

    let mut checks = Vec::new();
    let mut all_ok = true;

    for (name, url) in endpoints {
        let (ok, detail) = match state.http.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    (true, format!("HTTP {}", status.as_u16()))
                } else {
                    (false, format!("HTTP {}", status.as_u16()))
                }
            }
            Err(e) => (false, format!("{}", e)),
        };
        if !ok {
            all_ok = false;
        }
        checks.push(PipelineCheck {
            name: name.to_string(),
            ok,
            detail,
        });
    }

    Json(PipelineStatus {
        checks,
        healthy: all_ok,
    })
}

fn json_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ErrorBody>) {
    (
        status,
        Json(ErrorBody {
            error: message.into(),
        }),
    )
}

async fn index_handler() -> Response {
    let html = include_str!("../static/index.html");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}
