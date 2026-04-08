//! SIEM-Lite Operator — нативное окно (egui): разделы, кейсы, быстрый доступ к стеку.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod models;
mod theme;
mod ui;

use app::OperatorApp;
use eframe::egui;
use theme::setup_theme;

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
