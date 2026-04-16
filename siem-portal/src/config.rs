use std::{fs, time::Duration};

/// Интервалы серверного опроса upstream по категориям тем WebSocket (мс).
#[derive(Clone, Debug)]
pub struct RealtimePolicy {
    /// Базовый интервал (`SIEM_PORTAL_REALTIME_POLL_MS`), если тема не попала в категорию.
    pub default_ms: u64,
    pub ui_config_ms: u64,
    pub stack_status_ms: u64,
    /// Overview / infrastructure / operations / data-quality (ClickHouse + Prom тяжёлые).
    pub dashboards_ms: u64,
    /// Алерты, кейсы, поиск событий, correlator stats — чаще меняются.
    pub hot_ms: u64,
}

impl RealtimePolicy {
    pub fn from_env_default(default_ms: u64) -> Self {
        let d = default_ms.clamp(1_000, 300_000);
        Self {
            default_ms: d,
            ui_config_ms: env_ms("SIEM_PORTAL_REALTIME_MS_UI", 120_000),
            stack_status_ms: env_ms("SIEM_PORTAL_REALTIME_MS_STACK", 10_000),
            dashboards_ms: env_ms("SIEM_PORTAL_REALTIME_MS_DASHBOARDS", 10_000),
            hot_ms: env_ms("SIEM_PORTAL_REALTIME_MS_HOT", 3_000),
        }
    }

    pub fn poll_ms_for_topic(&self, topic: &str) -> u64 {
        if topic == "ui.config" {
            return self.ui_config_ms;
        }
        if topic == "stack.status" {
            return self.stack_status_ms;
        }
        if topic.starts_with("overview:h:")
            || topic.starts_with("infrastructure:h:")
            || topic.starts_with("operations:h:")
            || topic.starts_with("data_quality:h:")
        {
            return self.dashboards_ms;
        }
        if topic == "correlator.rules" {
            return self.dashboards_ms;
        }
        if topic == "alerts.overview"
            || topic == "alertmanager.alerts"
            || topic == "detections.overview"
            || topic == "correlator.stats"
            || topic.starts_with("cases.")
            || topic.starts_with("case.")
            || topic.starts_with("events.search")
            || topic == "events.search"
            || topic.starts_with("event.detail:")
            || topic.starts_with("entity.context:")
        {
            return self.hot_ms;
        }
        self.default_ms
    }
}

fn env_ms(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
        .clamp(500, 300_000)
}

#[derive(Clone)]
pub struct Config {
    pub bind: String,
    pub http_timeout: Duration,
    pub case_management: String,
    pub prometheus: String,
    pub alertmanager: String,
    pub correlator: String,
    pub grafana: String,
    pub clickhouse: ClickHouseConfig,
    /// URLs shown in the browser (host-facing). Defaults to localhost ports if unset.
    pub public: PublicLinks,
    /// Базовый интервал realtime и тиры `SIEM_PORTAL_REALTIME_MS_*`.
    pub realtime_policy: RealtimePolicy,
}

#[derive(Debug, Clone)]
pub struct ClickHouseConfig {
    pub url: String,
    pub user: String,
    pub database: String,
    pub password: String,
}

#[derive(Clone, serde::Serialize)]
pub struct PublicLinks {
    pub grafana: String,
    pub prometheus: String,
    pub alertmanager: String,
    pub case_management: String,
    pub siem_overview_dashboard: String,
    /// Host-facing Vector HTTP ingest base (`http://host:8080`); POST NDJSON to `{base}/logs`.
    pub vector_http_base: String,
    /// Redpanda admin API (often `:9644`) for rpk / debugging.
    pub redpanda_admin: String,
}

