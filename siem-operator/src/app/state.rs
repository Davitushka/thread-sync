use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::types::{AssetFilters, CaseFilters, EventFilters};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PersistedState {
    pub api_base: String,
    pub whoami: String,
    pub role: String,
    pub active_view: String,
    pub auto_triage_enabled: bool,
    #[serde(default)]
    pub last_section: String,
    pub filters: CaseFilters,
    #[serde(default)]
    pub event_filters: EventFilters,
    #[serde(default)]
    pub asset_filters: AssetFilters,
}

pub(super) fn load_state(path: &Path) -> Result<PersistedState, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str::<PersistedState>(&content).map_err(|e| e.to_string())
}

pub(super) fn save_state(path: &Path, state: &PersistedState) -> Result<(), String> {
    let body = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(path, body).map_err(|e| e.to_string())
}
