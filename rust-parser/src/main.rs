//! siem-parser — высокопроизводительный сервис парсинга и нормализации логов.
//!
//! Принимает события по HTTP POST /parse (батч JSON array),
//! применяет парсинг → PII маскирование → GeoIP обогащение,
//! отправляет нормализованные события в Kafka/Redpanda.

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use prometheus::{Encoder, TextEncoder};
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use siem_parser::{
    config::AppConfig,
    enrichment::{Enricher, EnrichmentConfig},
    metrics,
    normalizer::NormalizationPipeline,
};

struct AppState {
    pipeline: NormalizationPipeline,
    producer: FutureProducer,
    config: AppConfig,
}

#[derive(Deserialize)]
struct ParseRequest {
    events: Vec<RawEvent>,
}

#[derive(Deserialize)]
struct RawEvent {
    raw: String,
    source_type: String,
    host: String,
}

#[derive(Serialize)]
struct ParseResponse {
    processed: usize,
    errors: usize,
    error_details: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Инициализация structured logging в JSON
    tracing_subscriber::registry()
        .with(fmt::layer().json())
        .with(EnvFilter::from_default_env().add_directive("siem_parser=info".parse()?))
        .init();

    let config = AppConfig::from_env().unwrap_or_else(|e| {
        error!("Config error: {}. Using defaults.", e);
        // Дефолтная конфигурация для локального запуска
        serde_json::from_value(serde_json::json!({
            "server": { "host": "0.0.0.0", "port": 7000, "metrics_port": 9100, "workers": 4 },
            "kafka": { "bootstrap_servers": "localhost:9092", "topic": "siem.events", "dlq_topic": "siem.events.dlq" },
            "geoip": { "city_db_path": "/etc/geoip/GeoLite2-City.mmdb", "asn_db_path": "/etc/geoip/GeoLite2-ASN.mmdb" },
            "processing": { "max_event_size_bytes": 1048576, "channel_capacity": 100000, "enable_pii_masking": true, "enable_geoip": true }
        })).expect("default config is valid")
    });

    metrics::init();

    // Инициализация обогащения
    let enricher = Enricher::new(&EnrichmentConfig {
        geoip_city_db_path: config.geoip.city_db_path.clone(),
        geoip_asn_db_path: config.geoip.asn_db_path.clone(),
        cache_size: config.geoip.cache_size,
        ..Default::default()
    });

    let pipeline = NormalizationPipeline::new(
        enricher,
        config.processing.enable_pii_masking,
        config.processing.enable_geoip,
    );

    // Kafka producer
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &config.kafka.bootstrap_servers)
        .set("enable.idempotence", "true")
        .set("acks", "all")
        .set("retries", "2147483647")
        .set("max.in.flight.requests.per.connection", "5")
        .set("linger.ms", config.kafka.linger_ms.to_string())
        .set("batch.size", "1048576")
        .set("compression.type", "snappy")
        .create()?;

    let state = Arc::new(AppState { pipeline, producer, config: config.clone() });

    // HTTP API
    let app = Router::new()
        .route("/parse", post(handle_parse))
        .route("/health", get(handle_health))
        .route("/ready", get(handle_ready))
        .route("/metrics", get(handle_metrics))
        .with_state(state);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    info!(addr = %addr, "siem-parser starting");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("siem-parser stopped gracefully");
    Ok(())
}

async fn handle_parse(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> impl IntoResponse {
    let request: ParseRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Invalid JSON: {}", e)
            }))).into_response();
        }
    };

    let mut processed = 0usize;
    let mut errors = 0usize;
    let mut error_details = Vec::new();

    for raw_event in request.events {
        let raw_bytes = Bytes::from(raw_event.raw.into_bytes());
        match state.pipeline.process(raw_bytes, &raw_event.source_type, &raw_event.host) {
            Ok(normalized) => {
                let payload = match serde_json::to_vec(&normalized) {
                    Ok(p) => p,
                    Err(e) => {
                        errors += 1;
                        error_details.push(format!("Serialization error: {}", e));
                        continue;
                    }
                };

                let topic = state.config.kafka.topic.clone();
                let partition_key = normalized.source_ip.clone().unwrap_or_else(|| normalized.event_id.to_string());

                let record = FutureRecord::to(&topic)
                    .payload(&payload)
                    .key(&partition_key);

                match state.producer.send(record, Duration::from_secs(5)).await {
                    Ok(_) => {
                        metrics::KAFKA_PRODUCED_TOTAL
                            .with_label_values(&[&topic, "ok"])
                            .inc();
                        processed += 1;
                    }
                    Err((e, _)) => {
                        error!(error = %e, "Kafka produce failed");
                        metrics::KAFKA_PRODUCED_TOTAL
                            .with_label_values(&[&topic, "error"])
                            .inc();
                        errors += 1;
                        error_details.push(format!("Kafka error: {}", e));
                    }
                }
            }
            Err(e) => {
                errors += 1;
                error_details.push(format!("Parse error: {}", e));
            }
        }
    }

    (StatusCode::OK, Json(ParseResponse { processed, errors, error_details })).into_response()
}

async fn handle_health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn handle_ready() -> impl IntoResponse {
    // В продакшне проверяем соединение с Kafka
    Json(serde_json::json!({ "status": "ready" }))
}

async fn handle_metrics() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap_or_default();
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        buffer,
    )
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C"),
        _ = terminate => info!("Received SIGTERM"),
    }
}
