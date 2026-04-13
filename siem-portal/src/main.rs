mod config;
mod event_search;
mod handlers;
mod infrastructure;
mod overview;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::event_search::EventSearchService;
use crate::infrastructure::InfrastructureService;
use crate::overview::OverviewService;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<config::Config>,
    pub http: reqwest::Client,
    pub event_search: EventSearchService,
    pub infrastructure: InfrastructureService,
    pub overview: OverviewService,
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
    let event_search = EventSearchService::new(http.clone(), cfg.clickhouse.clone());
    let infrastructure = InfrastructureService::new(http.clone(), cfg.prometheus.clone());
    let overview = OverviewService::new(http.clone(), cfg.clickhouse.clone());

    let state = AppState {
        cfg: Arc::clone(&cfg),
        http,
        event_search,
        infrastructure,
        overview,
    };

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/", get(handlers::ui_root))
        .route("/assets/{*path}", get(handlers::asset_path))
        .route("/favicon.ico", get(handlers::favicon_noop))
        .route("/api/v1/ui/config", get(handlers::ui_config))
        .route("/api/v1/overview", get(handlers::overview_dashboard))
        .route("/api/v1/infrastructure", get(handlers::infrastructure_dashboard))
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
        .route("/api/v1/proxy/cases", get(handlers::proxy_cases).post(handlers::proxy_create_case))
        .route(
            "/api/v1/proxy/cases/{id}",
            get(handlers::proxy_case_detail).patch(handlers::proxy_patch_case),
        )
        .route(
            "/api/v1/proxy/cases/{id}/timeline",
            post(handlers::proxy_case_timeline),
        )
        .route(
            "/api/v1/proxy/cases/{id}/events",
            post(handlers::proxy_case_event_link),
        )
        .route(
            "/api/v1/proxy/cases/{id}/alerts",
            post(handlers::proxy_case_alert_link),
        )
        .route(
            "/api/v1/proxy/cases/{id}/investigate",
            get(handlers::proxy_investigate),
        )
        .route("/api/v1/proxy/correlator/stats", get(handlers::proxy_correlator_stats))
        .route("/api/v1/proxy/correlator/rules", get(handlers::proxy_correlator_rules))
        .route("/api/v1/events/search", get(handlers::search_events))
        .route("/api/v1/events/{id}", get(handlers::get_event))
        .route(
            "/api/v1/entities/{kind}/{value}/context",
            get(handlers::entity_context),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .fallback(handlers::spa_fallback)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&cfg.bind).await.map_err(|e| {
        anyhow::anyhow!(
            "bind {} failed: {} — порт занят или адрес неверный. Закройте другой процесс на этом порту или задайте SIEM_PORTAL_ADDR, например 127.0.0.1:8092",
            cfg.bind,
            e
        )
    })?;
    let port = cfg.bind.rsplit(':').next().unwrap_or("8091");
    tracing::info!(
        bind = %cfg.bind,
        "siem-portal listening — UI: http://127.0.0.1:{}/  (если в браузере не открывается localhost — используйте именно 127.0.0.1; не https)",
        port
    );

    axum::serve(listener, app).await?;
    Ok(())
}
