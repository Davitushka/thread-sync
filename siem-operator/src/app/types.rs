use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Section {
    #[default]
    Overview,
    Detections,
    Alerts,
    Events,
    Investigations,
    Assets,
    Cases,
    StackControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SavedView {
    All,
    MyQueue,
    Critical24h,
    NoAssignee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UserRole {
    Analyst,
    Senior,
    Manager,
}

#[derive(Debug, Clone)]
pub(super) struct AuditEntry {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
}

#[derive(Debug, Clone)]
pub(super) enum PendingAction {
    Close { reason: String },
    MoveStatus { status: String },
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(super) struct CaseFilters {
    pub search: String,
    pub severity: String,
    pub status: String,
    pub assignee: String,
    pub source: String,
    pub mitre: String,
    pub stale_only: bool,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(super) struct EventFilters {
    pub search: String,
    pub severity: String,
    pub state: String,
    pub silenced_only: bool,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(super) struct AssetFilters {
    pub search: String,
    pub risk: String,
    pub source: String,
    pub stale_only: bool,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(super) struct DetectionFilters {
    pub search: String,
    pub severity: String,
    pub state: String,
}
