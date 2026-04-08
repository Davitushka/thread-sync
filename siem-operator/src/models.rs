use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CasesResponse {
    pub cases: Vec<CaseBrief>,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaseBrief {
    pub id: String,
    pub display_key: String,
    pub title: String,
    pub severity: String,
    pub status: String,
    #[serde(default)]
    pub assignee: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertState {
    Firing,
    Acknowledged,
}

#[derive(Debug, Clone)]
pub struct AlertItem {
    pub id: String,
    pub title: String,
    pub severity: String,
    pub source: String,
    pub mitre_tactic: String,
    pub fired_at: String,
    pub state: AlertState,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PortalEvent {
    #[serde(default)]
    pub labels: EventLabels,
    #[serde(default)]
    pub status: EventStatus,
    #[serde(default, rename = "startsAt")]
    pub starts_at: String,
    #[serde(default)]
    #[serde(rename = "endsAt")]
    pub ends_at: String,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventLabels {
    #[serde(default)]
    pub alertname: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub instance: String,
    #[serde(default)]
    pub job: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventStatus {
    #[serde(default)]
    pub state: String,
    #[serde(default, rename = "silencedBy")]
    pub silenced_by: Vec<String>,
}
