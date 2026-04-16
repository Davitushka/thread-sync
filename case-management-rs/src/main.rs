mod alertmanager;
mod handlers;
mod models;
mod portal_notify;
mod store;
mod validate;

use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct PortalNotifyConfig {
    pub base_url: String,
    pub secret: String,
}

#[derive(Clone)]
pub struct AppState {
    pub store: store::Store,
    pub auto_from_alerts: bool,
    pub auto_min_severity: String,
    pub default_actor: String,
    pub grafana_base_url: String,
    pub http: reqwest::Client,
    pub portal_notify: Option<PortalNotifyConfig>,
    /// Bearer token for API authentication. If empty, auth is disabled (dev mode).
    pub api_key: String,
    /// Bearer token that Alertmanager must send in Authorization header.
    pub webhook_secret: String,
    /// Semaphore to limit concurrent portal notification tasks
    pub notify_sem: std::sync::Arc<tokio::sync::Semaphore>,
}

pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({"error": self.message}))).into_response()
    }
}

#[cfg(feature = "embed-spa")]
#[derive(rust_embed::RustEmbed)]
#[folder = "../case-management/web/dist"]
struct Asset;

#[cfg(feature = "embed-spa")]
async fn spa_handler(
    method: axum::http::Method,
    uri: axum::http::Uri,
) -> Response {
    use axum::http::Method;

    if method != Method::GET {
        return StatusCode::NOT_FOUND.into_response();
    }

    let path = uri.path().trim_start_matches('/');

    if !path.is_empty() && path.contains('.') {
        if let Some(file) = Asset::get(path) {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();
            return Response::builder()
                .status(StatusCode::OK)
                .header("content-type", mime)
                .body(axum::body::Body::from(file.data.to_vec()))
                .unwrap();
        }
    }

    match Asset::get("index.html") {
        Some(file) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html; charset=utf-8")
            .body(axum::body::Body::from(file.data.to_vec()))
            .unwrap(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(not(feature = "embed-spa"))]
async fn spa_handler() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Bearer token authentication middleware.
/// If `api_key` is set in state, requires `Authorization: Bearer <key>` on all /api/ routes.
async fn auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if state.api_key.is_empty() {
        return next.run(req).await;
    }
    let ok = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| {
            auth.strip_prefix("Bearer ")
                .or_else(|| auth.strip_prefix("bearer "))
                .map(|t| t.trim() == state.api_key)
        })
        .unwrap_or(false);
    if ok {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({"error": "unauthorized"}))).into_response()
    }
}

