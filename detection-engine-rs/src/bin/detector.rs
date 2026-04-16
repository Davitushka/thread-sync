use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::extract::State;
use axum::routing::get;
use axum::Router;
use prometheus::{Encoder, TextEncoder};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::Message;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use detection_engine_rs::alert::Alert;
use detection_engine_rs::engine::Engine;
use detection_engine_rs::rules::brute_force::BruteForceRule;
use detection_engine_rs::rules::privilege_escalation::PrivilegeEscalationRule;
use detection_engine_rs::rules::rate_limit::RateLimitEvasionRule;
use detection_engine_rs::rules::sql_injection::SQLInjectionRule;
use detection_engine_rs::rules::{Rule, StatefulRule};
use detection_engine_rs::state_store::RedisStore;

fn get_env(key: &str, fallback: &str) -> String {
    env::var(key).unwrap_or_else(|_| fallback.to_string())
}

#[derive(Clone)]
struct DetectorState {
    redis_store: Option<Arc<RedisStore>>,
    kafka_broker: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .json()
        .init();

    let kafka_brokers = get_env("KAFKA_BOOTSTRAP_SERVERS", "redpanda:9092");
    let kafka_topic = get_env("KAFKA_TOPIC", "siem.events");
    let kafka_group = get_env("KAFKA_GROUP_ID", "detection-engine");
    let redis_addr = get_env("REDIS_ADDR", "redis:6379");
    let redis_password = get_env("REDIS_PASSWORD", "");
    let metrics_addr = get_env("METRICS_ADDR", ":9110");

    let listen_addr = if metrics_addr.starts_with(':') {
        format!("0.0.0.0{}", metrics_addr)
    } else {
        metrics_addr.clone()
    };

    info!(
        kafka = %kafka_brokers,
        topic = %kafka_topic,
        redis = %redis_addr,
        "starting detection engine",
    );


    let (state_store, redis_arc): (
        Option<Arc<dyn detection_engine_rs::state_store::StateStore>>,
        Option<Arc<RedisStore>>,
    ) = match RedisStore::new(&redis_addr, &redis_password, 0).await {
        Ok(store) => {
            if let Err(e) = store.ping().await {
                warn!(%e, "Redis unavailable, running in stateless mode");
                (None, None)
            } else {
                info!("connected to Redis");
                let arc = Arc::new(store);
                (Some(arc.clone()), Some(arc))
            }
        }
        Err(e) => {
            warn!(%e, "Redis connection failed, running in stateless mode");
            (None, None)
        }
    };

    let stateless_rules: Vec<Box<dyn Rule>> = vec![Box::new(SQLInjectionRule::new())];
    let stateful_rules: Vec<Box<dyn StatefulRule>> = vec![
        Box::new(BruteForceRule::new()),
        Box::new(RateLimitEvasionRule::new()),
        Box::new(PrivilegeEscalationRule::new()),
    ];

    let (alert_tx, mut alert_rx) = mpsc::channel::<Alert>(1000);

    let engine = Arc::new(Engine::new(
        stateless_rules,
        stateful_rules,
        state_store,
        alert_tx,
    ));
    let rule_count = engine.rule_count().await;
    info!(rules = rule_count, "detection engine initialized");

    tokio::spawn(async move {
        while let Some(alert) = alert_rx.recv().await {
            warn!(
                rule_id = alert.rule_id,
                severity = %alert.severity,
                description = alert.description,
                mitre_tags = ?alert.mitre_tags,
                fired_at = %alert.fired_at,
                "ALERT",
            );
        }
    });

    let engine_clone = engine.clone();
    let brokers = kafka_brokers.clone();
    let topic = kafka_topic.clone();
    let group = kafka_group.clone();
    tokio::spawn(async move {
        if let Err(e) = run_kafka_consumer(&brokers, &topic, &group, engine_clone).await {
            error!(%e, "Kafka consumer error");
        }
    });

    let detector_state = DetectorState {
        redis_store: redis_arc,
        kafka_broker: kafka_brokers
            .split(',')
            .next()
            .unwrap_or("")
            .to_string(),
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/ready", get(ready_handler))
        .with_state(detector_state);

    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;
    info!(addr = %listen_addr, "metrics server starting");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    #[cfg(unix)]
    tokio::select! {
        _ = ctrl_c => {}
        _ = sigterm.recv() => {}
    }
    #[cfg(not(unix))]
    ctrl_c.await.ok();
    info!("shutting down detection engine...");
}

async fn run_kafka_consumer(
    brokers: &str,
    topic: &str,
    group: &str,
    engine: Arc<Engine>,
) -> Result<()> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", group)
        .set("auto.offset.reset", "latest")
        .set("enable.auto.commit", "false")
        .set("fetch.min.bytes", "1")
        .set("fetch.message.max.bytes", "10485760")
        .set("fetch.wait.max.ms", "500")
        .create()?;

    consumer.subscribe(&[topic])?;
    info!(brokers, topic, group, "Kafka consumer started");

    loop {
        match consumer.recv().await {
            Ok(msg) => {
                if let Some(payload) = msg.payload() {
                    if !payload.is_empty() {
                        engine.process_raw(payload).await;
                    }
                }
                if let Err(e) = consumer.commit_message(&msg, CommitMode::Async) {
                    warn!(%e, "commit failed");
                }
            }
            Err(e) => {
                warn!(%e, "Kafka fetch error");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

async fn health_handler() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok", "service": "detector"}))
}

async fn metrics_handler() -> impl axum::response::IntoResponse {
    let encoder = TextEncoder::new();
    let families = prometheus::gather();
    let mut buf = Vec::new();
    encoder.encode(&families, &mut buf).unwrap();
    (
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        buf,
    )
}

async fn ready_handler(
    State(state): State<DetectorState>,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    if let Some(ref store) = state.redis_store {
        if store.ping().await.is_err() {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(
                    serde_json::json!({"status": "not_ready", "reason": "redis_unavailable"}),
                ),
            );
        }
    }

    match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect(&state.kafka_broker),
    )
    .await
    {
        Ok(Ok(_)) => (
            axum::http::StatusCode::OK,
            axum::Json(serde_json::json!({"status": "ready"})),
        ),
        _ => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(
                serde_json::json!({"status": "not_ready", "reason": "kafka_unavailable"}),
            ),
        ),
    }
}
