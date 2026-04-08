//! SIEM-Lite Operator — нативное окно (egui): разделы, кейсы, быстрый доступ к стеку.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Section {
    #[default]
    Cases,
    Stack,
    Connection,
}

#[derive(Debug, Clone, Deserialize)]
struct CasesResponse {
    cases: Vec<CaseBrief>,
    total: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct CaseBrief {
    id: String,
    display_key: String,
    title: String,
    severity: String,
    status: String,
    #[serde(default)]
    assignee: Option<String>,
    created_at: String,
}

struct OperatorApp {
    api_base: String,
    section: Section,
    cases: Vec<CaseBrief>,
    total: i64,
    status: String,
    loading: bool,
    rx: Option<Receiver<Result<CasesResponse, String>>>,
    selected: Option<usize>,
}

impl Default for OperatorApp {
    fn default() -> Self {
        let api_base = std::env::var("SIEM_OPERATOR_API")
            .unwrap_or_else(|_| "http://127.0.0.1:8088".to_string());
        Self {
            api_base,
            section: Section::default(),
            cases: Vec::new(),
            total: 0,
            status: "Нажми «Обновить» или дождись авто-загрузки.".to_string(),
            loading: false,
            rx: None,
            selected: None,
        }
    }
}

fn setup_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(16, 20, 26);
    visuals.window_fill = egui::Color32::from_rgb(22, 27, 36);
    visuals.extreme_bg_color = egui::Color32::from_rgb(12, 15, 20);
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(30, 36, 48);
    visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(26, 32, 42);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(38, 46, 60);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(52, 64, 86);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(64, 88, 118);
    visuals.widgets.open.bg_fill = egui::Color32::from_rgb(48, 60, 80);
    visuals.selection.bg_fill = egui::Color32::from_rgb(36, 112, 160);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 180, 240));
    visuals.hyperlink_color = egui::Color32::from_rgb(120, 190, 255);
    visuals.warn_fg_color = egui::Color32::from_rgb(255, 200, 100);
    visuals.error_fg_color = egui::Color32::from_rgb(255, 120, 120);
    visuals.window_rounding = egui::Rounding::same(10.0);
    visuals.menu_rounding = egui::Rounding::same(6.0);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(12.0, 10.0);
    style.spacing.button_padding = egui::vec2(16.0, 9.0);
    style.spacing.window_margin = egui::Margin::same(14.0);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(22.0),
    );
    style.text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(13.0),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(13.0),
    );
    ctx.set_style(style);
}

fn severity_color(sev: &str) -> egui::Color32 {
    match sev.to_lowercase().as_str() {
        "critical" | "crit" => egui::Color32::from_rgb(235, 75, 85),
        "high" => egui::Color32::from_rgb(245, 140, 70),
        "medium" | "med" => egui::Color32::from_rgb(235, 195, 80),
        "low" => egui::Color32::from_rgb(90, 200, 140),
        "info" | "informational" => egui::Color32::from_rgb(110, 165, 235),
        _ => egui::Color32::from_rgb(150, 158, 175),
    }
}

fn pill_label(ui: &mut egui::Ui, text: impl Into<String>, color: egui::Color32) {
    let text = text.into();
    let galley = ui.fonts(|f| {
        f.layout_no_wrap(
            text.clone(),
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        )
    });
    let pad = egui::vec2(8.0, 3.0);
    let size = galley.size() + pad * 2.0;
    let (rect, _resp) = ui.allocate_at_least(size, egui::Sense::hover());
    let fill = color.gamma_multiply(0.35);
    let stroke = color.gamma_multiply(0.85);
    ui.painter().rect(
        rect,
        egui::Rounding::same(4.0),
        fill,
        egui::Stroke::new(1.0, stroke),
    );
    ui.painter().galley(
        rect.left_top() + pad,
        galley,
        egui::Color32::WHITE,
    );
}

