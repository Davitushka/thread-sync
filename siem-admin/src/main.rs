use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bollard::container::{ListContainersOptions, StartContainerOptions, StopContainerOptions};
use bollard::Docker;
use serde::Serialize;
use tower_http::cors::CorsLayer;

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

const SIEM_PREFIX: &str = "siem-";

/// Имена без префикса `siem-` (как в docker-compose `container_name`).
const SIEM_CONTAINER_EXCEPTIONS: &[&str] = &["detection-engine"];

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
        .route("/api/services/{name}/stop", post(stop_service))
        .route("/api/services/{name}/start", post(start_service))
        .route("/api/services/{name}/restart", post(restart_service))
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
    let opts = ListContainersOptions {
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
) -> Result<Json<serde_json::Value>, StatusCode> {
    let container_name = resolve_container(&name);
    state
        .docker
        .stop_container(&container_name, Some(StopContainerOptions { t: 15 }))
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({"status": "stopped", "name": container_name})))
}

async fn start_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let container_name = resolve_container(&name);
    state
        .docker
        .start_container(&container_name, None::<StartContainerOptions<String>>)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({"status": "started", "name": container_name})))
}

async fn restart_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let container_name = resolve_container(&name);
    state
        .docker
        .restart_container(&container_name, Some(bollard::container::RestartContainerOptions { t: 15 }))
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({"status": "restarted", "name": container_name})))
}

fn resolve_container(name: &str) -> String {
    if SIEM_CONTAINER_EXCEPTIONS.contains(&name) || name.starts_with(SIEM_PREFIX) {
        name.to_string()
    } else {
        format!("{SIEM_PREFIX}{name}")
    }
}

async fn pipeline_status(
    State(state): State<AppState>,
) -> Json<PipelineStatus> {
    // Метрики надёжнее «готовности» API: /health у Vector и /v1/status/ready у Redpanda
    // на части версий/сборок отвечают иначе; :9598 и :9644 /metrics стабильны.
    let endpoints: Vec<(&str, &str)> = vec![
        ("Vector Aggregator", "http://siem-vector-aggregator:9598/metrics"),
        ("Redpanda", "http://siem-redpanda:9644/metrics"),
        ("ClickHouse", "http://siem-clickhouse:8123/?query=SELECT%201"),
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

async fn index_handler() -> Response {
    let html = include_str!("../static/index.html");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}
