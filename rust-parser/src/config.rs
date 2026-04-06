use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub kafka: KafkaConfig,
    pub geoip: GeoIpConfig,
    pub processing: ProcessingConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
    #[serde(default = "default_workers")]
    pub workers: usize,
    /// Если задано (env `SIEM__SERVER__API_KEY`), для `POST /parse` и `POST /alerts/ingest`
    /// требуется заголовок `X-API-Key` или `Authorization: Bearer <ключ>`.
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KafkaConfig {
    pub bootstrap_servers: String,
    #[serde(default = "default_topic")]
    pub topic: String,
    #[serde(default = "default_dlq_topic")]
    pub dlq_topic: String,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_linger_ms")]
    pub linger_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GeoIpConfig {
    #[serde(default = "default_city_db")]
    pub city_db_path: String,
    #[serde(default = "default_asn_db")]
    pub asn_db_path: String,
    #[serde(default = "default_cache_size")]
    pub cache_size: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProcessingConfig {
    #[serde(default = "default_max_event_size")]
    pub max_event_size_bytes: usize,
    #[serde(default = "default_channel_capacity")]
    pub channel_capacity: usize,
    #[serde(default = "crate::config::default_true")]
    pub enable_pii_masking: bool,
    #[serde(default = "crate::config::default_true")]
    pub enable_geoip: bool,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    7000
}
fn default_metrics_port() -> u16 {
    9100
}
fn default_workers() -> usize {
    num_cpus::get()
}
fn default_topic() -> String {
    "siem.events".to_string()
}
fn default_dlq_topic() -> String {
    "siem.events.dlq".to_string()
}
fn default_batch_size() -> usize {
    1000
}
fn default_linger_ms() -> u64 {
    5
}
fn default_city_db() -> String {
    "/etc/geoip/GeoLite2-City.mmdb".to_string()
}
fn default_asn_db() -> String {
    "/etc/geoip/GeoLite2-ASN.mmdb".to_string()
}
fn default_cache_size() -> usize {
    10_000
}
fn default_max_event_size() -> usize {
    1024 * 1024
}
fn default_channel_capacity() -> usize {
    100_000
}
pub fn default_true() -> bool {
    true
}

impl AppConfig {
    pub fn from_env() -> Result<Self, config::ConfigError> {
        config::Config::builder()
            .add_source(config::File::with_name("/etc/siem-parser/config").required(false))
            .add_source(config::Environment::with_prefix("SIEM").separator("__"))
            .build()?
            .try_deserialize()
    }
}
