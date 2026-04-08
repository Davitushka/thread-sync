use eframe::egui;

use crate::models::CaseBrief;
use crate::ui::widgets::{pill_label, severity_color};

use super::{OperatorApp, PendingAction, Section};

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
                action("overview", "Go: Overview", &mut |s| s.section = Section::Overview);
                action("detections", "Go: Detections", &mut |s| s.section = Section::Detections);
                action("alerts", "Go: Alerts", &mut |s| s.section = Section::Alerts);
                action("events", "Go: Events", &mut |s| s.section = Section::Events);
                action("investigations", "Go: Investigations", &mut |s| s.section = Section::Investigations);
                action("assets", "Go: Assets", &mut |s| s.section = Section::Assets);
                action("cases", "Go: Cases", &mut |s| s.section = Section::Cases);
                action("stack", "Go: Stack Control", &mut |s| s.section = Section::StackControl);
                action("settings", "Go: Settings", &mut |s| s.section = Section::Settings);
                action("refresh", "Action: Refresh cases", &mut |s| s.fetch_cases());
                action("refresh events", "Action: Refresh events", &mut |s| s.fetch_events());
                action("refresh assets", "Action: Refresh assets", &mut |s| s.fetch_assets());
                action("refresh detections", "Action: Refresh detections", &mut |s| s.fetch_detections());
                action("docker up", "Action: Docker start stack", &mut |s| s.run_docker_compose_action("up"));
                action("docker down", "Action: Docker stop stack", &mut |s| s.run_docker_compose_action("down"));
                action("docker restart", "Action: Docker restart stack", &mut |s| {
                    s.run_docker_compose_action("restart")
                });
                action("docker ps", "Action: Docker stack status", &mut |s| s.run_docker_compose_action("ps"));
                action("assign", "Action: Assign selected to me", &mut |s| s.assign_selected_to_me());
                action("close", "Action: Close selected", &mut |s| s.close_selected("Closed via command palette"));
                action("obs", "Action: Refresh Prometheus/Alertmanager", &mut |s| {
                    s.fetch_observability_snapshot()
                });
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

    pub(super) fn show_detections_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(12.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Detections").strong().size(24.0));
                    ui.label(
                        egui::RichText::new("Correlated rules, severity and firing states")
                            .small()
                            .color(egui::Color32::from_rgb(150, 165, 188)),
                    );
                });
                ui.add_space(8.0);
                if ui.available_width() < 1100.0 {
                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Refresh").clicked() {
                                self.fetch_detections();
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
                                                || (self.detection_filters.severity.is_empty() && v == "All"),
                                            v,
                                        )
                                        .clicked()
                                    {
                                        self.detection_filters.severity =
                                            if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Refresh").clicked() {
                            self.fetch_detections();
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
                                                || (self.detection_filters.severity.is_empty() && v == "All"),
                                            v,
                                        )
                                        .clicked()
                                    {
                                        self.detection_filters.severity =
                                            if v == "All" { String::new() } else { v.to_string() };
                                    }
                                }
                            });
                    });
                }
            });
        ui.add_space(8.0);
        let mut open_investigation: Option<String> = None;
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| egui::Grid::new("detections_grid").striped(true).show(ui, |ui| {
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
                    || d.severity.eq_ignore_ascii_case(&self.detection_filters.severity);
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
        }));
        if let Some(entity) = open_investigation {
            self.investigation_entity = entity.clone();
            self.section = Section::Investigations;
            self.fetch_investigation_for_entity(&entity);
        }
    }

    pub(super) fn show_investigations_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(12.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
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
                        let title = if self.investigation_entity.trim().is_empty() {
                            "Investigation finding".to_string()
                        } else {
                            format!("Investigation: {}", self.investigation_entity)
                        };
                        self.cases.insert(
                            0,
                            crate::models::CaseBrief {
                                id: format!("inv-{}", chrono::Utc::now().timestamp()),
                                display_key: format!("CASE-{}", self.cases.len() + 1),
                                title,
                                severity: "high".to_string(),
                                status: "New".to_string(),
                                assignee: None,
                                created_at: chrono::Utc::now().to_rfc3339(),
                            },
                        );
                        self.total += 1;
                        self.append_audit("Promoted investigation to case".to_string());
                        self.status = "Investigation promoted to case".to_string();
                        self.section = Section::Cases;
                    }
                });
            });
        ui.separator();
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
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
    }

    pub(super) fn show_stack_control_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(12.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 58, 79)))
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Stack Control").strong().size(24.0));
                ui.label("Docker orchestration and live status for SIEM stack.");
                ui.add_space(8.0);
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
                    if self.docker_loading {
                        ui.spinner();
                    }
                });
            });
        ui.add_space(8.0);
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(24, 30, 42))
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 56, 74)))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.label(egui::RichText::new(&self.docker_last_output).monospace());
            }));
    }
}
