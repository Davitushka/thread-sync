use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use axum::extract::State;
use axum::routing::get;
use axum::Router;
use chrono::Utc;
use prometheus::{Counter, Encoder, Opts, TextEncoder};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::Message;
use serde::Serialize;
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

// ── Config ──────────────────────────────────────────────────────────────────────

struct CorrelatorConfig {
    http_addr: String,
    redis_addr: String,
    redis_password: String,
    redis_db: i64,
    kafka_brokers: String,
    kafka_topic: String,
    kafka_group_id: String,
    alertmanager_url: String,
    brute_force_threshold: i64,
    brute_force_window: Duration,
    rate_limit_threshold: i64,
    rate_limit_window: Duration,
    priv_esc_threshold: i64,
    priv_esc_window: Duration,
    shutdown_timeout: Duration,
    log_level: String,
}

impl CorrelatorConfig {
    fn load_from_env() -> Result<Self> {
        let cfg = Self {
            http_addr: get_env("CORRELATOR_HTTP_ADDR", ":9111"),
            redis_addr: get_env("REDIS_ADDR", "redis:6379"),
            redis_password: get_env("REDIS_PASSWORD", ""),
            redis_db: get_env("REDIS_DB", "0").parse()?,
            kafka_brokers: get_env("KAFKA_BOOTSTRAP_SERVERS", "redpanda:9092"),
            kafka_topic: get_env("KAFKA_TOPIC", "siem.events"),
            kafka_group_id: get_env("KAFKA_GROUP_ID", "correlator"),
            alertmanager_url: get_env("ALERTMANAGER_URL", "http://alertmanager:9093"),
            brute_force_threshold: get_env("BRUTE_FORCE_THRESHOLD", "10").parse()?,
            brute_force_window: Duration::from_secs(120),
            rate_limit_threshold: get_env("RATE_LIMIT_THRESHOLD", "500").parse()?,
            rate_limit_window: Duration::from_secs(60),
            priv_esc_threshold: 3,
            priv_esc_window: Duration::from_secs(300),
            shutdown_timeout: Duration::from_secs(30),
            log_level: get_env("LOG_LEVEL", "info"),
        };
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<()> {
        if self.http_addr.is_empty() {
            bail!("HTTPAddr is required");
        }
        if self.redis_addr.is_empty() {
            bail!("RedisAddr is required");
        }
        if self.kafka_brokers.is_empty() {
            bail!("KafkaBrokers is required");
        }
        if self.brute_force_threshold < 1 {
            bail!("BruteForceThreshold must be >= 1");
        }
        if self.rate_limit_threshold < 1 {
            bail!("RateLimitThreshold must be >= 1");
        }
        Ok(())
    }
}

fn get_env(key: &str, fallback: &str) -> String {
    env::var(key).unwrap_or_else(|_| fallback.to_string())
}

// ── Alertmanager types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AlertmanagerAlert {
    labels: HashMap<String, String>,
    annotations: HashMap<String, String>,
    #[serde(rename = "startsAt")]
    starts_at: chrono::DateTime<Utc>,
}

// ── Shared state for axum handlers ──────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    engine: Arc<Engine>,
    redis_store: Arc<RedisStore>,
    alert_tx: mpsc::Sender<Alert>,
    cfg: Arc<CorrelatorConfig>,
}

