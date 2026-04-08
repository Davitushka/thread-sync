mod config;
mod handlers;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<config::Config>,
    pub http: reqwest::Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("siem_portal=info,info")),
        )
        .init();

    let cfg = Arc::new(config::Config::from_env());
    let http = reqwest::Client::builder()
        .use_rustls_tls()
        .pool_max_idle_per_host(8)
        .build()?;

    let state = AppState {
        cfg: Arc::clone(&cfg),
        http,
    };

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/", get(handlers::ui_root))
        .route("/favicon.ico", get(handlers::favicon_noop))
        .route("/api/v1/ui/config", get(handlers::ui_config))
        .route("/api/v1/stack/status", get(handlers::stack_status))
        .route(
            "/api/v1/proxy/prometheus/query",
            get(handlers::proxy_prometheus_query),
        )
        .route(
            "/api/v1/proxy/prometheus/query_range",
            get(handlers::proxy_prometheus_query_range),
        )
        .route(
            "/api/v1/proxy/alertmanager/v2/alerts",
            get(handlers::proxy_alertmanager_alerts),
        )
        .route(
            "/api/v1/proxy/alertmanager/v2/status",
            get(handlers::proxy_alertmanager_status),
        )
        .route("/api/v1/proxy/cases", get(handlers::proxy_cases))
        .route(
            "/api/v1/proxy/cases/:id/investigate",
            get(handlers::proxy_investigate),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&cfg.bind).await?;
    tracing::info!(addr = %cfg.bind, "siem-portal listening");

    axum::serve(listener, app).await?;
    Ok(())
}
