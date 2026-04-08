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