// ── main ────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = CorrelatorConfig::load_from_env()?;

    let filter = tracing_subscriber::EnvFilter::try_new(&cfg.log_level)
        .unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .init();

    let listen_addr = if cfg.http_addr.starts_with(':') {
        format!("0.0.0.0{}", cfg.http_addr)
    } else {
        cfg.http_addr.clone()
    };

    info!(
        http_addr = %cfg.http_addr,
        redis = %cfg.redis_addr,
        kafka = %cfg.kafka_brokers,
        alertmanager = %cfg.alertmanager_url,
        "starting correlator",
    );

    let redis_store = Arc::new(
        RedisStore::new(&cfg.redis_addr, &cfg.redis_password, cfg.redis_db).await?,
    );
    if let Err(e) = redis_store.ping().await {
        warn!(addr = %cfg.redis_addr, %e, "Redis unavailable at startup — stateful rules disabled");
    } else {
        info!(addr = %cfg.redis_addr, "Redis connected");
    }

    let (alert_tx, alert_rx) = mpsc::channel::<Alert>(1000);

    let mut brute_force = BruteForceRule::new();
    brute_force.threshold = cfg.brute_force_threshold;
    brute_force.window = cfg.brute_force_window;

    let mut rate_limit = RateLimitEvasionRule::new();
    rate_limit.threshold = cfg.rate_limit_threshold;
    rate_limit.window = cfg.rate_limit_window;

    let mut priv_esc = PrivilegeEscalationRule::new();
    priv_esc.threshold = cfg.priv_esc_threshold;

    let sqli = SQLInjectionRule::new();

    let stateless_rules: Vec<Box<dyn Rule>> = vec![Box::new(sqli)];
    let stateful_rules: Vec<Box<dyn StatefulRule>> = vec![
        Box::new(brute_force),
        Box::new(rate_limit),
        Box::new(priv_esc),
    ];

    let state_store: Arc<dyn detection_engine_rs::state_store::StateStore> = redis_store.clone();
    let engine = Arc::new(Engine::new(
        stateless_rules,
        stateful_rules,
        Some(state_store),
        alert_tx.clone(),
    ));
    let rule_count = engine.rule_count().await;
    info!(rules = rule_count, "detection engine ready");

    let cfg = Arc::new(cfg);

    // Correlator-specific metrics
    let corr_events_processed = Counter::with_opts(Opts::new(
        "correlator_events_processed_total",
        "Total events consumed from Kafka by correlator",
    ))?;
    prometheus::register(Box::new(corr_events_processed.clone()))?;

    let corr_parse_errors = Counter::with_opts(Opts::new(
        "correlator_parse_errors_total",
        "Total JSON parse errors in correlator consumer",
    ))?;
    prometheus::register(Box::new(corr_parse_errors.clone()))?;

    let corr_alerts_forwarded = Counter::with_opts(Opts::new(
        "correlator_alerts_forwarded_total",
        "Total alerts forwarded to Alertmanager",
    ))?;
    prometheus::register(Box::new(corr_alerts_forwarded.clone()))?;

    // Alert forwarder
    let am_url = cfg.alertmanager_url.clone();
    let fwd_counter = corr_alerts_forwarded.clone();
    tokio::spawn(run_alert_forwarder(alert_rx, am_url, fwd_counter));

    // Kafka consumer
    let kafka_engine = engine.clone();
    let kafka_brokers = cfg.kafka_brokers.clone();
    let kafka_topic = cfg.kafka_topic.clone();
    let kafka_group = cfg.kafka_group_id.clone();
    let events_ctr = corr_events_processed.clone();
    let parse_ctr = corr_parse_errors.clone();
    tokio::spawn(async move {
        run_kafka_consumer(
            &kafka_brokers,
            &kafka_topic,
            &kafka_group,
            kafka_engine,
            events_ctr,
            parse_ctr,
        )
        .await;
    });

    // Stats reporter
    let stats_engine = engine.clone();
    let stats_tx = alert_tx.clone();
    tokio::spawn(async move {
        run_stats_reporter(stats_engine, stats_tx).await;
    });

    // HTTP server
    let app_state = AppState {
        engine,
        redis_store,
        alert_tx,
        cfg: cfg.clone(),
    };

    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/metrics", get(handle_metrics))
        .route("/ready", get(handle_ready))
        .route("/api/v1/stats", get(handle_stats))
        .route("/api/v1/rules", get(handle_rules))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;
    info!(addr = %listen_addr, "HTTP server starting");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("correlator stopped");
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
    info!("received shutdown signal");
}

// ── Alert Forwarder ─────────────────────────────────────────────────────────────

async fn run_alert_forwarder(
    mut rx: mpsc::Receiver<Alert>,
    alertmanager_url: String,
    forwarded_counter: Counter,
) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client");

    while let Some(alert) = rx.recv().await {
        match forward_to_alertmanager(&client, &alertmanager_url, &alert).await {
            Ok(()) => {
                forwarded_counter.inc();
                info!(
                    rule_id = alert.rule_id,
                    severity = %alert.severity,
                    "alert forwarded",
                );
            }
            Err(e) => {
                warn!(
                    rule_id = alert.rule_id,
                    %e,
                    "failed to forward alert to Alertmanager",
                );
            }
        }
    }
}

async fn forward_to_alertmanager(
    client: &reqwest::Client,
    base_url: &str,
    alert: &Alert,
) -> Result<()> {
    let mut labels = HashMap::new();
    labels.insert("alertname".into(), alert.rule_title.clone());
    labels.insert("rule_id".into(), alert.rule_id.clone());
    labels.insert("severity".into(), alert.severity.to_string());
    if let Some(ref ip) = alert.source_ip {
        labels.insert("source_ip".into(), ip.clone());
    }
    if let Some(ref uid) = alert.user_id {
        labels.insert("user_id".into(), uid.clone());
    }

    let mut annotations = HashMap::new();
    annotations.insert("description".into(), alert.description.clone());
    annotations.insert("mitre_tags".into(), format!("{:?}", alert.mitre_tags));

    let am_alert = AlertmanagerAlert {
        labels,
        annotations,
        starts_at: alert.fired_at,
    };

    let url = format!("{}/api/v2/alerts", base_url);
    let resp = client
        .post(&url)
        .json(&[&am_alert])
        .send()
        .await?;

    if resp.status().as_u16() >= 300 {
        bail!("alertmanager responded {}", resp.status());
    }
    Ok(())
}

// ── Kafka Consumer ──────────────────────────────────────────────────────────────

