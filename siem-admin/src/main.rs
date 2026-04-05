use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bollard::container::{
    ListContainersOptions, RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::Docker;
use chrono::Utc;
use serde::Serialize;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    docker: Arc<Docker>,
    http: reqwest::Client,
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
    stress_action: String,
    seeded_alerts: usize,
    details: Vec<String>,
}

#[derive(Serialize)]
struct DataStatus {
    clickhouse_ok: bool,
    events_24h: u64,
    alerts_7d: u64,
    events_per_minute_24h: u64,
    top_ips_24h: u64,
    source_types_24h: u64,
    error: Option<String>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

const SIEM_PREFIX: &str = "siem-";

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
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/services", get(list_services))
        .route("/api/services/:name/stop", post(stop_service))
        .route("/api/services/:name/start", post(start_service))
        .route("/api/services/:name/restart", post(restart_service))
        .route("/api/fill-all-data", post(fill_all_data))
        .route("/api/data-status", get(data_status))
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

async fn fill_all_data(
    State(state): State<AppState>,
) -> Result<Json<FillAllDataResult>, (StatusCode, Json<ErrorBody>)> {
    let mut details = Vec::new();

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

    Ok(Json(FillAllDataResult {
        ok: true,
        stress_action,
        seeded_alerts,
        details,
    }))
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
        events_per_minute_24h: *values.get("events_per_minute_24h").unwrap_or(&0),
        top_ips_24h: *values.get("top_ips_24h").unwrap_or(&0),
        source_types_24h: *values.get("source_types_24h").unwrap_or(&0),
        error: None,
    })
}

async fn seed_alerts_into_clickhouse(http: &reqwest::Client) -> Result<usize, String> {
    let total = 50usize;
    let columns = clickhouse_table_columns(http, "siem.alerts").await?;
    let sql = build_seed_alerts_sql(total, &columns);
    execute_clickhouse_query(http, sql).await?;
    Ok(total)
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