impl Config {
    pub fn from_env() -> Self {
        let case_management = env_trim("SIEM_PORTAL_CASEMGMT_URL", "http://case-management:8088");
        let prometheus = env_trim("SIEM_PORTAL_PROMETHEUS_URL", "http://prometheus:9090");
        let alertmanager = env_trim("SIEM_PORTAL_ALERTMANAGER_URL", "http://alertmanager:9093");
        let correlator = env_trim("SIEM_PORTAL_CORRELATOR_URL", "http://correlator:9111");
        let grafana = env_trim("SIEM_PORTAL_GRAFANA_URL", "http://grafana:3000");
        let clickhouse_url = env_trim("SIEM_PORTAL_CLICKHOUSE_URL", "http://clickhouse:8123");
        let clickhouse_user = env_trim("SIEM_PORTAL_CLICKHOUSE_USER", "siem");
        let clickhouse_database = env_trim("SIEM_PORTAL_CLICKHOUSE_DATABASE", "siem");
        let clickhouse_password = read_secret(
            "SIEM_PORTAL_CLICKHOUSE_PASSWORD_FILE",
            "SIEM_PORTAL_CLICKHOUSE_PASSWORD",
        );

        let public_grafana = env_or("SIEM_PORTAL_PUBLIC_GRAFANA", "http://localhost:3000");
        let public_prometheus = env_or("SIEM_PORTAL_PUBLIC_PROMETHEUS", "http://localhost:9090");
        let public_alertmanager = env_or("SIEM_PORTAL_PUBLIC_ALERTMANAGER", "http://localhost:9093");
        let public_cases = env_or("SIEM_PORTAL_PUBLIC_CASEMGMT", "http://localhost:8088");
        let overview = env_or(
            "SIEM_PORTAL_PUBLIC_GRAFANA_OVERVIEW",
            "http://localhost:3000/d/siem-overview/siem-lite-obzor",
        );
        let public_vector_http = env_or("SIEM_PORTAL_PUBLIC_VECTOR_HTTP", "http://localhost:8080");
        let public_redpanda_admin = env_or("SIEM_PORTAL_PUBLIC_REDPANDA_ADMIN", "http://localhost:9644");

        let bind = env_trim("SIEM_PORTAL_ADDR", "0.0.0.0:8091");
        let timeout_secs: u64 = std::env::var("SIEM_PORTAL_HTTP_TIMEOUT_SEC")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);
        let realtime_default_ms: u64 = std::env::var("SIEM_PORTAL_REALTIME_POLL_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5_000)
            .clamp(1_000, 300_000);
        let realtime_policy = RealtimePolicy::from_env_default(realtime_default_ms);

        Self {
            bind,
            http_timeout: Duration::from_secs(timeout_secs.max(1)),
            case_management: trim_slash(case_management),
            prometheus: trim_slash(prometheus),
            alertmanager: trim_slash(alertmanager),
            correlator: trim_slash(correlator),
            grafana: trim_slash(grafana),
            clickhouse: ClickHouseConfig {
                url: trim_slash(clickhouse_url),
                user: clickhouse_user,
                database: clickhouse_database,
                password: clickhouse_password,
            },
            public: PublicLinks {
                grafana: trim_slash(public_grafana),
                prometheus: trim_slash(public_prometheus),
                alertmanager: trim_slash(public_alertmanager),
                case_management: trim_slash(public_cases),
                siem_overview_dashboard: trim_slash(overview),
                vector_http_base: trim_slash(public_vector_http),
                redpanda_admin: trim_slash(public_redpanda_admin),
            },
            realtime_policy,
        }
    }
}

fn env_trim(key: &str, default: &str) -> String {
    std::env::var(key)
        .unwrap_or_else(|_| default.to_string())
        .trim()
        .to_string()
}

fn env_or(key: &str, default: &str) -> String {
    let v = std::env::var(key).unwrap_or_default();
    if v.trim().is_empty() {
        default.to_string()
    } else {
        v.trim().to_string()
    }
}

fn read_secret(file_key: &str, value_key: &str) -> String {
    let file = std::env::var(file_key).unwrap_or_default();
    if !file.trim().is_empty() {
        if let Ok(secret) = fs::read_to_string(file.trim()) {
            return secret.trim().to_string();
        }
    }
    env_or(value_key, "")
}

fn trim_slash(mut s: String) -> String {
    while s.ends_with('/') {
        s.pop();
    }
    s
}
