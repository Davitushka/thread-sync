use std::time::Duration;

#[derive(Clone)]
pub struct Config {
    pub bind: String,
    pub http_timeout: Duration,
    pub case_management: String,
    pub prometheus: String,
    pub alertmanager: String,
    pub grafana: String,
    /// URLs shown in the browser (host-facing). Defaults to localhost ports if unset.
    pub public: PublicLinks,
}

#[derive(Clone, serde::Serialize)]
pub struct PublicLinks {
    pub grafana: String,
    pub prometheus: String,
    pub alertmanager: String,
    pub case_management: String,
    pub siem_overview_dashboard: String,
}

impl Config {
    pub fn from_env() -> Self {
        let case_management = env_trim("SIEM_PORTAL_CASEMGMT_URL", "http://case-management:8088");
        let prometheus = env_trim("SIEM_PORTAL_PROMETHEUS_URL", "http://prometheus:9090");
        let alertmanager = env_trim("SIEM_PORTAL_ALERTMANAGER_URL", "http://alertmanager:9093");
        let grafana = env_trim("SIEM_PORTAL_GRAFANA_URL", "http://grafana:3000");

        let public_grafana = env_or("SIEM_PORTAL_PUBLIC_GRAFANA", "http://localhost:3000");
        let public_prometheus = env_or("SIEM_PORTAL_PUBLIC_PROMETHEUS", "http://localhost:9090");
        let public_alertmanager = env_or("SIEM_PORTAL_PUBLIC_ALERTMANAGER", "http://localhost:9093");
        let public_cases = env_or("SIEM_PORTAL_PUBLIC_CASEMGMT", "http://localhost:8088");
        let overview = env_or(
            "SIEM_PORTAL_PUBLIC_GRAFANA_OVERVIEW",
            "http://localhost:3000/d/siem-overview/siem-lite-obzor",
        );

        let bind = env_trim("SIEM_PORTAL_ADDR", "0.0.0.0:8091");
        let timeout_secs: u64 = std::env::var("SIEM_PORTAL_HTTP_TIMEOUT_SEC")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        Self {
            bind,
            http_timeout: Duration::from_secs(timeout_secs.max(1)),
            case_management: trim_slash(case_management),
            prometheus: trim_slash(prometheus),
            alertmanager: trim_slash(alertmanager),
            grafana: trim_slash(grafana),
            public: PublicLinks {
                grafana: trim_slash(public_grafana),
                prometheus: trim_slash(public_prometheus),
                alertmanager: trim_slash(public_alertmanager),
                case_management: trim_slash(public_cases),
                siem_overview_dashboard: trim_slash(overview),
            },
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

fn trim_slash(mut s: String) -> String {
    while s.ends_with('/') {
        s.pop();
    }
    s
}
