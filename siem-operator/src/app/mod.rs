use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use eframe::egui;

use crate::models::{
    AlertItem, AlertState, CaseBrief, CaseDetailResponse, CaseTimelineEntry, CasesResponse,
    CreateCaseRequest, CreatedCaseResponse, DetectionStats, InvestigationResponse, LinkAlertRequest,
    PatchCaseRequest, PortalEvent, PromQueryRangeResponse, PromQueryResponse, StackStatusResponse,
    TimelineCreateRequest,
};
use crate::ui::widgets::{pill_label, section_nav_button, severity_color, stack_action_card};
mod state;
mod types;
mod metrics;
mod panels;

use metrics::{average_hours, kpi_card, sparkline_card};
use panels::build_case_sparkline_series;
use state::{load_state, save_state, PersistedState};
use types::{
    AssetFilters, AuditEntry, CaseFilters, DashboardPreset, DetectionFilters, EventFilters,
    PendingAction, SavedView, Section, UserRole,
};

#[derive(Debug, Clone)]
struct EventRow {
    id: String,
    title: String,
    severity: String,
    state: String,
    source: String,
    started_at: String,
    silenced: bool,
}

#[derive(Debug, Clone)]
struct AssetRow {
    name: String,
    source: String,
    risk: String,
    open_cases: usize,
    stale_cases: usize,
}

#[derive(Debug, Clone)]
struct DetectionRow {
    rule: String,
    severity: String,
    state: String,
    signal: String,
}

