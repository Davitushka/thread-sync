use eframe::egui;

use crate::models::CaseBrief;

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
                action("alerts", "Go: Alerts", &mut |s| s.section = Section::Alerts);
                action("events", "Go: Events", &mut |s| s.section = Section::Events);
                action("assets", "Go: Assets", &mut |s| s.section = Section::Assets);
                action("refresh", "Action: Refresh cases", &mut |s| s.fetch_cases());
                action("refresh events", "Action: Refresh events", &mut |s| s.fetch_events());
                action("refresh assets", "Action: Refresh assets", &mut |s| s.fetch_assets());
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
}
