use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use chrono::{DateTime, Utc};
use eframe::egui;

use crate::models::{AlertItem, AlertState, CaseBrief, CasesResponse};
use crate::ui::widgets::{pill_label, section_nav_button, severity_color, stack_action_card};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Section {
    #[default]
    Home,
    Cases,
    Alerts,
    Stack,
    Connection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SavedView {
    All,
    MyQueue,
    Critical24h,
    NoAssignee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserRole {
    Analyst,
    Senior,
    Manager,
}

#[derive(Debug, Clone)]
struct AuditEntry {
    timestamp: String,
    actor: String,
    action: String,
}

#[derive(Debug, Clone)]
enum PendingAction {
    Close { reason: String },
    MoveStatus { status: String },
}

#[derive(Default)]
struct CaseFilters {
    search: String,
    severity: String,
    status: String,
    assignee: String,
    source: String,
    mitre: String,
    stale_only: bool,
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
    active_view: SavedView,
    close_reason: String,
    whoami: String,
    palette_open: bool,
    palette_query: String,
    obs_loading: bool,
    obs_rx: Option<Receiver<Result<ObsSnapshot, String>>>,
    obs_snapshot: Option<ObsSnapshot>,
    role: UserRole,
    pending_action: Option<PendingAction>,
    audit_log: Vec<AuditEntry>,
    auto_triage_enabled: bool,
    playbook_steps: Vec<(String, bool)>,
}

impl Default for OperatorApp {
    fn default() -> Self {
        let api_base =
            std::env::var("SIEM_OPERATOR_API").unwrap_or_else(|_| "http://127.0.0.1:8088".to_string());
        Self {
            api_base,
            section: Section::default(),
            cases: Vec::new(),
            total: 0,
            status: "Нажми «Обновить» или дождись авто-загрузки.".to_string(),
            loading: false,
            rx: None,
            selected: None,
            alerts: seed_alerts(),
            filters: CaseFilters::default(),
            active_view: SavedView::All,
            close_reason: String::new(),
            whoami: "analyst".to_string(),
            palette_open: false,
            palette_query: String::new(),
            obs_loading: false,
            obs_rx: None,
            obs_snapshot: None,
            role: UserRole::Analyst,
            pending_action: None,
            audit_log: Vec::new(),
            auto_triage_enabled: true,
            playbook_steps: vec![
                ("Validate alert context".to_string(), false),
                ("Collect IOC artifacts".to_string(), false),
                ("Contain impacted asset".to_string(), false),
                ("Document evidence".to_string(), false),
            ],
        }
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
            self.section = Section::Cases;
            ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("case_search")));
        }
        if do_command_palette {
            self.palette_open = true;
            ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("command_palette_input")));
        }
    }

    fn fetch_cases(&mut self) {
        self.loading = true;
        self.status = "Загрузка…".to_string();
        let base = self.api_base.trim_end_matches('/').to_string();
        let url = format!("{base}/api/v1/cases?limit=300");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<CasesResponse, String> {
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(20))
                    .build()
                    .map_err(|e| e.to_string())?;
                let resp = client.get(&url).send().map_err(|e| e.to_string())?;
                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }
                resp.json::<CasesResponse>().map_err(|e| e.to_string())
            })();
            let _ = tx.send(result);
        });
        self.rx = Some(rx);
    }

    fn trim_api_base(&mut self) {
        self.api_base = self.api_base.trim().to_string();
    }

    fn assign_selected_to_me(&mut self) {
        let mut audit: Option<String> = None;
        if let Some(i) = self.selected {
            if let Some(case) = self.cases.get_mut(i) {
                case.assignee = Some(self.whoami.clone());
                self.status = format!("{} assigned to {}", case.display_key, self.whoami);
                audit = Some(format!("Assigned {} to {}", case.display_key, self.whoami));
            }
        }
        if let Some(a) = audit {
            self.append_audit(a);
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
        let mut audit: Option<String> = None;
        if let Some(i) = self.selected {
            if let Some(case) = self.cases.get_mut(i) {
                case.status = "Closed".to_string();
                self.status = format!("{} closed: {}", case.display_key, reason);
                audit = Some(format!("Closed {} ({})", case.display_key, reason));
            }
        }
        if let Some(a) = audit {
            self.append_audit(a);
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
        let mut audit: Option<String> = None;
        if let Some(i) = self.selected {
            if let Some(case) = self.cases.get_mut(i) {
                case.status = status.to_string();
                self.status = format!("{} -> {}", case.display_key, status);
                audit = Some(format!("Transition {} -> {}", case.display_key, status));
            }
        }
        if let Some(a) = audit {
            self.append_audit(a);
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
                ui.vertical(|ui| {
                    ui.add_space(4.0);
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
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new("Разделы")
                            .small()
                            .color(egui::Color32::from_rgb(120, 128, 145)),
                    );
                    ui.add_space(8.0);
                    if section_nav_button(ui, "Кейсы", "список и действия", self.section == Section::Cases) {
                        self.section = Section::Cases;
                    }
                    if section_nav_button(ui, "Dashboard", "KPI и SLA", self.section == Section::Home) {
                        self.section = Section::Home;
                    }
                    if section_nav_button(ui, "Alerts", "Inbox и Promote", self.section == Section::Alerts) {
                        self.section = Section::Alerts;
                    }
                    if section_nav_button(
                        ui,
                        "Обзор стека",
                        "Grafana, Portal, метрики",
                        self.section == Section::Stack,
                    ) {
                        self.section = Section::Stack;
                    }
                    if section_nav_button(
                        ui,
                        "Подключение",
                        "URL API и окружение",
                        self.section == Section::Connection,
                    ) {
                        self.section = Section::Connection;
                    }
                });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(8.0);
                    if ui
                        .add_sized([ui.available_width(), 36.0], egui::Button::new("Выход из приложения"))
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    ui.label(
                        egui::RichText::new("v0.2")
                            .small()
                            .color(egui::Color32::from_rgb(90, 98, 115)),
                    );
                    ui.label(
                        egui::RichText::new("Hotkeys: R / A / C / /")
                            .small()
                            .color(egui::Color32::from_rgb(90, 98, 115)),
                    );
                });
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

    fn show_cases_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Кейсы")
                    .strong()
                    .size(22.0)
                    .color(egui::Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized(
                        [130.0, 38.0],
                        egui::Button::new(egui::RichText::new("⟳  Обновить").color(egui::Color32::WHITE)),
                    )
                    .clicked()
                {
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
        let case_key = case.display_key.clone();
        let case_title = case.title.clone();
        let case_status = case.status.clone();
        let case_created = case.created_at.clone();
        let case_assignee = case.assignee.clone().unwrap_or_else(|| "Unassigned".to_string());
        let case_stale = Self::is_stale(case);
        let mut rows = vec![
            (
                case_created.clone(),
                "Case created".to_string(),
                format!("{} {}", case_key, case_title),
            ),
            (
                case_created.clone(),
                "Status snapshot".to_string(),
                case_status,
            ),
            (
                Utc::now().to_rfc3339(),
                "Assignee".to_string(),
                case_assignee,
            ),
        ];
        if case_stale {
            rows.push((
                Utc::now().to_rfc3339(),
                "SLA".to_string(),
                "Breach detected".to_string(),
            ));
        }
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(30, 36, 48))
            .inner_margin(egui::Margin::same(10.0))
            .rounding(egui::Rounding::same(8.0))
            .show(ui, |ui| {
                for (ts, ev, details) in rows {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(ts).monospace().small());
                        ui.label(egui::RichText::new(ev).strong());
                        ui.label(details);
                    });
                    ui.separator();
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

        ui.heading("Home Dashboard");
        ui.label("Оперативная сводка по кейсам и SLA.");
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
            ui.checkbox(&mut self.auto_triage_enabled, "Auto-triage rules");
            if ui.button("Run triage now").clicked() {
                self.apply_auto_triage_rules();
            }
        });
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            kpi_card(ui, "Open", &open_count.to_string(), egui::Color32::from_rgb(120, 190, 255));
            kpi_card(
                ui,
                "Critical",
                &critical_count.to_string(),
                egui::Color32::from_rgb(235, 75, 85),
            );
            kpi_card(
                ui,
                "SLA breaches",
                &stale_count.to_string(),
                egui::Color32::from_rgb(245, 140, 70),
            );
            kpi_card(
                ui,
                "MTTA proxy",
                &format!("{}h", mtta_proxy),
                egui::Color32::from_rgb(235, 195, 80),
            );
            kpi_card(
                ui,
                "MTTR proxy",
                &format!("{}h", mttr_proxy),
                egui::Color32::from_rgb(90, 200, 140),
            );
        });
        ui.add_space(12.0);
        let (open_series, crit_series) = build_case_sparkline_series(&self.cases);
        ui.horizontal_wrapped(|ui| {
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
        ui.add_space(12.0);
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
        ui.add_space(8.0);
        ui.label("MTTA/MTTR пока считаются как прокси по age; после API событий подключим точный расчет.");
    }

    fn show_alerts_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Alerts Inbox");
        ui.label("Минимальный поток: alert -> Promote to Case.");
        ui.add_space(10.0);
        let mut promote_idx: Option<usize> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, alert) in self.alerts.iter_mut().enumerate() {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(30, 36, 48))
                    .inner_margin(egui::Margin::same(10.0))
                    .rounding(egui::Rounding::same(8.0))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(&alert.id).monospace());
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
                        ui.label(egui::RichText::new(&alert.title).strong());
                        ui.label(format!(
                            "Source: {} · MITRE: {} · Fired: {}",
                            alert.source, alert.mitre_tactic, alert.fired_at
                        ));
                        ui.horizontal(|ui| {
                            if ui.button("Acknowledge").clicked() {
                                alert.state = AlertState::Acknowledged;
                                self.status = format!("{} acknowledged", alert.id);
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
                let new_case = CaseBrief {
                    id: format!("promoted-{}", alert.id),
                    display_key: format!("CASE-{}", self.cases.len() + 1),
                    title: alert.title.clone(),
                    severity: alert.severity.clone(),
                    status: "New".to_string(),
                    assignee: None,
                    created_at: Utc::now().to_rfc3339(),
                };
                self.cases.insert(0, new_case);
                self.total += 1;
                self.status = format!("Alert {} promoted to case", alert.id);
                self.append_audit(format!("Promoted alert {} to case", alert.id));
                self.alerts.remove(i);
            }
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
            if self.obs_loading {
                ui.spinner();
                ui.label("loading...");
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

    fn fetch_observability_snapshot(&mut self) {
        self.obs_loading = true;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<ObsSnapshot, String> {
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(8))
                    .build()
                    .map_err(|e| e.to_string())?;

                let prom_ver_resp = client
                    .get("http://127.0.0.1:9090/api/v1/status/buildinfo")
                    .send()
                    .map_err(|e| format!("prom buildinfo: {e}"))?;
                let prom_ver: serde_json::Value = prom_ver_resp.json().map_err(|e| e.to_string())?;
                let prom_version = prom_ver["data"]["version"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();

                let prom_targets_resp = client
                    .get("http://127.0.0.1:9090/api/v1/targets?state=active")
                    .send()
                    .map_err(|e| format!("prom targets: {e}"))?;
                let prom_targets: serde_json::Value = prom_targets_resp.json().map_err(|e| e.to_string())?;
                let active = prom_targets["data"]["activeTargets"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let prom_total_targets = active.len();
                let prom_up_targets = active
                    .iter()
                    .filter(|t| t["health"].as_str().unwrap_or_default().eq_ignore_ascii_case("up"))
                    .count();

                let am_resp = client
                    .get("http://127.0.0.1:9093/api/v2/alerts")
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

    fn show_command_palette(&mut self, ctx: &egui::Context) {
        if !self.palette_open {
            return;
        }
        let mut open = self.palette_open;
        egui::Window::new("Command Palette")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label("Ctrl+K: переход и быстрые действия");
                ui.add(
                    egui::TextEdit::singleline(&mut self.palette_query)
                        .id_source("command_palette_input")
                        .desired_width(f32::INFINITY)
                        .hint_text("type: cases, alerts, dashboard, refresh, assign, close"),
                );
                ui.separator();
                let q = self.palette_query.to_lowercase();
                let mut action = |keyword: &str, label: &str, f: &mut dyn FnMut(&mut Self)| {
                    if (q.is_empty() || label.to_lowercase().contains(&q) || keyword.contains(&q))
                        && ui.button(label).clicked()
                    {
                        f(self);
                        self.palette_open = false;
                    }
                };
                action("dashboard", "Go: Dashboard", &mut |s| s.section = Section::Home);
                action("cases", "Go: Cases", &mut |s| s.section = Section::Cases);
                action("alerts", "Go: Alerts", &mut |s| s.section = Section::Alerts);
                action("stack", "Go: Stack", &mut |s| s.section = Section::Stack);
                action("refresh", "Action: Refresh cases", &mut |s| s.fetch_cases());
                action("assign", "Action: Assign selected to me", &mut |s| s.assign_selected_to_me());
                action("close", "Action: Close selected", &mut |s| s.close_selected("Closed via command palette"));
                action("obs", "Action: Refresh Prometheus/Alertmanager", &mut |s| {
                    s.fetch_observability_snapshot()
                });
            });
        self.palette_open = open;
    }

    fn show_critical_confirmation(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_action.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Confirm critical action")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Critical case action requires explicit confirmation.");
                ui.label(format!("Role: {}", self.role_label()));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.pending_action = None;
                        self.append_audit("Cancelled critical action".to_string());
                    }
                    if ui.button("Confirm").clicked() {
                        let mut audit: Option<String> = None;
                        match pending.clone() {
                            PendingAction::Close { reason } => {
                                if let Some(i) = self.selected {
                                    if let Some(case) = self.cases.get_mut(i) {
                                        case.status = "Closed".to_string();
                                        self.status = format!("{} closed: {}", case.display_key, reason);
                                        audit = Some(format!(
                                            "Confirmed critical close {} ({})",
                                            case.display_key, reason
                                        ));
                                    }
                                }
                            }
                            PendingAction::MoveStatus { status } => {
                                if let Some(i) = self.selected {
                                    if let Some(case) = self.cases.get_mut(i) {
                                        case.status = status.clone();
                                        self.status = format!("{} -> {}", case.display_key, status);
                                        audit = Some(format!(
                                            "Confirmed critical transition {} -> {}",
                                            case.display_key, status
                                        ));
                                    }
                                }
                            }
                        }
                        if let Some(a) = audit {
                            self.append_audit(a);
                        }
                        self.pending_action = None;
                    }
                });
            });
        if !open {
            self.pending_action = None;
        }
    }
}