#[derive(Debug, Clone, Default)]
struct StackServiceStatus {
    service: String,
    status: String,
    detail: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PortalAlertsEnvelope {
    #[serde(default)]
    data: Vec<PortalEvent>,
}

pub struct OperatorApp {
    api_base: String,
    section: Section,
    cases: Vec<CaseBrief>,
    total: i64,
    status: String,
    loading: bool,
    rx: Option<Receiver<Result<CasesResponse, String>>>,
    selected: Option<usize>,
    alerts: Vec<AlertItem>,
    filters: CaseFilters,
    event_filters: EventFilters,
    asset_filters: AssetFilters,
    events: Vec<EventRow>,
    assets: Vec<AssetRow>,
    detections: Vec<DetectionRow>,
    active_view: SavedView,
    close_reason: String,
    whoami: String,
    global_search_query: String,
    palette_open: bool,
    palette_query: String,
    obs_loading: bool,
    obs_rx: Option<Receiver<Result<ObsSnapshot, String>>>,
    obs_snapshot: Option<ObsSnapshot>,
    events_loading: bool,
    events_rx: Option<Receiver<Result<Vec<EventRow>, String>>>,
    detections_loading: bool,
    detections_rx: Option<Receiver<Result<Vec<DetectionRow>, String>>>,
    investigation_loading: bool,
    investigation_rx: Option<Receiver<Result<InvestigationResponse, String>>>,
    assets_loading: bool,
    docker_loading: bool,
    docker_rx: Option<Receiver<Result<String, String>>>,
    docker_last_output: String,
    detection_filters: DetectionFilters,
    investigation_entity: String,
    investigation_notes: Vec<String>,
    investigation_details: Option<InvestigationResponse>,
    investigation_note_input: String,
    case_timeline: Vec<CaseTimelineEntry>,
    timeline_loading: bool,
    timeline_rx: Option<Receiver<Result<Vec<CaseTimelineEntry>, String>>>,
    alerts_loading: bool,
    alerts_rx: Option<Receiver<Result<Vec<AlertItem>, String>>>,
    stack_status_loading: bool,
    stack_status_rx: Option<Receiver<Result<Vec<StackServiceStatus>, String>>>,
    stack_status: Vec<StackServiceStatus>,
    detection_engine_url: String,
    detection_stats_loading: bool,
    detection_stats_rx: Option<Receiver<Result<DetectionStats, String>>>,
    detection_stats: Option<DetectionStats>,
    metrics_series_rx: Option<Receiver<Result<(Vec<f64>, Vec<f64>, Vec<f64>), String>>>,
    metrics_loading: bool,
    eps_series: Vec<f64>,
    alerts_series: Vec<f64>,
    mtta_series: Vec<f64>,
    role: UserRole,
    pending_action: Option<PendingAction>,
    audit_log: Vec<AuditEntry>,
    auto_triage_enabled: bool,
    auto_refresh_enabled: bool,
    auto_refresh_interval_sec: u64,
    last_auto_refresh_at: Instant,
    use_light_theme: bool,
    compact_mode: bool,
    wallpaper_preset: String,
    wallpaper_tint: [u8; 3],
    dashboard_preset: DashboardPreset,
    widget_kpi: bool,
    widget_trends: bool,
    widget_sources: bool,
    widget_severity_mix: bool,
    widget_analyst_load: bool,
    widget_audit: bool,
    playbook_steps: Vec<(String, bool)>,
    last_persist_blob: String,
}

impl Default for OperatorApp {
    fn default() -> Self {
        let api_base =
            std::env::var("SIEM_OPERATOR_API").unwrap_or_else(|_| "http://127.0.0.1:8088".to_string());
        let mut app = Self {
            api_base,
            section: Section::default(),
            cases: Vec::new(),
            total: 0,
            status: "Нажми «Обновить» или дождись авто-загрузки.".to_string(),
            loading: false,
            rx: None,
            selected: None,
            alerts: Vec::new(),
            filters: CaseFilters::default(),
            event_filters: EventFilters::default(),
            asset_filters: AssetFilters::default(),
            events: Vec::new(),
            assets: Vec::new(),
            detections: Vec::new(),
            active_view: SavedView::All,
            close_reason: String::new(),
            whoami: "analyst".to_string(),
            global_search_query: String::new(),
            palette_open: false,
            palette_query: String::new(),
            obs_loading: false,
            obs_rx: None,
            obs_snapshot: None,
            events_loading: false,
            events_rx: None,
            detections_loading: false,
            detections_rx: None,
            investigation_loading: false,
            investigation_rx: None,
            assets_loading: false,
            docker_loading: false,
            docker_rx: None,
            docker_last_output: "No docker action executed yet.".to_string(),
            detection_filters: DetectionFilters::default(),
            investigation_entity: String::new(),
            investigation_notes: Vec::new(),
            investigation_details: None,
            investigation_note_input: String::new(),
            case_timeline: Vec::new(),
            timeline_loading: false,
            timeline_rx: None,
            alerts_loading: false,
            alerts_rx: None,
            stack_status_loading: false,
            stack_status_rx: None,
            stack_status: Vec::new(),
            detection_engine_url: std::env::var("SIEM_OPERATOR_DETECTION_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:9111".to_string()),
            detection_stats_loading: false,
            detection_stats_rx: None,
            detection_stats: None,
            metrics_series_rx: None,
            metrics_loading: false,
            eps_series: Vec::new(),
            alerts_series: Vec::new(),
            mtta_series: Vec::new(),
            role: UserRole::Analyst,
            pending_action: None,
            audit_log: Vec::new(),
            auto_triage_enabled: true,
            auto_refresh_enabled: true,
            auto_refresh_interval_sec: 20,
            last_auto_refresh_at: Instant::now(),
            use_light_theme: false,
            compact_mode: false,
            wallpaper_preset: "midnight".to_string(),
            wallpaper_tint: [22, 27, 36],
            dashboard_preset: DashboardPreset::Soc,
            widget_kpi: true,
            widget_trends: true,
            widget_sources: true,
            widget_severity_mix: true,
            widget_analyst_load: true,
            widget_audit: true,
            playbook_steps: vec![
                ("Validate alert context".to_string(), false),
                ("Collect IOC artifacts".to_string(), false),
                ("Contain impacted asset".to_string(), false),
                ("Document evidence".to_string(), false),
            ],
            last_persist_blob: String::new(),
        };
        app.load_persisted_state_if_exists();
        app
    }
}

#[derive(Debug, Clone)]
struct ObsSnapshot {
    fetched_at: String,
    prom_total_targets: usize,
    prom_up_targets: usize,
    prom_version: String,
    am_alerts_active: usize,
    am_alerts_silenced: usize,
}

impl OperatorApp {
    fn http_client(timeout_sec: u64) -> Result<reqwest::blocking::Client, String> {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_sec))
            .build()
            .map_err(|e| e.to_string())
    }

    fn portal_base(&self) -> String {
        if let Ok(v) = std::env::var("SIEM_OPERATOR_PORTAL") {
            let t = v.trim();
            if !t.is_empty() {
                return t.trim_end_matches('/').to_string();
            }
        }
        let base = self.api_base.trim_end_matches('/');
        if base.contains(":8091") {
            return base.to_string();
        }
        if let Some((scheme, rest)) = base.split_once("://") {
            let host = rest.split('/').next().unwrap_or(rest);
            let host_only = host.split(':').next().unwrap_or(host);
            return format!("{scheme}://{host_only}:8091");
        }
        "http://127.0.0.1:8091".to_string()
    }

    fn case_mgmt_base(&self) -> String {
        let base = self.api_base.trim_end_matches('/');
        if base.contains(":8091") {
            return base.replace(":8091", ":8088");
        }
        base.to_string()
    }

    fn role_label(&self) -> &'static str {
        match self.role {
            UserRole::Analyst => "Analyst",
            UserRole::Senior => "Senior",
            UserRole::Manager => "Manager",
        }
    }

    fn can_confirm_critical(&self) -> bool {
        matches!(self.role, UserRole::Senior | UserRole::Manager)
    }

    fn selected_case_is_critical(&self) -> bool {
        self.selected
            .and_then(|i| self.cases.get(i))
            .map(|c| c.severity.to_lowercase() == "critical")
            .unwrap_or(false)
    }

    fn append_audit(&mut self, action: String) {
        self.audit_log.insert(
            0,
            AuditEntry {
                timestamp: Utc::now().to_rfc3339(),
                actor: format!("{} ({})", self.whoami, self.role_label()),
                action,
            },
        );
        self.audit_log.truncate(150);
    }

    fn state_path() -> PathBuf {
        PathBuf::from("operator-state.json")
    }

    fn saved_view_as_str(view: SavedView) -> &'static str {
        match view {
            SavedView::All => "all",
            SavedView::MyQueue => "my_queue",
            SavedView::Critical24h => "critical_24h",
            SavedView::NoAssignee => "no_assignee",
        }
    }

    fn saved_view_from_str(v: &str) -> SavedView {
        match v {
            "my_queue" => SavedView::MyQueue,
            "critical_24h" => SavedView::Critical24h,
            "no_assignee" => SavedView::NoAssignee,
            _ => SavedView::All,
        }
    }

    fn role_as_str(role: UserRole) -> &'static str {
        match role {
            UserRole::Analyst => "analyst",
            UserRole::Senior => "senior",
            UserRole::Manager => "manager",
        }
    }

    fn role_from_str(v: &str) -> UserRole {
        match v {
            "senior" => UserRole::Senior,
            "manager" => UserRole::Manager,
            _ => UserRole::Analyst,
        }
    }

    fn section_as_str(section: Section) -> &'static str {
        match section {
            Section::Overview => "overview",
            Section::Detections => "detections",
            Section::Alerts => "alerts",
            Section::Events => "events",
            Section::Investigations => "investigations",
            Section::Assets => "assets",
            Section::Cases => "cases",
            Section::StackControl => "stack_control",
            Section::Settings => "settings",
        }
    }

    fn section_from_str(v: &str) -> Section {
        match v {
            "detections" => Section::Detections,
            "alerts" => Section::Alerts,
            "events" => Section::Events,
            "investigations" => Section::Investigations,
            "assets" => Section::Assets,
            "cases" => Section::Cases,
            "stack_control" => Section::StackControl,
            "settings" => Section::Settings,
            _ => Section::Overview,
        }
    }

    fn to_persisted_state(&self) -> PersistedState {
        PersistedState {
            api_base: self.api_base.clone(),
            whoami: self.whoami.clone(),
            role: Self::role_as_str(self.role).to_string(),
            active_view: Self::saved_view_as_str(self.active_view).to_string(),
            auto_triage_enabled: self.auto_triage_enabled,
            last_section: Self::section_as_str(self.section).to_string(),
            filters: self.filters.clone(),
            event_filters: self.event_filters.clone(),
            asset_filters: self.asset_filters.clone(),
            detection_filters: self.detection_filters.clone(),
            selected_investigation_entity: self.investigation_entity.clone(),
            auto_refresh_enabled: self.auto_refresh_enabled,
            auto_refresh_interval_sec: self.auto_refresh_interval_sec,
            theme_mode: if self.use_light_theme {
                "light".to_string()
            } else {
                "dark".to_string()
            },
            compact_mode: self.compact_mode,
            wallpaper_preset: self.wallpaper_preset.clone(),
            wallpaper_tint: self.wallpaper_tint,
            detection_engine_url: self.detection_engine_url.clone(),
        }
    }

    fn load_persisted_state_if_exists(&mut self) {
        let path = Self::state_path();
        if !path.exists() {
            return;
        }
        if let Ok(saved) = load_state(&path) {
            self.api_base = saved.api_base;
            self.whoami = saved.whoami;
            self.role = Self::role_from_str(&saved.role);
            self.active_view = Self::saved_view_from_str(&saved.active_view);
            self.auto_triage_enabled = saved.auto_triage_enabled;
            self.section = Self::section_from_str(&saved.last_section);
            self.filters = saved.filters;
            self.event_filters = saved.event_filters;
            self.asset_filters = saved.asset_filters;
            self.detection_filters = saved.detection_filters;
            self.investigation_entity = saved.selected_investigation_entity;
            self.auto_refresh_enabled = saved.auto_refresh_enabled;
            self.auto_refresh_interval_sec = saved.auto_refresh_interval_sec.clamp(10, 120);
            self.use_light_theme = saved.theme_mode.eq_ignore_ascii_case("light");
            self.compact_mode = saved.compact_mode;
            self.wallpaper_preset = saved.wallpaper_preset;
            self.wallpaper_tint = saved.wallpaper_tint;
            self.detection_engine_url = saved.detection_engine_url;
            if let Ok(blob) = serde_json::to_string(&self.to_persisted_state()) {
                self.last_persist_blob = blob;
            }
        }
    }

    fn maybe_persist_state(&mut self) {
        let state = self.to_persisted_state();
        let Ok(blob) = serde_json::to_string(&state) else {
            return;
        };
        if blob == self.last_persist_blob {
            return;
        }
        let path = Self::state_path();
        if save_state(&path, &state).is_ok() {
            self.last_persist_blob = blob;
        }
    }

    fn has_active_fetches(&self) -> bool {
        self.loading
            || self.obs_loading
            || self.events_loading
            || self.alerts_loading
            || self.detections_loading
            || self.detection_stats_loading
            || self.investigation_loading
            || self.timeline_loading
            || self.stack_status_loading
            || self.metrics_loading
            || self.assets_loading
            || self.rx.is_some()
            || self.obs_rx.is_some()
            || self.events_rx.is_some()
            || self.alerts_rx.is_some()
            || self.detections_rx.is_some()
            || self.investigation_rx.is_some()
            || self.timeline_rx.is_some()
            || self.stack_status_rx.is_some()
            || self.detection_stats_rx.is_some()
            || self.metrics_series_rx.is_some()
    }

    fn tick_auto_refresh(&mut self, ctx: &egui::Context) {
        if !self.auto_refresh_enabled {
            return;
        }
        let interval = self.auto_refresh_interval_sec.clamp(10, 120);
        let elapsed = self.last_auto_refresh_at.elapsed();
        if elapsed >= Duration::from_secs(interval) && !self.has_active_fetches() {
            self.fetch_cases();
            self.fetch_alerts();
            self.fetch_events();
            self.fetch_detections();
            self.fetch_detection_stats();
            self.fetch_stack_status();
            self.fetch_overview_metrics_series();
            if !self.investigation_entity.trim().is_empty() {
                let entity = self.investigation_entity.clone();
                self.fetch_investigation_for_entity(&entity);
            }
            self.fetch_assets();
            self.fetch_observability_snapshot();
            self.last_auto_refresh_at = Instant::now();
            self.status = format!("Auto-refresh sync started ({}s)", interval);
        } else {
            let remaining = Duration::from_secs(interval).saturating_sub(elapsed);
            let ms = remaining.as_millis().clamp(200, 1000) as u64;
            ctx.request_repaint_after(Duration::from_millis(ms));
        }
    }

    fn apply_dashboard_preset(&mut self, preset: DashboardPreset) {
        self.dashboard_preset = preset;
        match preset {
            DashboardPreset::Soc => {
                self.widget_kpi = true;
                self.widget_trends = true;
                self.widget_sources = true;
                self.widget_severity_mix = true;
                self.widget_analyst_load = true;
                self.widget_audit = true;
            }
            DashboardPreset::Executive => {
                self.widget_kpi = true;
                self.widget_trends = true;
                self.widget_sources = false;
                self.widget_severity_mix = true;
                self.widget_analyst_load = true;
                self.widget_audit = false;
            }
            DashboardPreset::Hunting => {
                self.widget_kpi = true;
                self.widget_trends = true;
                self.widget_sources = true;
                self.widget_severity_mix = true;
                self.widget_analyst_load = false;
                self.widget_audit = true;
            }
        }
    }

    fn apply_hotkeys(&mut self, ctx: &egui::Context) {
        let mut do_refresh = false;
        let mut do_assign = false;
        let mut do_close = false;
        let mut do_focus_search = false;
        let mut do_command_palette = false;
        ctx.input(|i| {
            do_refresh = i.key_pressed(egui::Key::R);
            do_assign = i.key_pressed(egui::Key::A);
            do_close = i.key_pressed(egui::Key::C);
            do_focus_search = i.key_pressed(egui::Key::Slash);
            do_command_palette = i.modifiers.ctrl && i.key_pressed(egui::Key::K);
        });
        if do_refresh {
            self.fetch_cases();
        }
        if do_assign {
            self.assign_selected_to_me();
        }
        if do_close {
            self.close_selected("Closed via hotkey");
        }
        if do_focus_search {
            self.section = Section::Events;
            ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("event_search")));
        }
        if do_command_palette {
            self.palette_open = true;
            ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("command_palette_input")));
        }
    }

    fn fetch_cases(&mut self) {
        self.loading = true;
        self.status = "Загрузка…".to_string();
        let portal_url = format!("{}/api/v1/proxy/cases?limit=300", self.portal_base());
        let direct_url = format!("{}/api/v1/cases?limit=300", self.case_mgmt_base());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<CasesResponse, String> {
                let client = Self::http_client(20)?;
                for url in [&portal_url, &direct_url] {
                    let resp = client.get(url).send().map_err(|e| e.to_string())?;
                    if resp.status().is_success() {
                        return resp.json::<CasesResponse>().map_err(|e| e.to_string());
                    }
                }
                Err("cases fetch failed via portal and direct API".to_string())
            })();
            let _ = tx.send(result);
        });
        self.rx = Some(rx);
    }

    fn fetch_events(&mut self) {
        self.events_loading = true;
        let base = self.portal_base();
        let url = format!("{base}/api/v1/proxy/alertmanager/v2/alerts");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<Vec<EventRow>, String> {
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(20))
                    .build()
                    .map_err(|e| e.to_string())?;
                let resp = client.get(&url).send().map_err(|e| e.to_string())?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }
                let raw = resp.text().map_err(|e| e.to_string())?;
                let body = serde_json::from_str::<Vec<PortalEvent>>(&raw)
                    .or_else(|_| serde_json::from_str::<PortalAlertsEnvelope>(&raw).map(|x| x.data))
                    .map_err(|e| format!("decode failed: {e}"))?;
                let rows = body
                    .into_iter()
                    .map(|ev| EventRow {
                        id: if ev.fingerprint.is_empty() {
                            "event".to_string()
                        } else {
                            ev.fingerprint
                        },
                        title: if ev.labels.alertname.is_empty() {
                            "alert".to_string()
                        } else {
                            ev.labels.alertname
                        },
                        severity: if ev.labels.severity.is_empty() {
                            "unknown".to_string()
                        } else {
                            ev.labels.severity
                        },
                        state: if ev.status.state.is_empty() {
                            "unknown".to_string()
                        } else {
                            ev.status.state
                        },
                        source: if ev.labels.instance.is_empty() {
                            ev.labels.job
                        } else {
                            ev.labels.instance
                        },
                        started_at: if ev.starts_at.is_empty() { ev.ends_at } else { ev.starts_at },
                        silenced: !ev.status.silenced_by.is_empty(),
                    })
                    .collect();
                Ok(rows)
            })();
            let _ = tx.send(result);
        });
        self.events_rx = Some(rx);
    }

    fn fetch_alerts(&mut self) {
        self.alerts_loading = true;
        let base = self.portal_base();
        let url = format!("{base}/api/v1/proxy/alertmanager/v2/alerts");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<Vec<AlertItem>, String> {
                let client = Self::http_client(20)?;
                let resp = client.get(&url).send().map_err(|e| e.to_string())?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }
                let raw = resp.text().map_err(|e| e.to_string())?;
                let body = serde_json::from_str::<Vec<PortalEvent>>(&raw)
                    .or_else(|_| serde_json::from_str::<PortalAlertsEnvelope>(&raw).map(|x| x.data))
                    .map_err(|e| format!("decode failed: {e}"))?;
                let mapped = body
                    .into_iter()
                    .map(|ev| AlertItem {
                        id: if ev.fingerprint.is_empty() {
                            format!("am-{}", Utc::now().timestamp_millis())
                        } else {
                            ev.fingerprint
                        },
                        title: if ev.labels.alertname.is_empty() {
                            "Alertmanager alert".to_string()
                        } else {
                            ev.labels.alertname
                        },
                        severity: if ev.labels.severity.is_empty() {
                            "unknown".to_string()
                        } else {
                            ev.labels.severity
                        },
                        source: if ev.labels.instance.is_empty() {
                            ev.labels.job
                        } else {
                            ev.labels.instance
                        },
                        mitre_tactic: "N/A".to_string(),
                        fired_at: if ev.starts_at.is_empty() {
                            ev.ends_at
                        } else {
                            ev.starts_at
                        },
                        state: if ev.status.state.eq_ignore_ascii_case("active") {
                            AlertState::Firing
                        } else {
                            AlertState::Acknowledged
                        },
                    })
                    .collect();
                Ok(mapped)
            })();
            let _ = tx.send(result);
        });
        self.alerts_rx = Some(rx);
    }

    fn fetch_detection_stats(&mut self) {
        self.detection_stats_loading = true;
        let url = format!(
            "{}/api/v1/stats",
            self.detection_engine_url.trim_end_matches('/')
        );
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<DetectionStats, String> {
                let client = Self::http_client(12)?;
                let resp = client.get(&url).send().map_err(|e| e.to_string())?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }
                resp.json::<DetectionStats>().map_err(|e| e.to_string())
            })();
            let _ = tx.send(result);
        });
        self.detection_stats_rx = Some(rx);
    }

    fn fetch_stack_status(&mut self) {
        self.stack_status_loading = true;
        let url = format!("{}/api/v1/stack/status", self.portal_base());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<Vec<StackServiceStatus>, String> {
                let client = Self::http_client(12)?;
                let resp = client.get(&url).send().map_err(|e| e.to_string())?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }
                let parsed = resp
                    .json::<StackStatusResponse>()
                    .map_err(|e| format!("decode stack status: {e}"))?;
                let mut rows = Vec::new();
                for (service, value) in parsed.checks {
                    let ok = value["ok"].as_bool().unwrap_or(false);
                    let detail = value["message"].as_str().unwrap_or_default().to_string();
                    rows.push(StackServiceStatus {
                        service,
                        status: if ok { "up".to_string() } else { "down".to_string() },
                        detail,
                    });
                }
                Ok(rows)
            })();
            let _ = tx.send(result);
        });
        self.stack_status_rx = Some(rx);
    }

    fn fetch_case_timeline(&mut self, case_id: &str) {
        self.timeline_loading = true;
        let url = format!(
            "{}/api/v1/cases/{}",
            self.case_mgmt_base(),
            case_id.trim()
        );
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<Vec<CaseTimelineEntry>, String> {
                let client = Self::http_client(12)?;
                let resp = client.get(&url).send().map_err(|e| e.to_string())?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }
                let detail = resp.json::<CaseDetailResponse>().map_err(|e| e.to_string())?;
                Ok(detail.timeline)
            })();
            let _ = tx.send(result);
        });
        self.timeline_rx = Some(rx);
    }

    fn fetch_overview_metrics_series(&mut self) {
        self.metrics_loading = true;
        let end = Utc::now().timestamp();
        let start = end - 24 * 3600;
        let base = self.portal_base();
        let q_eps = format!("{base}/api/v1/proxy/prometheus/query_range?query=sum(rate(ALERTS[5m]))&start={start}&end={end}&step=300");
        let q_alerts = format!("{base}/api/v1/proxy/prometheus/query_range?query=count(ALERTS)&start={start}&end={end}&step=300");
        let q_mtta = format!("{base}/api/v1/proxy/prometheus/query_range?query=avg_over_time(up[30m])&start={start}&end={end}&step=300");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<(Vec<f64>, Vec<f64>, Vec<f64>), String> {
                let client = Self::http_client(15)?;
                let fetch = |url: &str| -> Result<Vec<f64>, String> {
                    let resp = client.get(url).send().map_err(|e| e.to_string())?;
                    if !resp.status().is_success() {
                        return Err(format!("HTTP {}", resp.status()));
                    }
                    let body = resp
                        .json::<PromQueryRangeResponse>()
                        .map_err(|e| e.to_string())?;
                    let mut values = Vec::new();
                    if let Some(series) = body.data.result.first() {
                        for pair in &series.values {
                            let v = pair
                                .get(1)
                                .and_then(|x| x.as_str())
                                .and_then(|s| s.parse::<f64>().ok())
                                .unwrap_or(0.0);
                            values.push(v);
                        }
                    }
                    Ok(values)
                };
                Ok((fetch(&q_eps)?, fetch(&q_alerts)?, fetch(&q_mtta)?))
            })();
            let _ = tx.send(result);
        });
        self.metrics_series_rx = Some(rx);
    }

    fn fetch_detections(&mut self) {
        self.detections_loading = true;
        let base = self.portal_base();
        let url = format!("{base}/api/v1/proxy/prometheus/query?query=ALERTS");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<Vec<DetectionRow>, String> {
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .build()
                    .map_err(|e| e.to_string())?;
                let resp = client.get(&url).send().map_err(|e| e.to_string())?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }
                let body = resp.json::<PromQueryResponse>().map_err(|e| e.to_string())?;
                let rows = body
                    .data
                    .result
                    .into_iter()
                    .map(|s| {
                        let rule = s.metric["alertname"].as_str().unwrap_or("alert").to_string();
                        let severity = s.metric["severity"].as_str().unwrap_or("unknown").to_string();
                        let state = s.metric["alertstate"].as_str().unwrap_or("firing").to_string();
                        let signal = s.value.get(1).and_then(|v| v.as_str()).unwrap_or("0").to_string();
                        DetectionRow {
                            rule,
                            severity,
                            state,
                            signal,
                        }
                    })
                    .collect();
                Ok(rows)
            })();
            let _ = tx.send(result);
        });
        self.detections_rx = Some(rx);
    }

    fn fetch_investigation_for_entity(&mut self, entity: &str) {
        let entity = entity.trim().to_string();
        if entity.is_empty() {
            return;
        }
        self.investigation_loading = true;
        let portal_url = format!("{}/api/v1/proxy/cases/{entity}/investigate", self.portal_base());
        let direct_url = format!("{}/api/v1/cases/{entity}/investigate", self.case_mgmt_base());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<InvestigationResponse, String> {
                let client = Self::http_client(15)?;
                let mut last_err = String::new();
                for url in [&portal_url, &direct_url] {
                    let resp = client.get(url).send().map_err(|e| e.to_string())?;
                    if !resp.status().is_success() {
                        last_err = format!("{} -> HTTP {}", url, resp.status());
                        continue;
                    }
                    let raw = resp.text().map_err(|e| e.to_string())?;
                    if let Ok(parsed) = serde_json::from_str::<InvestigationResponse>(&raw) {
                        return Ok(parsed);
                    }
                }
                Err(format!("investigation decode failed: {last_err}"))
            })();
            let _ = tx.send(result);
        });
        self.investigation_rx = Some(rx);
    }

    fn rebuild_assets_from_cases(&mut self) {
        use std::collections::BTreeMap;
        let mut map: BTreeMap<String, AssetRow> = BTreeMap::new();
        for case in &self.cases {
            let name = case
                .assignee
                .clone()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| Self::inferred_source(case).to_lowercase());
            let risk = if case.severity.eq_ignore_ascii_case("critical") {
                "critical"
            } else if case.severity.eq_ignore_ascii_case("high") {
                "high"
            } else {
                "normal"
            };
            let entry = map.entry(name.clone()).or_insert(AssetRow {
                name,
                source: Self::inferred_source(case).to_string(),
                risk: risk.to_string(),
                open_cases: 0,
                stale_cases: 0,
            });
            if !Self::is_closed_status(&case.status) {
                entry.open_cases += 1;
            }
            if Self::is_stale(case) {
                entry.stale_cases += 1;
            }
            if risk == "critical" {
                entry.risk = "critical".to_string();
            } else if risk == "high" && entry.risk != "critical" {
                entry.risk = "high".to_string();
            }
        }
        self.assets = map.into_values().collect();
    }

    fn fetch_assets(&mut self) {
        self.assets_loading = true;
        self.rebuild_assets_from_cases();
        self.assets_loading = false;
    }

    fn discover_compose_dir() -> Option<PathBuf> {
        let mut candidates = vec![PathBuf::from("../deploy/docker"), PathBuf::from("deploy/docker")];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(bin_dir) = exe.parent() {
                let root_guess = bin_dir
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("deploy")
                    .join("docker");
                candidates.push(root_guess);
            }
        }
        candidates
            .into_iter()
            .map(|p| p.canonicalize().unwrap_or(p))
            .find(|p| p.join("docker-compose.yml").exists())
    }

    fn run_docker_compose_action(&mut self, action: &'static str) {
        if self.docker_loading {
            self.status = "Docker command is already running".to_string();
            return;
        }
        let Some(workdir) = Self::discover_compose_dir() else {
            self.status = "Cannot find deploy/docker with docker-compose.yml".to_string();
            return;
        };
        self.docker_loading = true;
        self.status = format!("Running docker compose {action}...");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let command = match action {
                    "up" => "docker compose up -d",
                    "down" => "docker compose down",
                    "restart" => "docker compose down && docker compose up -d",
                    "ps" => "docker compose ps",
                    _ => return Err(format!("Unsupported docker action: {action}")),
                };
                let mut cmd = if cfg!(target_os = "windows") {
                    let mut c = Command::new("cmd");
                    c.arg("/C").arg(command);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.arg("-lc").arg(command);
                    c
                };
                let output = cmd.current_dir(Path::new(&workdir)).output().map_err(|e| e.to_string())?;
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let merged = if stderr.trim().is_empty() {
                    stdout
                } else if stdout.trim().is_empty() {
                    stderr
                } else {
                    format!("{stdout}\n{stderr}")
                };
                if output.status.success() {
                    Ok(merged)
                } else {
                    Err(merged)
                }
            })();
            let _ = tx.send(result);
        });
        self.docker_rx = Some(rx);
    }

    fn trim_api_base(&mut self) {
        self.api_base = self.api_base.trim().to_string();
    }

    fn patch_case_remote(&self, case_id: &str, req: &PatchCaseRequest) -> Result<(), String> {
        let url = format!("{}/api/v1/cases/{}", self.case_mgmt_base(), case_id);
        let client = Self::http_client(15)?;
        let resp = client
            .patch(&url)
            .header("X-SOC-Actor", &self.whoami)
            .json(req)
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("PATCH failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    fn create_case_remote(&self, req: &CreateCaseRequest) -> Result<CreatedCaseResponse, String> {
        let url = format!("{}/api/v1/cases", self.case_mgmt_base());
        let client = Self::http_client(20)?;
        let resp = client
            .post(&url)
            .header("X-SOC-Actor", &self.whoami)
            .json(req)
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Create case failed: HTTP {}", resp.status()));
        }
        resp.json::<CreatedCaseResponse>().map_err(|e| e.to_string())
    }

    fn add_timeline_remote(&self, case_id: &str, body: &str) -> Result<(), String> {
        let url = format!("{}/api/v1/cases/{}/timeline", self.case_mgmt_base(), case_id);
        let client = Self::http_client(15)?;
        let req = TimelineCreateRequest {
            body: body.to_string(),
        };
        let resp = client
            .post(&url)
            .header("X-SOC-Actor", &self.whoami)
            .json(&req)
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Timeline write failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    fn link_alert_remote(&self, case_id: &str, alert: &AlertItem) -> Result<(), String> {
        let url = format!("{}/api/v1/cases/{}/alerts", self.case_mgmt_base(), case_id);
        let client = Self::http_client(15)?;
        let req = LinkAlertRequest {
            fingerprint: alert.id.clone(),
            rule_id: None,
            rule_title: Some(alert.title.clone()),
            severity: Some(alert.severity.clone()),
            description: Some(format!("source={} fired_at={}", alert.source, alert.fired_at)),
            context: serde_json::json!({
                "source": alert.source,
                "mitre_tactic": alert.mitre_tactic,
            }),
        };
        let resp = client
            .post(&url)
            .header("X-SOC-Actor", &self.whoami)
            .json(&req)
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Alert link failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    fn assign_selected_to_me(&mut self) {
        if let Some(i) = self.selected {
            if let Some(case) = self.cases.get(i) {
                let req = PatchCaseRequest {
                    status: None,
                    severity: None,
                    assignee: Some(self.whoami.clone()),
                    resolution: None,
                };
                match self.patch_case_remote(&case.id, &req) {
                    Ok(_) => {
                        self.status = format!("{} assigned to {}", case.display_key, self.whoami);
                        self.append_audit(format!("Assigned {} to {}", case.display_key, self.whoami));
                        self.fetch_cases();
                    }
                    Err(e) => self.status = format!("Assign failed: {e}"),
                }
            }
        }
    }

    fn close_selected(&mut self, reason: &str) {
        if self.selected_case_is_critical() {
            if !self.can_confirm_critical() {
                self.status = "RBAC: critical actions require Senior/Manager role".to_string();
                self.append_audit("Denied critical close: insufficient role".to_string());
                return;
            }
            self.pending_action = Some(PendingAction::Close {
                reason: reason.to_string(),
            });
            return;
        }
        if let Some(i) = self.selected {
            if let Some(case) = self.cases.get(i) {
                let req = PatchCaseRequest {
                    status: Some("closed".to_string()),
                    severity: None,
                    assignee: None,
                    resolution: Some(reason.to_string()),
                };
                match self.patch_case_remote(&case.id, &req) {
                    Ok(_) => {
                        self.status = format!("{} closed: {}", case.display_key, reason);
                        self.append_audit(format!("Closed {} ({})", case.display_key, reason));
                        self.fetch_cases();
                    }
                    Err(e) => self.status = format!("Close failed: {e}"),
                }
            }
        }
    }

    fn move_selected_to_status(&mut self, status: &str) {
        if self.selected_case_is_critical() && (status == "Closed" || status == "Escalated") {
            if !self.can_confirm_critical() {
                self.status = "RBAC: critical transitions require Senior/Manager role".to_string();
                self.append_audit("Denied critical transition: insufficient role".to_string());
                return;
            }
            self.pending_action = Some(PendingAction::MoveStatus {
                status: status.to_string(),
            });
            return;
        }
        if let Some(i) = self.selected {
            if let Some(case) = self.cases.get(i) {
                let req = PatchCaseRequest {
                    status: Some(Self::normalize_status_for_api(status)),
                    severity: None,
                    assignee: None,
                    resolution: None,
                };
                match self.patch_case_remote(&case.id, &req) {
                    Ok(_) => {
                        self.status = format!("{} -> {}", case.display_key, status);
                        self.append_audit(format!("Transition {} -> {}", case.display_key, status));
                        self.fetch_cases();
                    }
                    Err(e) => self.status = format!("Transition failed: {e}"),
                }
            }
        }
    }

    fn apply_auto_triage_rules(&mut self) {
        let mut changed = 0usize;
        for case in &mut self.cases {
            if case.severity.eq_ignore_ascii_case("critical") && case.assignee.is_none() {
                case.assignee = Some("tier2-oncall".to_string());
                changed += 1;
            }
            if case.severity.eq_ignore_ascii_case("high")
                && case.title.to_lowercase().contains("auth")
                && !case.status.to_lowercase().contains("escalated")
            {
                case.status = "Escalated".to_string();
                changed += 1;
            }
        }
        if changed > 0 {
            self.status = format!("Auto-triage applied {} updates", changed);
            self.append_audit(format!("Auto-triage updated {} fields", changed));
        }
    }

    fn promote_alert_to_case(&mut self, alert: &AlertItem) -> Result<(), String> {
        let req = CreateCaseRequest {
            title: alert.title.clone(),
            description: format!("Promoted from alert {}", alert.id),
            severity: alert.severity.to_lowercase(),
            status: "new".to_string(),
            assignee: None,
            source: "siem-operator".to_string(),
        };
        let created = self.create_case_remote(&req)?;
        self.link_alert_remote(&created.id, alert)?;
        self.add_timeline_remote(
            &created.id,
            &format!("Promoted alert {} to case {}", alert.id, created.display_key),
        )?;
        self.fetch_cases();
        Ok(())
    }

    fn promote_investigation_to_case(&mut self) -> Result<(), String> {
        let title = if self.investigation_entity.trim().is_empty() {
            "Investigation finding".to_string()
        } else {
            format!("Investigation: {}", self.investigation_entity)
        };
        let req = CreateCaseRequest {
            title,
            description: self
                .investigation_details
                .as_ref()
                .map(|d| d.summary.clone())
                .unwrap_or_else(|| "Promoted from investigation".to_string()),
            severity: "high".to_string(),
            status: "new".to_string(),
            assignee: None,
            source: "siem-operator".to_string(),
        };
        let created = self.create_case_remote(&req)?;
        self.add_timeline_remote(
            &created.id,
            &format!("Investigation {} promoted to case", self.investigation_entity),
        )?;
        self.fetch_cases();
        Ok(())
    }

    fn export_selected_case_markdown(&mut self) {
        let Some(i) = self.selected else {
            self.status = "Select case before export".to_string();
            return;
        };
        let Some(case) = self.cases.get(i) else {
            return;
        };
        let mut out = String::new();
        out.push_str(&format!("# Incident Report {}\n\n", case.display_key));
        out.push_str(&format!("- Title: {}\n", case.title));
        out.push_str(&format!("- Severity: {}\n", case.severity));
        out.push_str(&format!("- Status: {}\n", case.status));
        out.push_str(&format!(
            "- Assignee: {}\n",
            case.assignee.as_deref().unwrap_or("Unassigned")
        ));
        out.push_str(&format!("- Created at: {}\n\n", case.created_at));
        out.push_str("## Timeline\n");
        out.push_str(&format!("- {} Case created\n", case.created_at));
        out.push_str(&format!(
            "- {} Snapshot: status={}, severity={}\n",
            Utc::now().to_rfc3339(),
            case.status,
            case.severity
        ));
        out.push_str("\n## Actions\n");
        for (idx, (step, done)) in self.playbook_steps.iter().enumerate() {
            let marker = if *done { "x" } else { " " };
            out.push_str(&format!("- [{}] {}. {}\n", marker, idx + 1, step));
        }
        let mut path = PathBuf::from("reports");
        let _ = fs::create_dir_all(&path);
        path.push(format!("{}.md", case.display_key));
        match fs::write(&path, out) {
            Ok(_) => {
                self.status = format!("Report exported: {}", path.display());
                self.append_audit(format!("Exported report {}", path.display()));
            }
            Err(e) => {
                self.status = format!("Export failed: {e}");
            }
        }
    }

    fn case_age_hours(case: &CaseBrief) -> Option<i64> {
        let dt = DateTime::parse_from_rfc3339(&case.created_at).ok()?;
        let age = Utc::now().signed_duration_since(dt.with_timezone(&Utc));
        Some(age.num_hours().max(0))
    }

    fn is_closed_status(status: &str) -> bool {
        let s = status.to_lowercase();
        s.contains("closed") || s.contains("resolved") || s.contains("done")
    }

    fn normalize_status_for_api(status: &str) -> String {
        match status.trim().to_lowercase().as_str() {
            "new" => "new".to_string(),
            "in progress" | "in_progress" => "in_progress".to_string(),
            "escalated" => "escalated".to_string(),
            "closed" => "closed".to_string(),
            other => other.replace(' ', "_"),
        }
    }

    fn is_stale(case: &CaseBrief) -> bool {
        if Self::is_closed_status(&case.status) {
            return false;
        }
        Self::case_age_hours(case).map(|h| h >= 24).unwrap_or(false)
    }

    fn inferred_source(case: &CaseBrief) -> &'static str {
        let title = case.title.to_lowercase();
        if title.contains("auth") || title.contains("login") {
            "Identity"
        } else if title.contains("network") || title.contains("scan") {
            "Network"
        } else if title.contains("endpoint") || title.contains("edr") {
            "Endpoint"
        } else {
            "SIEM"
        }
    }

    fn inferred_mitre(case: &CaseBrief) -> &'static str {
        let title = case.title.to_lowercase();
        if title.contains("phish") {
            "TA0001 Initial Access"
        } else if title.contains("credential") || title.contains("auth") {
            "TA0006 Credential Access"
        } else if title.contains("lateral") {
            "TA0008 Lateral Movement"
        } else {
            "TA0005 Defense Evasion"
        }
    }

    fn filtered_case_indices(&self) -> Vec<usize> {
        self.cases
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                if !self.filters.search.trim().is_empty() {
                    let needle = self.filters.search.to_lowercase();
                    let hay = format!(
                        "{} {} {} {}",
                        c.display_key,
                        c.title,
                        c.assignee.clone().unwrap_or_default(),
                        c.status
                    )
                    .to_lowercase();
                    if !hay.contains(&needle) {
                        return false;
                    }
                }
                if self.filters.severity != "All" && c.severity.to_lowercase() != self.filters.severity.to_lowercase() {
                    return false;
                }
                if self.filters.status != "All"
                    && !c.status.to_lowercase().contains(&self.filters.status.to_lowercase())
                {
                    return false;
                }
                if self.filters.assignee == "Unassigned" && c.assignee.is_some() {
                    return false;
                }
                if self.filters.assignee == "Assigned" && c.assignee.is_none() {
                    return false;
                }
                if self.filters.source != "All" && Self::inferred_source(c) != self.filters.source {
                    return false;
                }
                if self.filters.mitre != "All" && Self::inferred_mitre(c) != self.filters.mitre {
                    return false;
                }
                if self.filters.stale_only && !Self::is_stale(c) {
                    return false;
                }
                true
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn apply_saved_view(&mut self, view: SavedView) {
        self.active_view = view;
        self.filters = CaseFilters::default();
        match view {
            SavedView::All => {}
            SavedView::MyQueue => {
                self.filters.assignee = "Assigned".to_string();
                self.filters.search = self.whoami.clone();
            }
            SavedView::Critical24h => {
                self.filters.severity = "critical".to_string();
                self.filters.stale_only = true;
            }
            SavedView::NoAssignee => {
                self.filters.assignee = "Unassigned".to_string();
            }
        }
    }

    fn show_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .resizable(true)
            .default_width(230.0)
            .min_width(200.0)
            .max_width(300.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(14, 18, 24))
                    .inner_margin(egui::Margin::same(16.0))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(32, 40, 54))),
            )
            .show(ctx, |ui| {
                let footer_reserved = 102.0;
                let nav_height = (ui.available_height() - footer_reserved).max(120.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), nav_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("SIEM-Lite")
                                            .strong()
                                            .size(20.0)
                                            .color(egui::Color32::WHITE),
                                    );
                                    ui.label(
                                        egui::RichText::new("Operator")
                                            .size(13.0)
                                            .color(egui::Color32::from_rgb(120, 190, 255)),
                                    );
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui
                                        .add_sized([32.0, 32.0], egui::Button::new(egui::RichText::new("⚙").size(16.0)))
                                        .on_hover_text("Settings")
                                        .clicked()
                                    {
                                        self.section = Section::Settings;
                                    }
                                });
                            });
                            ui.add_space(20.0);
                            ui.label(
                                egui::RichText::new("Разделы")
                                    .small()
                                    .color(egui::Color32::from_rgb(120, 128, 145)),
                            );
                            ui.add_space(8.0);
                            if section_nav_button(ui, "Overview", "KPI и SLA", self.section == Section::Overview) {
                                self.section = Section::Overview;
                            }
                            if section_nav_button(ui, "Detections", "Rules и сигналы", self.section == Section::Detections) {
                                self.section = Section::Detections;
                            }
                            if section_nav_button(ui, "Alerts", "Inbox и Promote", self.section == Section::Alerts) {
                                self.section = Section::Alerts;
                            }
                            if section_nav_button(ui, "Events", "Поток и триаж", self.section == Section::Events) {
                                self.section = Section::Events;
                            }
                            if section_nav_button(
                                ui,
                                "Investigations",
                                "Timeline и workbench",
                                self.section == Section::Investigations,
                            ) {
                                self.section = Section::Investigations;
                            }
                            if section_nav_button(ui, "Assets", "Хосты и риск", self.section == Section::Assets) {
                                self.section = Section::Assets;
                            }
                            if section_nav_button(ui, "Cases", "Lifecycle response", self.section == Section::Cases) {
                                self.section = Section::Cases;
                            }
                            if section_nav_button(
                                ui,
                                "Stack Control",
                                "Docker и health",
                                self.section == Section::StackControl,
                            ) {
                                self.section = Section::StackControl;
                            }
                        });
                    },
                );
                ui.separator();
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Hotkeys: R / A / C / /")
                        .small()
                        .color(egui::Color32::from_rgb(90, 98, 115)),
                );
                ui.label(
                    egui::RichText::new("v0.2")
                        .small()
                        .color(egui::Color32::from_rgb(90, 98, 115)),
                );
                if ui
                    .add_sized([ui.available_width(), 36.0], egui::Button::new("Выход из приложения"))
                    .clicked()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
    }

    fn show_status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(28.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(12, 16, 22))
                    .inner_margin(egui::Margin::symmetric(14.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        egui::RichText::new(&self.status)
                            .small()
                            .monospace()
                            .color(egui::Color32::from_rgb(175, 185, 200)),
                    );
                });
            });
    }

    fn current_section_label(&self) -> &'static str {
        match self.section {
            Section::Overview => "Overview",
            Section::Detections => "Detections",
            Section::Alerts => "Alerts",
            Section::Events => "Events",
            Section::Investigations => "Investigations",
            Section::Assets => "Assets",
            Section::Cases => "Cases",
            Section::StackControl => "Stack Control",
            Section::Settings => "Settings",
        }
    }

    fn is_compact(&self, ui: &egui::Ui) -> bool {
        self.compact_mode || ui.available_width() < 1100.0
    }

    fn show_top_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("web_toolbar")
            .exact_height(76.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(16, 20, 28))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(36, 45, 62)))
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("SIEM-Lite")
                                .strong()
                                .size(18.0)
                                .color(egui::Color32::WHITE),
                        );
                        ui.label(
                            egui::RichText::new(format!(" / {}", self.current_section_label()))
                                .small()
                                .color(egui::Color32::from_rgb(145, 165, 192)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("{} ({})", self.whoami, self.role_label()))
                                    .small()
                                    .color(egui::Color32::from_rgb(190, 205, 230)),
                            );
                        });
                    });
                    ui.add_space(4.0);
                    if self.is_compact(ui) {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Global").small());
                            ui.add(
                                egui::TextEdit::singleline(&mut self.global_search_query)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("Search case, alert, host, IOC..."),
                            );
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Search").clicked() {
                                    let q = self.global_search_query.trim();
                                    if !q.is_empty() {
                                        self.filters.search = q.to_string();
                                        self.event_filters.search = q.to_string();
                                        self.asset_filters.search = q.to_string();
                                        self.detection_filters.search = q.to_string();
                                        self.section = Section::Cases;
                                        self.status = format!("Global search applied: {}", q);
                                    }
                                }
                                if ui.button("CmdK").clicked() {
                                    self.palette_open = true;
                                }
                            });
                        });
                    } else {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Global").small());
                            ui.add(
                                egui::TextEdit::singleline(&mut self.global_search_query)
                                    .desired_width(320.0)
                                    .hint_text("Search case, alert, host, IOC..."),
                            );
                            if ui.button("Search").clicked() {
                                let q = self.global_search_query.trim();
                                if !q.is_empty() {
                                    self.filters.search = q.to_string();
                                    self.event_filters.search = q.to_string();
                                    self.asset_filters.search = q.to_string();
                                    self.detection_filters.search = q.to_string();
                                    self.section = Section::Cases;
                                    self.status = format!("Global search applied: {}", q);
                                }
                            }
                            if ui.button("CmdK").clicked() {
                                self.palette_open = true;
                            }
                        });
                    }
                });
            });
    }

    fn background_fill_color(&self) -> egui::Color32 {
        if self.wallpaper_preset.eq_ignore_ascii_case("custom") {
            return egui::Color32::from_rgb(
                self.wallpaper_tint[0],
                self.wallpaper_tint[1],
                self.wallpaper_tint[2],
            );
        }
        match self.wallpaper_preset.as_str() {
            "graphite" => egui::Color32::from_rgb(28, 30, 34),
            "ocean" => egui::Color32::from_rgb(17, 33, 52),
            "sunset" => egui::Color32::from_rgb(44, 28, 36),
            _ => egui::Color32::from_rgb(22, 27, 36),
        }
    }

    fn show_cases_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(12.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Cases")
                            .strong()
                            .size(24.0)
                            .color(egui::Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new("Incident lifecycle, ownership and SLA actions")
                            .small()
                            .color(egui::Color32::from_rgb(150, 165, 188)),
                    );
                    if ui.button("Refresh").clicked() {
                        self.fetch_cases();
                    }
                });
            });
        ui.add_space(6.0);
        let filtered = self.filtered_case_indices();
        ui.label(egui::RichText::new(format!("Показано: {} · В ответе: {} · Всего: {}", filtered.len(), self.cases.len(), self.total)).size(13.0).color(egui::Color32::from_rgb(150, 160, 178)));
        ui.add_space(14.0);

        ui.horizontal_wrapped(|ui| {
            ui.label("Views:");
            for (view, title) in [
                (SavedView::All, "All"),
                (SavedView::MyQueue, "My Queue"),
                (SavedView::Critical24h, "Critical 24h"),
                (SavedView::NoAssignee, "No assignee"),
            ] {
                let selected = self.active_view == view;
                if ui.selectable_label(selected, title).clicked() {
                    self.apply_saved_view(view);
                }
            }
        });
        ui.add_space(8.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                let compact = self.is_compact(ui);
                if compact {
                    ui.vertical(|ui| {
                        ui.label("Search:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.filters.search)
                                .id_source("case_search")
                                .desired_width(f32::INFINITY),
                        );
                        egui::ComboBox::from_label("Severity")
                            .selected_text(if self.filters.severity.is_empty() { "All" } else { &self.filters.severity })
                            .show_ui(ui, |ui| {
                                for v in ["All", "critical", "high", "medium", "low", "info"] {
                                    if ui.selectable_label(self.filters.severity == v || (self.filters.severity.is_empty() && v == "All"), v).clicked() {
                                        self.filters.severity = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("Status")
                            .selected_text(if self.filters.status.is_empty() { "All" } else { &self.filters.status })
                            .show_ui(ui, |ui| {
                                for v in ["All", "new", "in progress", "escalated", "closed"] {
                                    if ui.selectable_label(self.filters.status == v || (self.filters.status.is_empty() && v == "All"), v).clicked() {
                                        self.filters.status = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("Assignee")
                            .selected_text(if self.filters.assignee.is_empty() { "All" } else { &self.filters.assignee })
                            .show_ui(ui, |ui| {
                                for v in ["All", "Assigned", "Unassigned"] {
                                    if ui.selectable_label(self.filters.assignee == v || (self.filters.assignee.is_empty() && v == "All"), v).clicked() {
                                        self.filters.assignee = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("Source")
                            .selected_text(if self.filters.source.is_empty() { "All" } else { &self.filters.source })
                            .show_ui(ui, |ui| {
                                for v in ["All", "SIEM", "Identity", "Network", "Endpoint"] {
                                    if ui.selectable_label(self.filters.source == v || (self.filters.source.is_empty() && v == "All"), v).clicked() {
                                        self.filters.source = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("MITRE")
                            .selected_text(if self.filters.mitre.is_empty() { "All" } else { &self.filters.mitre })
                            .show_ui(ui, |ui| {
                                for v in ["All", "TA0001 Initial Access", "TA0006 Credential Access", "TA0008 Lateral Movement", "TA0005 Defense Evasion"] {
                                    if ui.selectable_label(self.filters.mitre == v || (self.filters.mitre.is_empty() && v == "All"), v).clicked() {
                                        self.filters.mitre = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        ui.checkbox(&mut self.filters.stale_only, "SLA stale only");
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Search:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.filters.search)
                                .id_source("case_search")
                                .desired_width(220.0),
                        );
                        egui::ComboBox::from_label("Severity")
                            .selected_text(if self.filters.severity.is_empty() { "All" } else { &self.filters.severity })
                            .show_ui(ui, |ui| {
                                for v in ["All", "critical", "high", "medium", "low", "info"] {
                                    if ui.selectable_label(self.filters.severity == v || (self.filters.severity.is_empty() && v == "All"), v).clicked() {
                                        self.filters.severity = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("Status")
                            .selected_text(if self.filters.status.is_empty() { "All" } else { &self.filters.status })
                            .show_ui(ui, |ui| {
                                for v in ["All", "new", "in progress", "escalated", "closed"] {
                                    if ui.selectable_label(self.filters.status == v || (self.filters.status.is_empty() && v == "All"), v).clicked() {
                                        self.filters.status = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                    });
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        egui::ComboBox::from_label("Assignee")
                            .selected_text(if self.filters.assignee.is_empty() { "All" } else { &self.filters.assignee })
                            .show_ui(ui, |ui| {
                                for v in ["All", "Assigned", "Unassigned"] {
                                    if ui.selectable_label(self.filters.assignee == v || (self.filters.assignee.is_empty() && v == "All"), v).clicked() {
                                        self.filters.assignee = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("Source")
                            .selected_text(if self.filters.source.is_empty() { "All" } else { &self.filters.source })
                            .show_ui(ui, |ui| {
                                for v in ["All", "SIEM", "Identity", "Network", "Endpoint"] {
                                    if ui.selectable_label(self.filters.source == v || (self.filters.source.is_empty() && v == "All"), v).clicked() {
                                        self.filters.source = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("MITRE")
                            .selected_text(if self.filters.mitre.is_empty() { "All" } else { &self.filters.mitre })
                            .show_ui(ui, |ui| {
                                for v in ["All", "TA0001 Initial Access", "TA0006 Credential Access", "TA0008 Lateral Movement", "TA0005 Defense Evasion"] {
                                    if ui.selectable_label(self.filters.mitre == v || (self.filters.mitre.is_empty() && v == "All"), v).clicked() {
                                        self.filters.mitre = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        ui.checkbox(&mut self.filters.stale_only, "SLA stale only");
                    });
                }
            });
        ui.add_space(10.0);

        ui.horizontal_wrapped(|ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("Веб-главная").color(egui::Color32::WHITE))
                        .min_size(egui::vec2(0.0, 36.0)),
                )
                .clicked()
            {
                let _ = webbrowser::open(&format!("{}/", self.api_base.trim_end_matches('/')));
            }
            let has_sel = self.selected.is_some();
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(egui::RichText::new("Карточка кейса").color(egui::Color32::WHITE))
                        .min_size(egui::vec2(0.0, 36.0)),
                )
                .clicked()
            {
                if let Some(i) = self.selected {
                    if let Some(c) = self.cases.get(i) {
                        let _ =
                            webbrowser::open(&format!("{}/cases/{}", self.api_base.trim_end_matches('/'), c.id));
                    }
                }
            }
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(
                        egui::RichText::new("Рабочий стол расследования").color(egui::Color32::WHITE),
                    )
                    .min_size(egui::vec2(0.0, 36.0)),
                )
                .clicked()
            {
                if let Some(i) = self.selected {
                    if let Some(c) = self.cases.get(i) {
                        let _ = webbrowser::open(&format!(
                            "{}/cases/{}/investigate",
                            self.api_base.trim_end_matches('/'),
                            c.id
                        ));
                    }
                }
            }
            if self.loading {
                ui.add_space(8.0);
                ui.spinner();
            }
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Quick actions:");
            if ui.button("Assign to me (A)").clicked() {
                self.assign_selected_to_me();
            }
            egui::ComboBox::from_label("Change severity")
                .selected_text("Select")
                .show_ui(ui, |ui| {
                    for sev in ["critical", "high", "medium", "low", "info"] {
                        if ui.button(sev).clicked() {
                            if let Some(i) = self.selected {
                                if let Some(case) = self.cases.get_mut(i) {
                                    case.severity = sev.to_string();
                                    self.status = format!("{} severity -> {}", case.display_key, sev);
                                }
                            }
                        }
                    }
                });
            ui.add(egui::TextEdit::singleline(&mut self.close_reason).hint_text("Close reason"));
            if ui.button("Close (C)").clicked() {
                let reason = if self.close_reason.trim().is_empty() {
                    "manual close".to_string()
                } else {
                    self.close_reason.clone()
                };
                self.close_selected(&reason);
            }
        });

        ui.add_space(16.0);
        let h = ui.available_height().max(120.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .max_height(h)
            .show(ui, |ui| {
                egui::Grid::new("cases_grid")
                    .num_columns(6)
                    .spacing([14.0, 6.0])
                    .striped(true)
                    .min_col_width(60.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Ключ")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 210, 225)),
                        );
                        ui.label(
                            egui::RichText::new("Заголовок")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 210, 225)),
                        );
                        ui.label(
                            egui::RichText::new("Sev")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 210, 225)),
                        );
                        ui.label(
                            egui::RichText::new("Статус")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 210, 225)),
                        );
                        ui.label(
                            egui::RichText::new("Исполнитель")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 210, 225)),
                        );
                        ui.label(
                            egui::RichText::new("Создан")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 210, 225)),
                        );
                        ui.label(
                            egui::RichText::new("SLA")
                                .strong()
                                .color(egui::Color32::from_rgb(200, 210, 225)),
                        );
                        ui.end_row();
                        for i in &filtered {
                            let i = *i;
                            let c = &self.cases[i];
                            let sel = self.selected == Some(i);
                            if ui.selectable_label(sel, &c.display_key).clicked() {
                                self.selected = Some(i);
                            }
                            if ui.selectable_label(sel, &c.title).clicked() {
                                self.selected = Some(i);
                            }
                            ui.horizontal(|ui| {
                                pill_label(ui, &c.severity, severity_color(&c.severity));
                            });
                            if ui.selectable_label(sel, &c.status).clicked() {
                                self.selected = Some(i);
                            }
                            if ui
                                .selectable_label(sel, c.assignee.as_deref().unwrap_or("—"))
                                .clicked()
                            {
                                self.selected = Some(i);
                            }
                            if ui.selectable_label(sel, &c.created_at).clicked() {
                                self.selected = Some(i);
                            }
                            let stale = Self::is_stale(c);
                            let sla_text = if stale { "BREACH" } else { "OK" };
                            let color = if stale {
                                egui::Color32::from_rgb(235, 75, 85)
                            } else {
                                egui::Color32::from_rgb(90, 200, 140)
                            };
                            ui.horizontal(|ui| {
                                pill_label(ui, sla_text, color);
                            });
                            ui.end_row();
                        }
                    });
            });
        ui.add_space(14.0);
        self.show_kanban_panel(ui);
        ui.add_space(14.0);
        self.show_case_timeline_panel(ui);
    }

    fn show_kanban_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Kanban (quick move)").strong().size(18.0));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Move -> New").clicked() {
                self.move_selected_to_status("New");
            }
            if ui.button("Move -> In Progress").clicked() {
                self.move_selected_to_status("In Progress");
            }
            if ui.button("Move -> Escalated").clicked() {
                self.move_selected_to_status("Escalated");
            }
            if ui.button("Move -> Closed").clicked() {
                self.move_selected_to_status("Closed");
            }
        });
        ui.add_space(8.0);

        let columns = ["New", "In Progress", "Escalated", "Closed"];
        ui.columns(columns.len(), |cols| {
            for (idx, col) in columns.iter().enumerate() {
                cols[idx].label(egui::RichText::new(*col).strong());
                let items: Vec<(usize, String, String)> = self
                    .cases
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.status.to_lowercase().contains(&col.to_lowercase()))
                    .take(6)
                    .map(|(i, c)| (i, c.display_key.clone(), c.title.clone()))
                    .collect();
                for (case_idx, key, title) in items {
                    let selected = self.selected == Some(case_idx);
                    let text = format!("{key}: {title}");
                    if cols[idx].selectable_label(selected, text).clicked() {
                        self.selected = Some(case_idx);
                    }
                }
            }
        });
    }

    fn show_case_timeline_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Incident timeline").strong().size(18.0));
        ui.add_space(6.0);
        let Some(idx) = self.selected else {
            ui.label("Выбери кейс в таблице или канбане.");
            return;
        };
        let Some(case) = self.cases.get(idx) else {
            return;
        };
        let case_id = case.id.clone();
        let case_key = case.display_key.clone();
        if ui.button("Refresh timeline from API").clicked() {
            self.fetch_case_timeline(&case_id);
        }
        if self.timeline_loading {
            ui.spinner();
        }
        ui.add_space(6.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(30, 36, 48))
            .inner_margin(egui::Margin::same(10.0))
            .rounding(egui::Rounding::same(8.0))
            .show(ui, |ui| {
                if self.case_timeline.is_empty() {
                    ui.label("No API timeline loaded yet.");
                } else {
                    for row in &self.case_timeline {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(&row.created_at).monospace().small());
                            ui.label(egui::RichText::new(&row.entry_type).strong());
                            ui.label(row.body.clone().unwrap_or_default());
                            if !row.actor.is_empty() {
                                ui.label(format!("by {}", row.actor));
                            }
                        });
                        ui.separator();
                    }
                }
            });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Add timeline note:");
            ui.add(
                egui::TextEdit::singleline(&mut self.investigation_note_input)
                    .desired_width(320.0)
                    .hint_text("comment to case timeline"),
            );
            if ui.button("Post note").clicked() {
                let note = self.investigation_note_input.trim().to_string();
                if !note.is_empty() {
                    match self.add_timeline_remote(&case_id, &note) {
                        Ok(_) => {
                            self.investigation_note_input.clear();
                            self.fetch_case_timeline(&case_id);
                            self.status = "Timeline entry added".to_string();
                        }
                        Err(e) => self.status = format!("Timeline post failed: {e}"),
                    }
                }
            }
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Playbook runner").strong());
            if ui.button("Run all").clicked() {
                for step in &mut self.playbook_steps {
                    step.1 = true;
                }
                self.append_audit(format!("Run playbook for {}", case_key));
            }
            if ui.button("Reset").clicked() {
                for step in &mut self.playbook_steps {
                    step.1 = false;
                }
            }
            if ui.button("Export Markdown report").clicked() {
                self.export_selected_case_markdown();
            }
        });
        for step in &mut self.playbook_steps {
            ui.checkbox(&mut step.1, &step.0);
        }
    }

    fn show_home_panel(&mut self, ui: &mut egui::Ui) {
        let open_count = self.cases.iter().filter(|c| !Self::is_closed_status(&c.status)).count();
        let critical_count = self
            .cases
            .iter()
            .filter(|c| c.severity.to_lowercase() == "critical" && !Self::is_closed_status(&c.status))
            .count();
        let stale_count = self.cases.iter().filter(|c| Self::is_stale(c)).count();
        let mttr_proxy = average_hours(
            self.cases
                .iter()
                .filter(|c| Self::is_closed_status(&c.status))
                .filter_map(Self::case_age_hours),
        );
        let mtta_proxy = average_hours(
            self.cases
                .iter()
                .filter(|c| !Self::is_closed_status(&c.status) && c.assignee.is_none())
                .filter_map(Self::case_age_hours),
        );

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(12.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("SOC Overview")
                            .strong()
                            .size(24.0)
                            .color(egui::Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new("Live posture, triage pressure, SLA and stack control")
                            .small()
                            .color(egui::Color32::from_rgb(150, 165, 188)),
                    );
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Refresh All").clicked() {
                        self.fetch_cases();
                        self.fetch_alerts();
                        self.fetch_events();
                        self.fetch_detection_stats();
                        self.fetch_stack_status();
                        self.fetch_overview_metrics_series();
                        self.fetch_observability_snapshot();
                        self.fetch_assets();
                    }
                    if ui.button("Refresh Cases").clicked() {
                        self.fetch_cases();
                    }
                    if ui.button("Refresh Events").clicked() {
                        self.fetch_events();
                    }
                    if ui
                        .add_enabled(self.selected.is_some(), egui::Button::new("Export selected report"))
                        .clicked()
                    {
                        self.export_selected_case_markdown();
                    }
                });
                if let Some(stats) = &self.detection_stats {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        kpi_card(
                            ui,
                            "Rules",
                            &stats.rules_count.to_string(),
                            egui::Color32::from_rgb(110, 165, 235),
                        );
                        kpi_card(
                            ui,
                            "Pending alerts",
                            &stats.pending_alerts.to_string(),
                            egui::Color32::from_rgb(245, 140, 70),
                        );
                        kpi_card(
                            ui,
                            "Kafka lag",
                            &stats.kafka_lag.to_string(),
                            egui::Color32::from_rgb(235, 195, 80),
                        );
                    });
                }
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Role:");
                    egui::ComboBox::from_id_salt("role_selector")
                        .selected_text(self.role_label())
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(matches!(self.role, UserRole::Analyst), "Analyst").clicked() {
                                self.role = UserRole::Analyst;
                            }
                            if ui.selectable_label(matches!(self.role, UserRole::Senior), "Senior").clicked() {
                                self.role = UserRole::Senior;
                            }
                            if ui.selectable_label(matches!(self.role, UserRole::Manager), "Manager").clicked() {
                                self.role = UserRole::Manager;
                            }
                        });
                    ui.checkbox(&mut self.auto_triage_enabled, "Auto-triage");
                    ui.checkbox(&mut self.auto_refresh_enabled, "Auto-refresh");
                    ui.add(
                        egui::Slider::new(&mut self.auto_refresh_interval_sec, 10..=120)
                            .text("interval")
                            .suffix("s"),
                    );
                    if ui.button("Run triage now").clicked() {
                        self.apply_auto_triage_rules();
                    }
                });
            });
        ui.add_space(10.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(26, 32, 45))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 120, 210)))
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Docker Stack Control").strong().size(18.0));
                    if self.docker_loading {
                        ui.spinner();
                        ui.label("running...");
                    }
                });
                ui.label("Запуск и остановка всего SIEM-стека прямо из Operator.");
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!self.docker_loading, egui::Button::new("Start Stack"))
                        .clicked()
                    {
                        self.run_docker_compose_action("up");
                    }
                    if ui
                        .add_enabled(!self.docker_loading, egui::Button::new("Stop Stack"))
                        .clicked()
                    {
                        self.run_docker_compose_action("down");
                    }
                    if ui
                        .add_enabled(!self.docker_loading, egui::Button::new("Restart Stack"))
                        .clicked()
                    {
                        self.run_docker_compose_action("restart");
                    }
                    if ui
                        .add_enabled(!self.docker_loading, egui::Button::new("Stack Status"))
                        .clicked()
                    {
                        self.run_docker_compose_action("ps");
                    }
                });
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&self.docker_last_output)
                            .monospace()
                            .small()
                            .color(egui::Color32::from_rgb(180, 192, 210)),
                    );
                });
            });
        ui.add_space(12.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Custom Dashboard").strong());
                    if ui
                        .selectable_label(matches!(self.dashboard_preset, DashboardPreset::Soc), "SOC preset")
                        .clicked()
                    {
                        self.apply_dashboard_preset(DashboardPreset::Soc);
                    }
                    if ui
                        .selectable_label(
                            matches!(self.dashboard_preset, DashboardPreset::Executive),
                            "Executive preset",
                        )
                        .clicked()
                    {
                        self.apply_dashboard_preset(DashboardPreset::Executive);
                    }
                    if ui
                        .selectable_label(matches!(self.dashboard_preset, DashboardPreset::Hunting), "Hunting preset")
                        .clicked()
                    {
                        self.apply_dashboard_preset(DashboardPreset::Hunting);
                    }
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.widget_kpi, "KPI");
                    ui.checkbox(&mut self.widget_trends, "Trends");
                    ui.checkbox(&mut self.widget_sources, "Top Sources");
                    ui.checkbox(&mut self.widget_severity_mix, "Severity Mix");
                    ui.checkbox(&mut self.widget_analyst_load, "Analyst Load");
                    ui.checkbox(&mut self.widget_audit, "Audit");
                });
            });
        ui.add_space(10.0);

        let (mut open_series, mut crit_series) = build_case_sparkline_series(&self.cases);
        if !self.alerts_series.is_empty() {
            open_series = self.alerts_series.iter().map(|v| *v as f32).collect();
        }
        if !self.eps_series.is_empty() {
            crit_series = self.eps_series.iter().map(|v| *v as f32).collect();
        }
        let mut by_source: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        let mut by_severity: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        let mut by_analyst: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for c in &self.cases {
            *by_source.entry(Self::inferred_source(c).to_string()).or_insert(0) += 1;
            *by_severity.entry(c.severity.to_lowercase()).or_insert(0) += 1;
            *by_analyst
                .entry(c.assignee.clone().unwrap_or_else(|| "unassigned".to_string()))
                .or_insert(0) += 1;
        }
        let max_source = by_source.values().copied().max().unwrap_or(1) as f32;
        let max_sev = by_severity.values().copied().max().unwrap_or(1) as f32;
        let max_analyst = by_analyst.values().copied().max().unwrap_or(1) as f32;

        let compact_dashboard = ui.available_width() < 1200.0;
        if compact_dashboard {
            if self.widget_kpi {
                ui.label(egui::RichText::new("KPI").strong());
                ui.horizontal_wrapped(|ui| {
                    kpi_card(ui, "Open", &open_count.to_string(), egui::Color32::from_rgb(120, 190, 255));
                    kpi_card(ui, "Critical", &critical_count.to_string(), egui::Color32::from_rgb(235, 75, 85));
                    kpi_card(ui, "SLA breaches", &stale_count.to_string(), egui::Color32::from_rgb(245, 140, 70));
                    kpi_card(ui, "MTTA proxy", &format!("{}h", mtta_proxy), egui::Color32::from_rgb(235, 195, 80));
                    kpi_card(ui, "MTTR proxy", &format!("{}h", mttr_proxy), egui::Color32::from_rgb(90, 200, 140));
                });
                ui.add_space(10.0);
            }
            if self.widget_trends {
                ui.label(egui::RichText::new("Trends").strong());
                sparkline_card(
                    ui,
                    "Open trend (24h buckets)",
                    &open_series,
                    egui::Color32::from_rgb(110, 165, 235),
                );
                sparkline_card(
                    ui,
                    "Critical trend (24h buckets)",
                    &crit_series,
                    egui::Color32::from_rgb(235, 75, 85),
                );
                ui.add_space(10.0);
            }
            if self.widget_sources {
                ui.label(egui::RichText::new("Top Sources").strong());
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(24, 30, 42))
                    .rounding(egui::Rounding::same(10.0))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                    .show(ui, |ui| {
                        for (name, count) in by_source.iter().rev() {
                            ui.horizontal(|ui| {
                                ui.label(name);
                                ui.add(
                                    egui::ProgressBar::new((*count as f32 / max_source).clamp(0.0, 1.0))
                                        .desired_width(220.0)
                                        .text(count.to_string()),
                                );
                            });
                        }
                    });
                ui.add_space(10.0);
            }
            if self.widget_severity_mix {
                ui.label(egui::RichText::new("Severity Mix").strong());
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(24, 30, 42))
                    .rounding(egui::Rounding::same(10.0))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                    .show(ui, |ui| {
                        for (sev, count) in by_severity.iter().rev() {
                            ui.horizontal(|ui| {
                                pill_label(ui, sev, severity_color(sev));
                                ui.add(
                                    egui::ProgressBar::new((*count as f32 / max_sev).clamp(0.0, 1.0))
                                        .desired_width(220.0)
                                        .text(count.to_string()),
                                );
                            });
                        }
                    });
            }
        } else {
            ui.columns(2, |cols| {
                if self.widget_kpi {
                    cols[0].label(egui::RichText::new("KPI").strong());
                    cols[0].horizontal_wrapped(|ui| {
                        kpi_card(ui, "Open", &open_count.to_string(), egui::Color32::from_rgb(120, 190, 255));
                        kpi_card(ui, "Critical", &critical_count.to_string(), egui::Color32::from_rgb(235, 75, 85));
                        kpi_card(ui, "SLA breaches", &stale_count.to_string(), egui::Color32::from_rgb(245, 140, 70));
                        kpi_card(ui, "MTTA proxy", &format!("{}h", mtta_proxy), egui::Color32::from_rgb(235, 195, 80));
                        kpi_card(ui, "MTTR proxy", &format!("{}h", mttr_proxy), egui::Color32::from_rgb(90, 200, 140));
                    });
                    cols[0].add_space(10.0);
                }
                if self.widget_sources {
                    cols[0].label(egui::RichText::new("Top Sources").strong());
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(24, 30, 42))
                        .rounding(egui::Rounding::same(10.0))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
                        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                        .show(&mut cols[0], |ui| {
                            for (name, count) in by_source.iter().rev() {
                                ui.horizontal(|ui| {
                                    ui.label(name);
                                    ui.add(
                                        egui::ProgressBar::new((*count as f32 / max_source).clamp(0.0, 1.0))
                                            .desired_width(170.0)
                                            .text(count.to_string()),
                                    );
                                });
                            }
                        });
                }

                if self.widget_trends {
                    cols[1].label(egui::RichText::new("Trends").strong());
                    cols[1].vertical(|ui| {
                        sparkline_card(
                            ui,
                            "Open trend (24h buckets)",
                            &open_series,
                            egui::Color32::from_rgb(110, 165, 235),
                        );
                        sparkline_card(
                            ui,
                            "Critical trend (24h buckets)",
                            &crit_series,
                            egui::Color32::from_rgb(235, 75, 85),
                        );
                    });
                    cols[1].add_space(10.0);
                }
                if self.widget_severity_mix {
                    cols[1].label(egui::RichText::new("Severity Mix").strong());
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(24, 30, 42))
                        .rounding(egui::Rounding::same(10.0))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
                        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                        .show(&mut cols[1], |ui| {
                            for (sev, count) in by_severity.iter().rev() {
                                ui.horizontal(|ui| {
                                    pill_label(ui, sev, severity_color(sev));
                                    ui.add(
                                        egui::ProgressBar::new((*count as f32 / max_sev).clamp(0.0, 1.0))
                                            .desired_width(170.0)
                                            .text(count.to_string()),
                                    );
                                });
                            }
                        });
                }
            });
        }

        if self.widget_analyst_load {
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Analyst Load").strong());
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(24, 30, 42))
                .rounding(egui::Rounding::same(10.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
                .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    for (name, count) in by_analyst.iter().rev().take(8) {
                        ui.horizontal(|ui| {
                            ui.label(name);
                            ui.add(
                                egui::ProgressBar::new((*count as f32 / max_analyst).clamp(0.0, 1.0))
                                    .desired_width(260.0)
                                    .text(count.to_string()),
                            );
                        });
                    }
                });
        }

        if self.widget_audit {
            ui.add_space(10.0);
            ui.label(egui::RichText::new("Audit trail (latest)").strong());
            egui::ScrollArea::vertical().max_height(130.0).show(ui, |ui| {
                if self.audit_log.is_empty() {
                    ui.label("No audit events yet.");
                } else {
                    for event in self.audit_log.iter().take(10) {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(&event.timestamp).monospace().small());
                            ui.label(egui::RichText::new(&event.actor).strong());
                            ui.label(&event.action);
                        });
                    }
                }
            });
        }
        ui.add_space(8.0);
        ui.label("Custom dashboard widgets inspired by Grafana-style panel composition.");
    }

    fn show_alerts_panel(&mut self, ui: &mut egui::Ui) {
        let total_alerts = self.alerts.len();
        let firing_alerts = self
            .alerts
            .iter()
            .filter(|a| matches!(a.state, AlertState::Firing))
            .count();
        let critical_alerts = self
            .alerts
            .iter()
            .filter(|a| a.severity.eq_ignore_ascii_case("critical"))
            .count();
        let ack_alerts = total_alerts.saturating_sub(firing_alerts);

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(12.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Alerts Triage")
                            .strong()
                            .size(24.0)
                            .color(egui::Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new("Queue for ack, investigation, enrichment and case promotion")
                            .small()
                            .color(egui::Color32::from_rgb(150, 165, 188)),
                    );
                });
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Refresh live alerts").clicked() {
                        self.fetch_alerts();
                    }
                    if self.alerts_loading {
                        ui.spinner();
                    }
                    kpi_card(ui, "Total", &total_alerts.to_string(), egui::Color32::from_rgb(110, 165, 235));
                    kpi_card(
                        ui,
                        "Firing",
                        &firing_alerts.to_string(),
                        egui::Color32::from_rgb(235, 75, 85),
                    );
                    kpi_card(
                        ui,
                        "Critical",
                        &critical_alerts.to_string(),
                        egui::Color32::from_rgb(245, 140, 70),
                    );
                    kpi_card(
                        ui,
                        "Acknowledged",
                        &ack_alerts.to_string(),
                        egui::Color32::from_rgb(90, 200, 140),
                    );
                });
            });
        ui.add_space(10.0);
        let mut promote_idx: Option<usize> = None;
        let mut audit_ack: Option<String> = None;
        let mut ack_alert_id: Option<String> = None;
        let mut open_investigation: Option<String> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, alert) in self.alerts.iter_mut().enumerate() {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(24, 30, 42))
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                    .rounding(egui::Rounding::same(10.0))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(&alert.id).monospace().small());
                            pill_label(ui, &alert.severity, severity_color(&alert.severity));
                            let state_color = match alert.state {
                                AlertState::Firing => egui::Color32::from_rgb(235, 75, 85),
                                AlertState::Acknowledged => egui::Color32::from_rgb(110, 165, 235),
                            };
                            let state_text = match alert.state {
                                AlertState::Firing => "Firing",
                                AlertState::Acknowledged => "Ack",
                            };
                            pill_label(ui, state_text, state_color);
                        });
                        ui.label(egui::RichText::new(&alert.title).strong().size(15.0));
                        ui.label(format!(
                            "Source: {} · MITRE: {} · Fired: {}",
                            alert.source, alert.mitre_tactic, alert.fired_at
                        ));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Acknowledge").clicked() {
                                alert.state = AlertState::Acknowledged;
                                self.status = format!("{} acknowledged", alert.id);
                                ack_alert_id = Some(alert.id.clone());
                                audit_ack = Some(format!("Acknowledged alert {}", alert.id));
                            }
                            if ui.button("Investigate").clicked() {
                                open_investigation = Some(alert.id.clone());
                            }
                            if ui.button("Promote to Case").clicked() {
                                promote_idx = Some(i);
                            }
                        });
                    });
                ui.add_space(8.0);
            }
        });
        if let Some(i) = promote_idx {
            if let Some(alert) = self.alerts.get(i).cloned() {
                match self.promote_alert_to_case(&alert) {
                    Ok(_) => {
                        self.status = format!("Alert {} promoted to case", alert.id);
                        self.append_audit(format!("Promoted alert {} to case", alert.id));
                        self.alerts.remove(i);
                        self.fetch_alerts();
                    }
                    Err(e) => self.status = format!("Promote failed: {e}"),
                }
            }
        }
        if let Some(msg) = audit_ack {
            self.append_audit(msg);
        }
        if let Some(alert_id) = ack_alert_id {
            if let Some(i) = self.selected {
                if let Some(case) = self.cases.get(i) {
                    let _ = self.add_timeline_remote(&case.id, &format!("Acknowledged alert {alert_id}"));
                }
            }
        }
        if let Some(entity) = open_investigation {
            self.investigation_entity = entity.clone();
            self.fetch_investigation_for_entity(&entity);
            self.section = Section::Investigations;
        }
    }

    fn show_stack_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Обзор стека")
                .strong()
                .size(22.0)
                .color(egui::Color32::WHITE),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Открой сервисы в браузере — дашборды и метрики остаются там, пока мы не встроим графики в Operator.")
                .size(13.0)
                .color(egui::Color32::from_rgb(150, 160, 178)),
        );
        ui.add_space(18.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Refresh Prometheus + Alertmanager").clicked() {
                self.fetch_observability_snapshot();
            }
            if ui.button("Refresh Portal stack status").clicked() {
                self.fetch_stack_status();
            }
            if self.obs_loading {
                ui.spinner();
                ui.label("loading...");
            }
            if self.stack_status_loading {
                ui.spinner();
                ui.label("stack...");
            }
            if let Some(s) = &self.obs_snapshot {
                ui.label(
                    egui::RichText::new(format!(
                        "last sync: {} | prom up {}/{} | am active {}",
                        s.fetched_at, s.prom_up_targets, s.prom_total_targets, s.am_alerts_active
                    ))
                    .small(),
                );
            }
        });
        if let Some(s) = &self.obs_snapshot {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                kpi_card(ui, "Prometheus version", &s.prom_version, egui::Color32::from_rgb(110, 165, 235));
                kpi_card(
                    ui,
                    "Targets up",
                    &format!("{}/{}", s.prom_up_targets, s.prom_total_targets),
                    egui::Color32::from_rgb(90, 200, 140),
                );
                kpi_card(
                    ui,
                    "AM active alerts",
                    &s.am_alerts_active.to_string(),
                    egui::Color32::from_rgb(235, 75, 85),
                );
                kpi_card(
                    ui,
                    "AM silenced",
                    &s.am_alerts_silenced.to_string(),
                    egui::Color32::from_rgb(235, 195, 80),
                );
            });
        }
        if !self.stack_status.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Portal health checks").strong());
            egui::Grid::new("stack_panel_grid").striped(true).show(ui, |ui| {
                ui.strong("Service");
                ui.strong("Status");
                ui.strong("Details");
                ui.end_row();
                for row in &self.stack_status {
                    ui.label(&row.service);
                    pill_label(
                        ui,
                        &row.status,
                        if row.status.eq_ignore_ascii_case("up") {
                            egui::Color32::from_rgb(90, 200, 140)
                        } else {
                            egui::Color32::from_rgb(235, 75, 85)
                        },
                    );
                    ui.label(&row.detail);
                    ui.end_row();
                }
            });
        }

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 12.0;
            stack_action_card(
                ui,
                "Grafana",
                "http://localhost:3000",
                "Дашборды, визуализация, Explore.",
            );
            stack_action_card(
                ui,
                "SIEM Portal",
                "http://localhost:8091",
                "Единая веб-точка входа и прокси к стеку.",
            );
            stack_action_card(
                ui,
                "Prometheus",
                "http://localhost:9090",
                "Запросы к метрикам, targets, alerts.",
            );
            stack_action_card(
                ui,
                "Alertmanager",
                "http://localhost:9093",
                "Маршрутизация и тишина алертов.",
            );
            stack_action_card(
                ui,
                "Case management (веб)",
                &format!("{}/", self.api_base.trim_end_matches('/')),
                "Тот же хост, что и API — список кейсов и UI.",
            );
        });
    }

    fn show_connection_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Подключение")
                .strong()
                .size(22.0)
                .color(egui::Color32::WHITE),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Базовый URL case-management (тот же, что у веб-приложения). Можно задать переменной SIEM_OPERATOR_API.")
                .size(13.0)
                .color(egui::Color32::from_rgb(150, 160, 178)),
        );
        ui.add_space(16.0);

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(30, 36, 48))
            .rounding(egui::Rounding::same(10.0))
            .inner_margin(egui::Margin::same(18.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    egui::RichText::new("URL API")
                        .small()
                        .color(egui::Color32::from_rgb(140, 150, 168)),
                );
                ui.add_space(6.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.api_base)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [140.0, 38.0],
                            egui::Button::new(
                                egui::RichText::new("Сохранить и обновить").color(egui::Color32::WHITE),
                            ),
                        )
                        .clicked()
                    {
                        self.trim_api_base();
                        self.fetch_cases();
                        self.status = "URL обновлён, загрузка кейсов…".to_string();
                    }
                    if ui.button("Сброс на env / localhost").clicked() {
                        self.api_base =
                            std::env::var("SIEM_OPERATOR_API").unwrap_or_else(|_| "http://127.0.0.1:8088".to_string());
                        self.fetch_cases();
                    }
                });
            });

        ui.add_space(20.0);
        ui.label(
            egui::RichText::new("Подсказка")
                .strong()
                .color(egui::Color32::from_rgb(200, 210, 225)),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Если список кейсов пуст и внизу «Ошибка подключения» — подними docker compose и проверь GET /health на том же хосте.")
                .size(13.0)
                .color(egui::Color32::from_rgb(150, 160, 178)),
        );
    }

    fn filtered_events(&self) -> Vec<&EventRow> {
        self.events
            .iter()
            .filter(|e| {
                if !self.event_filters.search.trim().is_empty() {
                    let n = self.event_filters.search.to_lowercase();
                    let hay = format!("{} {} {}", e.title, e.source, e.id).to_lowercase();
                    if !hay.contains(&n) {
                        return false;
                    }
                }
                if !self.event_filters.severity.is_empty()
                    && !e.severity.eq_ignore_ascii_case(&self.event_filters.severity)
                {
                    return false;
                }
                if !self.event_filters.state.is_empty()
                    && !e.state.eq_ignore_ascii_case(&self.event_filters.state)
                {
                    return false;
                }
                if self.event_filters.silenced_only && !e.silenced {
                    return false;
                }
                true
            })
            .collect()
    }

    fn filtered_assets(&self) -> Vec<&AssetRow> {
        self.assets
            .iter()
            .filter(|a| {
                if !self.asset_filters.search.trim().is_empty()
                    && !a.name.to_lowercase().contains(&self.asset_filters.search.to_lowercase())
                {
                    return false;
                }
                if !self.asset_filters.risk.is_empty()
                    && !a.risk.eq_ignore_ascii_case(&self.asset_filters.risk)
                {
                    return false;
                }
                if !self.asset_filters.source.is_empty()
                    && !a.source.eq_ignore_ascii_case(&self.asset_filters.source)
                {
                    return false;
                }
                if self.asset_filters.stale_only && a.stale_cases == 0 {
                    return false;
                }
                true
            })
            .collect()
    }

    fn show_events_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(12.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Events").strong().size(24.0));
                    ui.label(
                        egui::RichText::new("Realtime flow of alerts and observability events")
                            .small()
                            .color(egui::Color32::from_rgb(150, 165, 188)),
                    );
                    if ui.button("Refresh").clicked() {
                        self.fetch_events();
                    }
                    if self.events_loading {
                        ui.spinner();
                    }
                });
            });
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                let compact = self.is_compact(ui);
                if compact {
                    ui.vertical(|ui| {
                        ui.label("Search:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.event_filters.search)
                                .id_source("event_search")
                                .desired_width(f32::INFINITY),
                        );
                        egui::ComboBox::from_label("Severity")
                            .selected_text(if self.event_filters.severity.is_empty() { "All" } else { &self.event_filters.severity })
                            .show_ui(ui, |ui| {
                                for v in ["All", "critical", "high", "medium", "low"] {
                                    if ui.selectable_label(self.event_filters.severity == v || (self.event_filters.severity.is_empty() && v == "All"), v).clicked() {
                                        self.event_filters.severity = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("State")
                            .selected_text(if self.event_filters.state.is_empty() { "All" } else { &self.event_filters.state })
                            .show_ui(ui, |ui| {
                                for v in ["All", "active", "suppressed", "unprocessed"] {
                                    if ui.selectable_label(self.event_filters.state == v || (self.event_filters.state.is_empty() && v == "All"), v).clicked() {
                                        self.event_filters.state = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        ui.checkbox(&mut self.event_filters.silenced_only, "Silenced only");
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Search:");
                        ui.add(egui::TextEdit::singleline(&mut self.event_filters.search).id_source("event_search"));
                        egui::ComboBox::from_label("Severity")
                            .selected_text(if self.event_filters.severity.is_empty() { "All" } else { &self.event_filters.severity })
                            .show_ui(ui, |ui| {
                                for v in ["All", "critical", "high", "medium", "low"] {
                                    if ui.selectable_label(self.event_filters.severity == v || (self.event_filters.severity.is_empty() && v == "All"), v).clicked() {
                                        self.event_filters.severity = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("State")
                            .selected_text(if self.event_filters.state.is_empty() { "All" } else { &self.event_filters.state })
                            .show_ui(ui, |ui| {
                                for v in ["All", "active", "suppressed", "unprocessed"] {
                                    if ui.selectable_label(self.event_filters.state == v || (self.event_filters.state.is_empty() && v == "All"), v).clicked() {
                                        self.event_filters.state = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        ui.checkbox(&mut self.event_filters.silenced_only, "Silenced only");
                    });
                }
            });
        ui.add_space(8.0);
        let rows = self.filtered_events();
        ui.label(format!("Events shown: {}", rows.len()));
        egui::ScrollArea::vertical().show(ui, |ui| {
            for e in rows {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(30, 36, 48))
                    .inner_margin(egui::Margin::same(10.0))
                    .rounding(egui::Rounding::same(8.0))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            pill_label(ui, &e.severity, severity_color(&e.severity));
                            ui.label(egui::RichText::new(&e.state).small());
                            if e.silenced {
                                pill_label(ui, "silenced", egui::Color32::from_rgb(235, 195, 80));
                            }
                            ui.label(egui::RichText::new(&e.id).monospace().small());
                        });
                        ui.label(egui::RichText::new(&e.title).strong());
                        ui.label(format!("source: {} · started: {}", e.source, e.started_at));
                    });
                ui.add_space(6.0);
            }
        });
    }

    fn show_assets_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(12.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Assets").strong().size(24.0));
                    ui.label(
                        egui::RichText::new("Risk posture and workload across hosts and owners")
                            .small()
                            .color(egui::Color32::from_rgb(150, 165, 188)),
                    );
                    if ui.button("Refresh").clicked() {
                        self.fetch_assets();
                    }
                    if self.assets_loading {
                        ui.spinner();
                    }
                });
            });
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                if self.is_compact(ui) {
                    ui.vertical(|ui| {
                        ui.label("Search:");
                        ui.add(egui::TextEdit::singleline(&mut self.asset_filters.search).desired_width(f32::INFINITY));
                        egui::ComboBox::from_label("Risk")
                            .selected_text(if self.asset_filters.risk.is_empty() { "All" } else { &self.asset_filters.risk })
                            .show_ui(ui, |ui| {
                                for v in ["All", "critical", "high", "normal"] {
                                    if ui.selectable_label(self.asset_filters.risk == v || (self.asset_filters.risk.is_empty() && v == "All"), v).clicked() {
                                        self.asset_filters.risk = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("Source")
                            .selected_text(if self.asset_filters.source.is_empty() { "All" } else { &self.asset_filters.source })
                            .show_ui(ui, |ui| {
                                for v in ["All", "SIEM", "Identity", "Network", "Endpoint"] {
                                    if ui.selectable_label(self.asset_filters.source == v || (self.asset_filters.source.is_empty() && v == "All"), v).clicked() {
                                        self.asset_filters.source = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        ui.checkbox(&mut self.asset_filters.stale_only, "SLA stale only");
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Search:");
                        ui.add(egui::TextEdit::singleline(&mut self.asset_filters.search));
                        egui::ComboBox::from_label("Risk")
                            .selected_text(if self.asset_filters.risk.is_empty() { "All" } else { &self.asset_filters.risk })
                            .show_ui(ui, |ui| {
                                for v in ["All", "critical", "high", "normal"] {
                                    if ui.selectable_label(self.asset_filters.risk == v || (self.asset_filters.risk.is_empty() && v == "All"), v).clicked() {
                                        self.asset_filters.risk = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        egui::ComboBox::from_label("Source")
                            .selected_text(if self.asset_filters.source.is_empty() { "All" } else { &self.asset_filters.source })
                            .show_ui(ui, |ui| {
                                for v in ["All", "SIEM", "Identity", "Network", "Endpoint"] {
                                    if ui.selectable_label(self.asset_filters.source == v || (self.asset_filters.source.is_empty() && v == "All"), v).clicked() {
                                        self.asset_filters.source = if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                        ui.checkbox(&mut self.asset_filters.stale_only, "SLA stale only");
                    });
                }
            });
        ui.add_space(8.0);
        let rows = self.filtered_assets();
        ui.label(format!("Assets shown: {}", rows.len()));
        egui::Grid::new("assets_grid").striped(true).show(ui, |ui| {
            ui.strong("Asset");
            ui.strong("Source");
            ui.strong("Risk");
            ui.strong("Open cases");
            ui.strong("SLA stale");
            ui.end_row();
            for a in rows {
                ui.label(&a.name);
                ui.label(&a.source);
                pill_label(ui, &a.risk, severity_color(&a.risk));
                ui.label(a.open_cases.to_string());
                ui.label(a.stale_cases.to_string());
                ui.end_row();
            }
        });
    }

    fn show_settings_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(12.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Settings").strong().size(24.0));
                ui.label(
                    egui::RichText::new("Visual style, behavior, and connection settings")
                        .small()
                        .color(egui::Color32::from_rgb(150, 165, 188)),
                );
            });
        ui.add_space(10.0);

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Appearance").strong());
                ui.checkbox(&mut self.use_light_theme, "Light theme");
                ui.checkbox(&mut self.compact_mode, "Compact mode (force compact layout)");
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Custom wallpaper").strong());
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .selectable_label(self.wallpaper_preset == "midnight", "Midnight")
                        .clicked()
                    {
                        self.wallpaper_preset = "midnight".to_string();
                    }
                    if ui
                        .selectable_label(self.wallpaper_preset == "graphite", "Graphite")
                        .clicked()
                    {
                        self.wallpaper_preset = "graphite".to_string();
                    }
                    if ui
                        .selectable_label(self.wallpaper_preset == "ocean", "Ocean")
                        .clicked()
                    {
                        self.wallpaper_preset = "ocean".to_string();
                    }
                    if ui
                        .selectable_label(self.wallpaper_preset == "sunset", "Sunset")
                        .clicked()
                    {
                        self.wallpaper_preset = "sunset".to_string();
                    }
                    if ui
                        .selectable_label(self.wallpaper_preset == "custom", "Custom")
                        .clicked()
                    {
                        self.wallpaper_preset = "custom".to_string();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Tint R");
                    ui.add(egui::Slider::new(&mut self.wallpaper_tint[0], 0..=255));
                    ui.label("G");
                    ui.add(egui::Slider::new(&mut self.wallpaper_tint[1], 0..=255));
                    ui.label("B");
                    ui.add(egui::Slider::new(&mut self.wallpaper_tint[2], 0..=255));
                });
            });

        ui.add_space(10.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Behavior").strong());
                ui.checkbox(&mut self.auto_refresh_enabled, "Auto refresh");
                ui.add(
                    egui::Slider::new(&mut self.auto_refresh_interval_sec, 10..=120)
                        .text("Auto refresh interval")
                        .suffix("s"),
                );
                ui.checkbox(&mut self.auto_triage_enabled, "Auto-triage");
            });

        ui.add_space(10.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Connection").strong());
                ui.add(
                    egui::TextEdit::singleline(&mut self.api_base)
                        .desired_width(f32::INFINITY)
                        .hint_text("API / Portal URL"),
                );
                ui.add_space(6.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.detection_engine_url)
                        .desired_width(f32::INFINITY)
                        .hint_text("Detection-engine URL (e.g. http://127.0.0.1:9111)"),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Apply URL and refresh").clicked() {
                        self.api_base = self.api_base.trim().to_string();
                        self.fetch_cases();
                        self.fetch_alerts();
                        self.fetch_events();
                        self.fetch_detections();
                        self.fetch_detection_stats();
                        self.fetch_stack_status();
                        self.fetch_overview_metrics_series();
                        self.fetch_assets();
                        self.fetch_observability_snapshot();
                        self.status = "Connection settings applied".to_string();
                    }
                    if ui.button("Reset to default").clicked() {
                        self.api_base = std::env::var("SIEM_OPERATOR_API")
                            .unwrap_or_else(|_| "http://127.0.0.1:8088".to_string());
                        self.detection_engine_url = std::env::var("SIEM_OPERATOR_DETECTION_URL")
                            .unwrap_or_else(|_| "http://127.0.0.1:9111".to_string());
                    }
                });
            });
    }

    fn fetch_observability_snapshot(&mut self) {
        self.obs_loading = true;
        let base = self.portal_base();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<ObsSnapshot, String> {
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(8))
                    .build()
                    .map_err(|e| e.to_string())?;

                let prom_build_url = format!("{base}/api/v1/proxy/prometheus/query?query=prometheus_build_info");
                let prom_ver_resp = client
                    .get(&prom_build_url)
                    .send()
                    .map_err(|e| format!("prom buildinfo: {e}"))?;
                let prom_ver: serde_json::Value = prom_ver_resp.json().map_err(|e| e.to_string())?;
                let prom_version = prom_ver["data"]["result"][0]["metric"]["version"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();

                let prom_targets_url = format!("{base}/api/v1/proxy/prometheus/query?query=up");
                let prom_targets_resp = client
                    .get(&prom_targets_url)
                    .send()
                    .map_err(|e| format!("prom targets: {e}"))?;
                let prom_targets: serde_json::Value = prom_targets_resp.json().map_err(|e| e.to_string())?;
                let active = prom_targets["data"]["result"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let prom_total_targets = active.len();
                let prom_up_targets = active
                    .iter()
                    .filter(|t| t["value"][1].as_str().unwrap_or_default() == "1")
                    .count();

                let am_url = format!("{base}/api/v1/proxy/alertmanager/v2/alerts");
                let am_resp = client
                    .get(&am_url)
                    .send()
                    .map_err(|e| format!("alertmanager alerts: {e}"))?;
                let am_alerts: serde_json::Value = am_resp.json().map_err(|e| e.to_string())?;
                let arr = am_alerts.as_array().cloned().unwrap_or_default();
                let am_alerts_active = arr
                    .iter()
                    .filter(|a| a["status"]["state"].as_str().unwrap_or_default() == "active")
                    .count();
                let am_alerts_silenced = arr
                    .iter()
                    .filter(|a| {
                        a["status"]["silencedBy"]
                            .as_array()
                            .map(|x| !x.is_empty())
                            .unwrap_or(false)
                    })
                    .count();

                Ok(ObsSnapshot {
                    fetched_at: Utc::now().to_rfc3339(),
                    prom_total_targets,
                    prom_up_targets,
                    prom_version,
                    am_alerts_active,
                    am_alerts_silenced,
                })
            })();
            let _ = tx.send(result);
        });
        self.obs_rx = Some(rx);
    }

}

impl eframe::App for OperatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.use_light_theme {
            ctx.set_visuals(egui::Visuals::light());
        } else {
            ctx.set_visuals(egui::Visuals::dark());
        }
        self.apply_hotkeys(ctx);
        self.tick_auto_refresh(ctx);
        if let Some(rx) = &self.obs_rx {
            match rx.try_recv() {
                Ok(Ok(snapshot)) => {
                    self.obs_snapshot = Some(snapshot);
                    self.obs_loading = false;
                    self.obs_rx = None;
                    self.status = "Observability snapshot updated".to_string();
                }
                Ok(Err(e)) => {
                    self.obs_loading = false;
                    self.obs_rx = None;
                    self.status = format!("Observability error: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.obs_loading = false;
                    self.obs_rx = None;
                }
            }
        }
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Ok(body)) => {
                    self.rx = None;
                    self.loading = false;
                    self.cases = body.cases;
                    self.total = body.total;
                    if self.auto_triage_enabled {
                        self.apply_auto_triage_rules();
                    }
                    self.rebuild_assets_from_cases();
                    self.status = format!(
                        "OK · кейсов в списке: {} · всего в базе: {}",
                        self.cases.len(),
                        self.total
                    );
                }
                Ok(Err(e)) => {
                    self.rx = None;
                    self.loading = false;
                    self.status = format!("Ошибка: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(50));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    self.loading = false;
                    self.status = "Поток оборвался".to_string();
                }
            }
        }
        if let Some(rx) = &self.events_rx {
            match rx.try_recv() {
                Ok(Ok(rows)) => {
                    self.events = rows;
                    self.events_loading = false;
                    self.events_rx = None;
                    self.status = format!("Events synced: {}", self.events.len());
                }
                Ok(Err(e)) => {
                    self.events_loading = false;
                    self.events_rx = None;
                    self.status = format!("Events error: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.events_loading = false;
                    self.events_rx = None;
                }
            }
        }
        if let Some(rx) = &self.detections_rx {
            match rx.try_recv() {
                Ok(Ok(rows)) => {
                    self.detections = rows;
                    self.detections_loading = false;
                    self.detections_rx = None;
                    self.status = format!("Detections synced: {}", self.detections.len());
                }
                Ok(Err(e)) => {
                    self.detections_loading = false;
                    self.detections_rx = None;
                    self.status = format!("Detections error: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.detections_loading = false;
                    self.detections_rx = None;
                }
            }
        }
        if let Some(rx) = &self.investigation_rx {
            match rx.try_recv() {
                Ok(Ok(details)) => {
                    self.investigation_notes = {
                        let mut rows = vec![format!("Summary: {}", details.summary)];
                        rows.extend(details.findings.iter().cloned());
                        rows
                    };
                    self.investigation_details = Some(details);
                    self.investigation_loading = false;
                    self.investigation_rx = None;
                    self.status = "Investigation loaded".to_string();
                }
                Ok(Err(e)) => {
                    self.investigation_loading = false;
                    self.investigation_rx = None;
                    self.status = format!("Investigation error: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.investigation_loading = false;
                    self.investigation_rx = None;
                }
            }
        }
        if let Some(rx) = &self.alerts_rx {
            match rx.try_recv() {
                Ok(Ok(rows)) => {
                    self.alerts = rows;
                    self.alerts_loading = false;
                    self.alerts_rx = None;
                }
                Ok(Err(e)) => {
                    self.alerts_loading = false;
                    self.alerts_rx = None;
                    self.status = format!("Alerts error: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.alerts_loading = false;
                    self.alerts_rx = None;
                }
            }
        }
        if let Some(rx) = &self.timeline_rx {
            match rx.try_recv() {
                Ok(Ok(rows)) => {
                    self.case_timeline = rows;
                    self.timeline_loading = false;
                    self.timeline_rx = None;
                }
                Ok(Err(e)) => {
                    self.timeline_loading = false;
                    self.timeline_rx = None;
                    self.status = format!("Timeline error: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.timeline_loading = false;
                    self.timeline_rx = None;
                }
            }
        }
        if let Some(rx) = &self.stack_status_rx {
            match rx.try_recv() {
                Ok(Ok(rows)) => {
                    self.stack_status = rows;
                    self.stack_status_loading = false;
                    self.stack_status_rx = None;
                }
                Ok(Err(e)) => {
                    self.stack_status_loading = false;
                    self.stack_status_rx = None;
                    self.status = format!("Stack status error: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.stack_status_loading = false;
                    self.stack_status_rx = None;
                }
            }
        }
        if let Some(rx) = &self.detection_stats_rx {
            match rx.try_recv() {
                Ok(Ok(stats)) => {
                    self.detection_stats = Some(stats);
                    self.detection_stats_loading = false;
                    self.detection_stats_rx = None;
                }
                Ok(Err(e)) => {
                    self.detection_stats_loading = false;
                    self.detection_stats_rx = None;
                    self.status = format!("Detection stats error: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.detection_stats_loading = false;
                    self.detection_stats_rx = None;
                }
            }
        }
        if let Some(rx) = &self.metrics_series_rx {
            match rx.try_recv() {
                Ok(Ok((eps, alerts, mtta))) => {
                    self.eps_series = eps;
                    self.alerts_series = alerts;
                    self.mtta_series = mtta;
                    self.metrics_loading = false;
                    self.metrics_series_rx = None;
                }
                Ok(Err(e)) => {
                    self.metrics_loading = false;
                    self.metrics_series_rx = None;
                    self.status = format!("Metrics series error: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.metrics_loading = false;
                    self.metrics_series_rx = None;
                }
            }
        }
        if let Some(rx) = &self.docker_rx {
            match rx.try_recv() {
                Ok(Ok(out)) => {
                    self.docker_loading = false;
                    self.docker_rx = None;
                    self.docker_last_output = out;
                    self.status = "Docker compose command completed".to_string();
                }
                Ok(Err(e)) => {
                    self.docker_loading = false;
                    self.docker_rx = None;
                    self.docker_last_output = e.clone();
                    self.status = format!("Docker compose failed: {e}");
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.docker_loading = false;
                    self.docker_rx = None;
                }
            }
        }

        self.show_top_toolbar(ctx);
        self.show_sidebar(ctx);
        self.show_status_bar(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(self.background_fill_color())
                    .inner_margin(egui::Margin::same(22.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| match self.section {
                        Section::Overview => self.show_home_panel(ui),
                        Section::Detections => self.show_detections_panel(ui),
                        Section::Alerts => self.show_alerts_panel(ui),
                        Section::Events => self.show_events_panel(ui),
                        Section::Investigations => self.show_investigations_panel(ui),
                        Section::Assets => self.show_assets_panel(ui),
                        Section::Cases => self.show_cases_panel(ui),
                        Section::StackControl => self.show_stack_control_panel(ui),
                        Section::Settings => self.show_settings_panel(ui),
                    });
            });
        self.show_critical_confirmation(ctx);
        self.show_command_palette(ctx);

        if self.cases.is_empty() && !self.loading && self.rx.is_none() && self.status.contains("Нажми") {
            self.fetch_cases();
            self.fetch_alerts();
            self.fetch_events();
            self.fetch_detections();
            self.fetch_detection_stats();
            self.fetch_stack_status();
            self.fetch_overview_metrics_series();
            self.fetch_assets();
            self.fetch_observability_snapshot();
        }
        self.maybe_persist_state();
    }
}