fn section_nav_button(ui: &mut egui::Ui, label: &str, subtitle: &str, selected: bool) -> bool {
    let desired = ui.available_width();
    let mut clicked = false;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(desired, 56.0),
        egui::Sense::click(),
    );
    let visuals = ui.style().interact_selectable(&response, selected);
    let fill = if selected {
        egui::Color32::from_rgb(36, 92, 135).gamma_multiply(0.55)
    } else if response.hovered() {
        egui::Color32::from_rgb(42, 52, 70)
    } else {
        egui::Color32::from_rgb(28, 34, 46)
    };
    ui.painter().rect(
        rect,
        egui::Rounding::same(8.0),
        fill,
        if selected {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 180, 240))
        } else {
            egui::Stroke::NONE
        },
    );
    let title_color = if selected {
        egui::Color32::WHITE
    } else {
        visuals.text_color()
    };
    ui.painter().text(
        rect.left_top() + egui::vec2(12.0, 10.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(15.5),
        title_color,
    );
    ui.painter().text(
        rect.left_top() + egui::vec2(12.0, 32.0),
        egui::Align2::LEFT_TOP,
        subtitle,
        egui::FontId::proportional(11.5),
        egui::Color32::from_rgb(140, 150, 168),
    );
    if response.clicked() {
        clicked = true;
    }
    ui.add_space(6.0);
    clicked
}

fn stack_action_card(
    ui: &mut egui::Ui,
    title: &str,
    url: &str,
    description: &str,
) {
    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgb(30, 36, 48))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(45, 55, 72),
        ))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::symmetric(16.0, 14.0));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .strong()
                        .size(16.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(description)
                        .size(13.0)
                        .color(egui::Color32::from_rgb(155, 165, 185)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(url)
                        .monospace()
                        .size(12.0)
                        .color(egui::Color32::from_rgb(120, 190, 255)),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized(
                        [100.0, 36.0],
                        egui::Button::new(egui::RichText::new("Открыть").color(egui::Color32::WHITE)),
                    )
                    .clicked()
                {
                    let _ = webbrowser::open(url);
                }
            });
        });
    });
}

impl OperatorApp {
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
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(32, 40, 54),
                    )),
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
                        .add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new("Выход из приложения"),
                        )
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    ui.label(
                        egui::RichText::new("v0.2")
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

}

impl eframe::App for OperatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Ok(body)) => {
                    self.rx = None;
                    self.loading = false;
                    self.cases = body.cases;
                    self.total = body.total;
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
            .show(ctx, |ui| {
                match self.section {
                    Section::Cases => self.show_cases_panel(ui),
                    Section::Stack => self.show_stack_panel(ui),
                    Section::Connection => self.show_connection_panel(ui),
                }
            });

        if self.cases.is_empty() && !self.loading && self.rx.is_none() && self.status.contains("Нажми") {
            self.fetch_cases();
        }
    }
}

impl OperatorApp {
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
        ui.label(
            egui::RichText::new(format!(
                "В ответе: {} · Всего в базе: {}",
                self.cases.len(),
                self.total
            ))
            .size(13.0)
            .color(egui::Color32::from_rgb(150, 160, 178)),
        );
        ui.add_space(14.0);

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
                        let _ = webbrowser::open(&format!(
                            "{}/cases/{}",
                            self.api_base.trim_end_matches('/'),
                            c.id
                        ));
                    }
                }
            }
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(
                        egui::RichText::new("Рабочий стол расследования")
                            .color(egui::Color32::WHITE),
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
                        ui.end_row();
                        for (i, c) in self.cases.iter().enumerate() {
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
                            ui.end_row();
                        }
                    });
            });
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
                ui.label(egui::RichText::new("URL API").small().color(egui::Color32::from_rgb(140, 150, 168)));
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
                            egui::Button::new(egui::RichText::new("Сохранить и обновить").color(egui::Color32::WHITE)),
                        )
                        .clicked()
                    {
                        self.trim_api_base();
                        self.fetch_cases();
                        self.status = "URL обновлён, загрузка кейсов…".to_string();
                    }
                    if ui.button("Сброс на env / localhost").clicked() {
                        self.api_base = std::env::var("SIEM_OPERATOR_API")
                            .unwrap_or_else(|_| "http://127.0.0.1:8088".to_string());
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
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SIEM-Lite Operator")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SIEM-Lite Operator",
        options,
        Box::new(|cc| {
            setup_theme(&cc.egui_ctx);
            Ok(Box::new(OperatorApp::default()) as Box<dyn eframe::App>)
        }),
    )
}
