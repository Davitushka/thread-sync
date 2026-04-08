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
