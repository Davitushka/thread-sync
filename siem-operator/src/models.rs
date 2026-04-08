use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Deserialize)]
pub struct CaseDetailResponse {
    #[serde(flatten)]
    pub case: CaseBrief,
    #[serde(default)]
    pub timeline: Vec<CaseTimelineEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaseTimelineEntry {
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub entry_type: String,
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub body: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct PromQueryResponse {
    #[serde(default)]
    pub data: PromData,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromData {
    #[serde(default)]
    pub result: Vec<PromSeries>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromSeries {
    #[serde(default)]
    pub metric: serde_json::Value,
    #[serde(default)]
    pub value: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvestigationResponse {
    #[serde(default)]
    pub grafana: String,
    #[serde(default)]
    pub suggested_clickhouse_queries: Vec<String>,
    #[serde(default)]
    pub process: String,
    #[serde(default)]
    pub case_id: String,
    #[serde(default)]
    pub display_key: String,
    #[serde(default)]
    pub due_at: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DetectionStats {
    #[serde(default)]
    pub rules_count: i64,
    #[serde(default)]
    pub pending_alerts: i64,
    #[serde(default)]
    pub kafka_lag: i64,
    #[serde(default)]
    pub health: String,
}

/// Публичные URL с `GET /api/v1/ui/config` на SIEM Portal (те же, что на веб-главной).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PortalPublicLinks {
    #[serde(default)]
    pub grafana: String,
    #[serde(default)]
    pub prometheus: String,
    #[serde(default)]
    pub alertmanager: String,
    #[serde(default)]
    pub case_management: String,
    #[serde(default)]
    pub siem_overview_dashboard: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PortalUiConfig {
    #[serde(default)]
    pub links: PortalPublicLinks,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchCaseRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateCaseRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub severity: String,
    #[serde(default)]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedCaseResponse {
    pub id: String,
    pub display_key: String,
    pub title: String,
    pub severity: String,
    pub status: String,
    #[serde(default)]
    pub assignee: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineCreateRequest {
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkAlertRequest {
    pub fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromQueryRangeResponse {
    #[serde(default)]
    pub data: PromRangeData,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromRangeData {
    #[serde(default)]
    pub result: Vec<PromRangeSeries>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromRangeSeries {
    #[serde(default)]
    pub metric: serde_json::Value,
    #[serde(default)]
    pub values: Vec<Vec<serde_json::Value>>,
}