async fn run_kafka_consumer(
    brokers: &str,
    topic: &str,
    group: &str,
    engine: Arc<Engine>,
    events_counter: Counter,
    parse_errors_counter: Counter,
) {
    let consumer: StreamConsumer = match ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", group)
        .set("auto.offset.reset", "latest")
        .set("enable.auto.commit", "false")
        .set("fetch.min.bytes", "1")
        .set("fetch.message.max.bytes", "10485760")
        .set("fetch.wait.max.ms", "500")
        .create()
    {
        Ok(c) => c,
        Err(e) => {
            error!(%e, "failed to create Kafka consumer");
            return;
        }
    };

    if let Err(e) = consumer.subscribe(&[topic]) {
        error!(%e, "failed to subscribe to Kafka topic");
        return;
    }

    info!(brokers, topic, group, "Kafka consumer started");

    loop {
        match consumer.recv().await {
            Ok(msg) => {
                let payload = match msg.payload() {
                    Some(p) if !p.is_empty() => p,
                    _ => {
                        let _ = consumer.commit_message(&msg, CommitMode::Async);
                        continue;
                    }
                };

                let event: detection_engine_rs::event::Event =
                    match serde_json::from_slice(payload) {
                        Ok(e) => e,
                        Err(e) => {
                            parse_errors_counter.inc();
                            let preview_len = payload.len().min(200);
                            let preview = std::str::from_utf8(&payload[..preview_len])
                                .unwrap_or("<binary>");
                            warn!(%e, raw = preview, "JSON parse error");
                            let _ = consumer.commit_message(&msg, CommitMode::Async);
                            continue;
                        }
                    };

                engine.process(&event).await;
                events_counter.inc();

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

// ── Stats Reporter ──────────────────────────────────────────────────────────────

async fn run_stats_reporter(engine: Arc<Engine>, alert_tx: mpsc::Sender<Alert>) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        let rule_count = engine.rule_count().await;
        let pending_alerts = alert_tx.max_capacity() - alert_tx.capacity();
        info!(
            active_rules = rule_count,
            pending_alerts,
            "correlator stats",
        );
    }
}

// ── HTTP Handlers ───────────────────────────────────────────────────────────────

async fn handle_health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok", "service": "correlator"}))
}

async fn handle_metrics() -> impl axum::response::IntoResponse {
    let encoder = TextEncoder::new();
    let families = prometheus::gather();
    let mut buf = Vec::new();
    encoder.encode(&families, &mut buf).unwrap();
    (
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        buf,
    )
}

async fn handle_ready(
    State(state): State<AppState>,
) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    if state.redis_store.ping().await.is_err() {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(
                serde_json::json!({"status": "not_ready", "reason": "redis_unavailable"}),
            ),
        );
    }

    let broker = state
        .cfg
        .kafka_brokers
        .split(',')
        .next()
        .unwrap_or("")
        .to_string();
    match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect(&broker),
    )
    .await
    {
        Ok(Ok(_)) => (
            axum::http::StatusCode::OK,
            axum::Json(
                serde_json::json!({"status": "ready", "service": "correlator"}),
            ),
        ),
        _ => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(
                serde_json::json!({"status": "not_ready", "reason": "kafka_unavailable"}),
            ),
        ),
    }
}

async fn handle_stats(
    State(state): State<AppState>,
) -> axum::Json<serde_json::Value> {
    let rules_count = state.engine.rule_count().await;
    let pending = state.alert_tx.max_capacity() - state.alert_tx.capacity();
    let capacity = state.alert_tx.max_capacity();

    axum::Json(serde_json::json!({
        "rules_count": rules_count,
        "pending_alerts": pending,
        "alert_capacity": capacity,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

async fn handle_rules(
    State(state): State<AppState>,
) -> axum::Json<serde_json::Value> {
    let cfg = &state.cfg;
    let rules = serde_json::json!([
        {
            "id": "brute_force_api",
            "type": "stateful",
            "threshold": cfg.brute_force_threshold,
            "window": format_dur(cfg.brute_force_window),
        },
        {
            "id": "rate_limit_evasion",
            "type": "stateful",
            "threshold": cfg.rate_limit_threshold,
            "window": format_dur(cfg.rate_limit_window),
        },
        {
            "id": "privilege_escalation_attempt",
            "type": "stateful",
            "threshold": cfg.priv_esc_threshold,
            "window": format_dur(cfg.priv_esc_window),
        },
        {
            "id": "sql_injection_attempt",
            "type": "stateless",
        },
    ]);
    axum::Json(rules)
}

fn format_dur(d: Duration) -> String {
    let total = d.as_secs();
    let m = total / 60;
    let s = total % 60;
    if m > 0 && s > 0 {
        format!("{}m{}s", m, s)
    } else if m > 0 {
        format!("{}m0s", m)
    } else {
        format!("{}s", s)
    }
}
