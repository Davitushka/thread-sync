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

// ── Alertmanager webhook → ClickHouse ─────────────────────────────────────────

/// Формат payload, который Alertmanager отправляет на webhook receiver.
/// https://prometheus.io/docs/alerting/latest/configuration/#webhook_config
#[derive(Debug, Deserialize)]
struct AlertmanagerWebhook {
    version: String,
    #[serde(rename = "groupKey")]
    group_key: Option<String>,
    status: String,
    alerts: Vec<AlertmanagerAlert>,
}

#[derive(Debug, Deserialize)]
struct AlertmanagerAlert {
    status: String,
    labels: std::collections::HashMap<String, String>,
    annotations: std::collections::HashMap<String, String>,
    #[serde(rename = "startsAt")]
    starts_at: String,
    #[serde(rename = "endsAt")]
    ends_at: Option<String>,
    #[serde(rename = "generatorURL")]
    generator_url: Option<String>,
    fingerprint: Option<String>,
}

struct AppState {
    pipeline: NormalizationPipeline,
    producer: FutureProducer,
    config: AppConfig,
    http_client: reqwest::Client,
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

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client");

    let state = Arc::new(AppState { pipeline, producer, config: config.clone(), http_client });

    // HTTP API
    let app = Router::new()
        .route("/parse", post(handle_parse))
        .route("/alerts/ingest", post(handle_alerts_ingest))
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

/// POST /alerts/ingest — принимает webhook от Alertmanager и пишет в siem.alerts через ClickHouse HTTP API.
///
/// Alertmanager конфигурация (alertmanager.yaml):
///   receivers:
///     - name: clickhouse-siem
///       webhook_configs:
///         - url: 'http://siem-parser:7000/alerts/ingest'
///           send_resolved: false
async fn handle_alerts_ingest(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> impl IntoResponse {
    let webhook: AlertmanagerWebhook = match serde_json::from_slice(&body) {
        Ok(w) => w,
        Err(e) => {
            error!(error = %e, "Failed to parse Alertmanager webhook payload");
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Invalid webhook JSON: {}", e)
            }))).into_response();
        }
    };

    // Игнорируем resolved уведомления — в siem.alerts пишем только новые алерты.
    let firing: Vec<&AlertmanagerAlert> = webhook.alerts.iter()
        .filter(|a| a.status == "firing")
        .collect();

    if firing.is_empty() {
        return (StatusCode::OK, Json(serde_json::json!({"inserted": 0, "reason": "no firing alerts"}))).into_response();
    }

    // Строим INSERT ... VALUES для ClickHouse HTTP API (формат Values).
    // URL: http://clickhouse:8123/?query=INSERT+INTO+siem.alerts+FORMAT+JSONEachRow
    let ch_url = std::env::var("CLICKHOUSE_URL")
        .unwrap_or_else(|_| "http://clickhouse:8123".to_string());
    let ch_user = std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "siem".to_string());
    // Читаем пароль из файла (Docker secret) или переменной окружения.
    let ch_password = if let Ok(path) = std::env::var("CLICKHOUSE_PASSWORD_FILE") {
        std::fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default())
    } else {
        std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default()
    };

    let mut rows: Vec<serde_json::Value> = Vec::with_capacity(firing.len());

    for alert in &firing {
        let labels = &alert.labels;
        let annotations = &alert.annotations;

        let rule_id = labels.get("rule_id")
            .or_else(|| labels.get("alertname"))
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let rule_title = labels.get("alertname")
            .cloned()
            .unwrap_or_else(|| rule_id.clone());

        // Маппинг severity: Alertmanager может слать warning/critical/high.
        // siem.alerts принимает Enum8('low'=1,'medium'=2,'high'=3,'critical'=4).
        let severity = match labels.get("severity").map(|s| s.as_str()) {
            Some("critical") => "critical",
            Some("high") | Some("warning") => "high",
            Some("medium") => "medium",
            _ => "low",
        };

        let description = annotations.get("description")
            .or_else(|| annotations.get("summary"))
            .cloned()
            .unwrap_or_default();

        let source_ip = labels.get("source_ip").cloned();
        let user_id = labels.get("user_id").cloned();

        let mitre_raw = labels.get("mitre_tags").cloned().unwrap_or_default();
        let mitre_tags: Vec<String> = mitre_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Fingerprint как event_id: если Alertmanager прислал fingerprint — используем его как UUID v5-namespace proxy.
        // Иначе генерируем на основе rule_id + starts_at.
        let alert_id_seed = alert.fingerprint.clone()
            .unwrap_or_else(|| format!("{}-{}", rule_id, alert.starts_at));

        let mut row = serde_json::json!({
            "alert_id": format!("{:0>32}", &alert_id_seed.chars().filter(|c| c.is_alphanumeric()).take(32).collect::<String>()),
            "triggered_at": alert.starts_at,
            "rule_id": rule_id,
            "rule_title": rule_title,
            "severity": severity,
            "description": description,
            "event_ids": [],
            "mitre_tags": mitre_tags,
            "status": "new",
            "notes": ""
        });

        if let Some(ip) = source_ip {
            row["source_ip"] = serde_json::Value::String(ip);
        }
        if let Some(uid) = user_id {
            row["user_id"] = serde_json::Value::String(uid);
        }

        rows.push(row);
    }

    // Сериализуем в JSONEachRow (одна JSON-строка на строку).
    let body_str: String = rows.iter()
        .filter_map(|r| serde_json::to_string(r).ok())
        .collect::<Vec<_>>()
        .join("\n");

    let query_url = format!(
        "{ch_url}/?query=INSERT+INTO+siem.alerts+FORMAT+JSONEachRow&user={ch_user}",
    );

    match state.http_client
        .post(&query_url)
        .basic_auth(&ch_user, Some(&ch_password))
        .header("Content-Type", "application/octet-stream")
        .body(body_str)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            info!(count = firing.len(), "Alerts written to siem.alerts");
            (StatusCode::OK, Json(serde_json::json!({"inserted": firing.len()}))).into_response()
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();
            error!(status, body = %body_text, "ClickHouse INSERT failed");
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": format!("ClickHouse error {}: {}", status, body_text)
            }))).into_response()
        }
        Err(e) => {
            error!(error = %e, "HTTP request to ClickHouse failed");
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": format!("Connection failed: {}", e)
            }))).into_response()
        }
    }
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
