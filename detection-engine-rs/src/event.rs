use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "@timestamp")]
    pub timestamp: DateTime<Utc>,
    pub event_id: String,
    pub source_type: String,
    pub event_type: String,
    pub severity: String,
    pub message: String,
    pub host: String,
    #[serde(default)]
    pub source_ip: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub status_code: Option<i32>,
    #[serde(default)]
    pub url_path: Option<String>,
    #[serde(default)]
    pub http_method: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<f64>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Event {
    pub fn str_val(opt: &Option<String>) -> &str {
        opt.as_deref().unwrap_or("")
    }

    pub fn int_val(opt: &Option<i32>) -> i32 {
        opt.unwrap_or(0)
    }
}