impl eframe::App for OperatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_hotkeys(ctx);
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

        self.show_sidebar(ctx);
        self.show_status_bar(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(22, 27, 36))
                    .inner_margin(egui::Margin::same(22.0)),
            )
            .show(ctx, |ui| match self.section {
                Section::Home => self.show_home_panel(ui),
                Section::Cases => self.show_cases_panel(ui),
                Section::Alerts => self.show_alerts_panel(ui),
                Section::Stack => self.show_stack_panel(ui),
                Section::Connection => self.show_connection_panel(ui),
            });
        self.show_critical_confirmation(ctx);
        self.show_command_palette(ctx);

        if self.cases.is_empty() && !self.loading && self.rx.is_none() && self.status.contains("Нажми") {
            self.fetch_cases();
            self.fetch_observability_snapshot();
        }
    }
}

fn average_hours(values: impl Iterator<Item = i64>) -> i64 {
    let v: Vec<i64> = values.collect();
    if v.is_empty() {
        return 0;
    }
    v.iter().sum::<i64>() / i64::try_from(v.len()).unwrap_or(1)
}

fn kpi_card(ui: &mut egui::Ui, label: &str, value: &str, accent: egui::Color32) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(30, 36, 48))
        .rounding(egui::Rounding::same(10.0))
        .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.7)))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_min_width(140.0);
            ui.label(egui::RichText::new(label).small());
            ui.label(egui::RichText::new(value).strong().size(24.0).color(accent));
        });
}

