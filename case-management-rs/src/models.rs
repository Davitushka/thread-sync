use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Case {
    pub id: Uuid,
    pub case_number: i64,
    #[sqlx(skip)]
    pub display_key: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub status: String,
    pub priority: i16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_notes: Option<String>,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Utc>>,
}

impl Case {
    pub fn apply_display_key(&mut self) {
        self.display_key = format!("INC-{}", self.case_number);
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TimelineEntry {
    pub id: Uuid,
    pub case_id: Uuid,
    pub actor: String,
    pub entry_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LinkedAlert {
    pub fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LinkedEvent {
    pub event_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub linked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CaseDetail {
    #[serde(flatten)]
    pub case: Case,
    pub timeline: Vec<TimelineEntry>,
    pub linked_alerts: Vec<LinkedAlert>,
    pub linked_events: Vec<LinkedEvent>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCaseRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: i16,
    pub assignee: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct PatchCaseRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub severity: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i16>,
    pub assignee: Option<String>,
    pub tags: Option<Vec<String>>,
    pub resolution: Option<String>,
    pub resolution_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TimelineCreateRequest {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct LinkEventRequest {
    pub event_id: Uuid,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LinkAlertRequest {
    pub fingerprint: String,
    pub rule_id: Option<String>,
    pub rule_title: Option<String>,
    pub severity: Option<String>,
    pub description: Option<String>,
}
