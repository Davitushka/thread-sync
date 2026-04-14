use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};

use crate::theme::{u8_radius, ThemePalette};

pub(super) fn average_hours(values: impl Iterator<Item = i64>) -> i64 {
    let v: Vec<i64> = values.collect();
    if v.is_empty() {
        return 0;
    }
    v.iter().sum::<i64>() / i64::try_from(v.len()).unwrap_or(1)
}

pub(super) fn kpi_card(
    ui: &mut egui::Ui,
    p: &ThemePalette,
    label: &str,
    value: &str,
    accent: egui::Color32,
) {
    egui::Frame::new()
        .fill(p.card_fill)
        .corner_radius(egui::CornerRadius::same(u8_radius(p.radius_card)))
        .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.75)))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_min_width(150.0);
            ui.label(
                egui::RichText::new(label)
                    .small()
                    .color(p.card_label),
            );
            ui.label(egui::RichText::new(value).strong().size(24.0).color(accent));
        });
}

pub(super) fn sparkline_card(
    ui: &mut egui::Ui,
    p: &ThemePalette,
    title: &str,
    values: &[f32],
    color: egui::Color32,
) {
    egui::Frame::new()
        .fill(p.card_fill)
        .corner_radius(egui::CornerRadius::same(u8_radius(p.radius_card)))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.65)))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .small()
                    .color(p.card_label),
            );
            if values.len() < 2 {
                return;
            }
            let points: PlotPoints = values
                .iter()
                .enumerate()
                .map(|(i, v)| [i as f64, *v as f64])
                .collect();
            Plot::new(format!("plot_{title}"))
                .height(70.0)
                .allow_zoom(false)
                .allow_drag(false)
                .allow_scroll(false)
                .show_axes([false, false])
                .show(ui, |plot_ui| {
                    plot_ui.line(Line::new("", points).color(color));
                });
        });
}