fn build_case_sparkline_series(cases: &[CaseBrief]) -> (Vec<f32>, Vec<f32>) {
    let mut open = vec![0.0_f32; 8];
    let mut critical = vec![0.0_f32; 8];
    for case in cases {
        let age = OperatorApp::case_age_hours(case).unwrap_or(0);
        let bucket = usize::min((age / 3) as usize, 7);
        let idx = 7usize.saturating_sub(bucket);
        if !OperatorApp::is_closed_status(&case.status) {
            open[idx] += 1.0;
        }
        if case.severity.eq_ignore_ascii_case("critical") {
            critical[idx] += 1.0;
        }
    }
    (open, critical)
}

fn sparkline_card(ui: &mut egui::Ui, title: &str, values: &[f32], color: egui::Color32) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(30, 36, 48))
        .rounding(egui::Rounding::same(10.0))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.7)))
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_min_width(250.0);
            ui.label(egui::RichText::new(title).small());
            let desired = egui::vec2(240.0, 52.0);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            if values.len() < 2 {
                return;
            }
            let max = values
                .iter()
                .copied()
                .fold(0.0_f32, f32::max)
                .max(1.0);
            let mut points: Vec<egui::Pos2> = Vec::with_capacity(values.len());
            for (i, v) in values.iter().enumerate() {
                let x = rect.left() + (i as f32 / (values.len() - 1) as f32) * rect.width();
                let y = rect.bottom() - (v / max) * rect.height();
                points.push(egui::pos2(x, y));
            }
            ui.painter()
                .line_segment([egui::pos2(rect.left(), rect.bottom()), egui::pos2(rect.right(), rect.bottom())], egui::Stroke::new(1.0, egui::Color32::from_gray(70)));
            ui.painter()
                .add(egui::Shape::line(points, egui::Stroke::new(2.0, color)));
        });
}

fn seed_alerts() -> Vec<AlertItem> {
    vec![
        AlertItem {
            id: "ALRT-1001".to_string(),
            title: "Multiple failed admin logins from rare geo".to_string(),
            severity: "high".to_string(),
            source: "Identity".to_string(),
            mitre_tactic: "TA0006 Credential Access".to_string(),
            fired_at: Utc::now().to_rfc3339(),
            state: AlertState::Firing,
        },
        AlertItem {
            id: "ALRT-1002".to_string(),
            title: "Suspicious lateral movement via SMB".to_string(),
            severity: "critical".to_string(),
            source: "Network".to_string(),
            mitre_tactic: "TA0008 Lateral Movement".to_string(),
            fired_at: Utc::now().to_rfc3339(),
            state: AlertState::Firing,
        },
        AlertItem {
            id: "ALRT-1003".to_string(),
            title: "EDR detected unsigned powershell execution".to_string(),
            severity: "medium".to_string(),
            source: "Endpoint".to_string(),
            mitre_tactic: "TA0005 Defense Evasion".to_string(),
            fired_at: Utc::now().to_rfc3339(),
            state: AlertState::Acknowledged,
        },
    ]
}
