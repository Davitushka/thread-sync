use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Нормализованное событие — единая ECS-совместимая схема для всех источников.
/// Используется как внутренний тип и как формат сериализации в Kafka/ClickHouse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEvent {
    /// RFC3339 timestamp события
    #[serde(rename = "@timestamp")]
    pub timestamp: DateTime<Utc>,

    /// Уникальный ID события (UUID v4)
    pub event_id: Uuid,

    /// Тип источника: "dotnet", "postgresql", "redis", "nginx", "kubernetes"
    pub source_type: String,

    /// Класс события: "application", "database", "cache", "network", "auth"
    pub event_type: String,

    /// Severity: "critical", "error", "warning", "info", "debug"
    pub severity: Severity,

    /// Основное сообщение (PII уже замаскировано)
    pub message: String,

    /// Хост-источник
    pub host: String,

    /// IP-адрес источника запроса (из X-Forwarded-For или RemoteAddr)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,

    /// ID пользователя (из claims, session, pg_user)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Действие: HTTP метод, SQL команда, Redis команда
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,

    /// HTTP статус или код ответа
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,

    /// URL path (без query string после маскирования)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_path: Option<String>,

    /// HTTP метод
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_method: Option<String>,

    /// Длительность в миллисекундах
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,

    /// GeoIP обогащение
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo: Option<GeoInfo>,

    /// Дополнительные поля source-specific
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Версия агента
    pub agent_version: String,

    /// Timestamp поступления в систему
    pub ingest_ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl Severity {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "debug" | "trace" | "verbose" => Self::Debug,
            "info" | "information" | "notice" => Self::Info,
            "warn" | "warning" => Self::Warning,
            "error" | "err" => Self::Error,
            "critical" | "fatal" | "emerg" | "alert" | "crit" => Self::Critical,
            _ => Self::Info,
        }
    }

    pub fn numeric_level(&self) -> u8 {
        match self {
            Self::Debug => 0,
            Self::Info => 1,
            Self::Warning => 2,
            Self::Error => 3,
            Self::Critical => 4,
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoInfo {
    pub country_iso: String,
    pub country_name: String,
    pub city: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub asn: Option<u32>,
    pub org: Option<String>,
}

impl NormalizedEvent {
    pub fn new(source_type: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            timestamp: now,
            event_id: Uuid::new_v4(),
            source_type: source_type.into(),
            event_type: "generic".to_string(),
            severity: Severity::Info,
            message: String::new(),
            host: String::new(),
            source_ip: None,
            user_id: None,
            action: None,
            status_code: None,
            url_path: None,
            http_method: None,
            duration_ms: None,
            geo: None,
            metadata: HashMap::new(),
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
            ingest_ts: now,
        }
    }
}