/// Webhook authentication: validates Alertmanager sends correct bearer token.
async fn webhook_auth_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if state.webhook_secret.is_empty() {
        return next.run(req).await;
    }
    let ok = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| {
            auth.strip_prefix("Bearer ")
                .or_else(|| auth.strip_prefix("bearer "))
                .map(|t| t.trim() == state.webhook_secret)
        })
        .unwrap_or(false);
    if ok {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({"error": "unauthorized"}))).into_response()
    }
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let dsn = std::env::var("CASEMGMT_DATABASE_URL").unwrap_or_default();
    let dsn = dsn.trim();
    if dsn.is_empty() {
        tracing::error!("CASEMGMT_DATABASE_URL is required");
        std::process::exit(1);
    }

    let db = store::Store::new(dsn).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "postgres");
        std::process::exit(1);
    });

    for (name, sql) in [
        ("001_init", include_str!("../migrations/001_init.sql")),
        (
            "002_process_investigation",
            include_str!("../migrations/002_process_investigation.sql"),
        ),
    ] {
        if let Err(e) = db.migrate(sql).await {
            tracing::error!(migration = name, error = %e, "migrate");
            std::process::exit(1);
        }
    }

    let auto_val = std::env::var("CASEMGMT_AUTO_CASE_FROM_ALERTS")
        .unwrap_or_default()
        .to_lowercase();
    let auto_from_alerts = !matches!(auto_val.trim(), "false" | "0" | "no");
    let auto_min_severity = std::env::var("CASEMGMT_AUTO_CASE_MIN_SEVERITY")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let auto_min_severity = if auto_min_severity.is_empty() {
        "medium".to_string()
    } else {
        auto_min_severity
    };
    let default_actor = std::env::var("CASEMGMT_DEFAULT_ACTOR")
        .unwrap_or_default()
        .trim()
        .to_string();
    let default_actor = if default_actor.is_empty() {
        "system".to_string()
    } else {
        default_actor
    };
    let grafana = std::env::var("CASEMGMT_GRAFANA_EXTERNAL_URL")
        .unwrap_or_default()
        .trim()
        .to_string();
    let grafana_base_url = if grafana.is_empty() {
        "http://localhost:3000".to_string()
    } else {
        grafana.trim_end_matches('/').to_string()
    };

    let portal_base = std::env::var("CASEMGMT_PORTAL_NOTIFY_URL")
        .unwrap_or_default()
        .trim()
        .to_string();
    let portal_secret = std::env::var("CASEMGMT_PORTAL_NOTIFY_SECRET")
        .unwrap_or_default()
        .trim()
        .to_string();
    let portal_notify = if !portal_base.is_empty() && !portal_secret.is_empty() {
        Some(PortalNotifyConfig {
            base_url: portal_base,
            secret: portal_secret,
        })
    } else {
        None
    };

    let api_key = std::env::var("CASEMGMT_API_KEY")
        .unwrap_or_default()
        .trim()
        .to_string();
    if api_key.is_empty() {
        tracing::warn!("CASEMGMT_API_KEY not set — API authentication disabled (dev mode)");
    }

    let webhook_secret = std::env::var("CASEMGMT_WEBHOOK_SECRET")
        .unwrap_or_default()
        .trim()
        .to_string();

    let http = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "reqwest client");
            std::process::exit(1);
        });

    let state = AppState {
        store: db,
        auto_from_alerts,
        auto_min_severity,
        default_actor,
        grafana_base_url,
        http,
        portal_notify,
        api_key,
        webhook_secret,
        notify_sem: std::sync::Arc::new(tokio::sync::Semaphore::new(16)),
    };

    let allowed_origin = std::env::var("CASEMGMT_CORS_ORIGIN")
        .unwrap_or_default()
        .trim()
        .to_string();
    let cors = if allowed_origin.is_empty() {
        CorsLayer::permissive()
    } else {
        let origin: axum::http::HeaderValue = allowed_origin.parse().unwrap_or_else(|_| {
            tracing::warn!("Invalid CASEMGMT_CORS_ORIGIN, falling back to permissive CORS");
            axum::http::HeaderValue::from_static("*")
        });
        CorsLayer::new()
            .allow_origin(origin)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PATCH,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
            ])
    };

    // API routes with Bearer auth
    let api_routes = Router::new()
        .route("/api/v1/cases", get(handlers::list_cases).post(handlers::create_case))
        .route(
            "/api/v1/cases/{id}/investigate",
            get(handlers::investigate_case),
        )
        .route(
            "/api/v1/cases/{id}",
            get(handlers::get_case).patch(handlers::patch_case),
        )
        .route("/api/v1/cases/{id}/timeline", post(handlers::add_timeline))
        .route("/api/v1/cases/{id}/events", post(handlers::link_event))
        .route("/api/v1/cases/{id}/alerts", post(handlers::link_alert))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Webhook route with separate secret
    let webhook_routes = Router::new()
        .route("/webhooks/alertmanager", post(alertmanager::handle_alertmanager))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            webhook_auth_middleware,
        ));

    let app = Router::new()
        .route("/health", get(handlers::health).head(handlers::health))
        .merge(api_routes)
        .merge(webhook_routes)
        .fallback(spa_handler)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = std::env::var("CASEMGMT_HTTP_ADDR")
        .unwrap_or_default()
        .trim()
        .to_string();
    let addr = if addr.is_empty() {
        "0.0.0.0:8088".to_string()
    } else {
        addr
    };

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, addr = %addr, "bind");
            std::process::exit(1);
        });

    tracing::info!(addr = %addr, "case-management listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "http");
            std::process::exit(1);
        });
}
