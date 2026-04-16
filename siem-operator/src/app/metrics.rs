use eframe::egui;

use crate::theme::{ThemePalette, u8_radius};

pub(super) fn average_hours(values: impl Iterator<Item = i64>) -> i64 {
    let (count, sum) = values.fold((0i64, 0i64), |(c, s), v| (c + 1, s + v));
    if count == 0 {
        return 0;
    }
    sum / count
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
            ui.label(egui::RichText::new(label).small().color(p.card_label));
            ui.label(egui::RichText::new(value).strong().size(24.0).color(accent));
        });
}

/// Lightweight sparkline using direct painter — same visual result as egui_plot::Plot
/// but 10-100x faster because it skips axes, zoom, legends, hit-testing, GPU buffers.
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
            ui.label(egui::RichText::new(title).small().color(p.card_label));
            if values.len() < 2 {
                return;
            }

            let height = 60.0;
            let (rect, _response) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());

            let max_val = values.iter().copied().fold(0.0f32, f32::max).max(1.0);
            let x_step = rect.width() / (values.len() - 1) as f32;
            let painter = ui.painter();

            // Fill area under the curve
            let fill_color = color.gamma_multiply(0.15);
            let mut fill_points = vec![egui::Pos2::new(rect.left(), rect.bottom())];
            for (i, &v) in values.iter().enumerate() {
                let x = rect.left() + i as f32 * x_step;
                let y = rect.bottom() - (v / max_val) * rect.height() * 0.9;
                fill_points.push(egui::Pos2::new(x, y));
            }
            fill_points.push(egui::Pos2::new(
                rect.left() + (values.len() - 1) as f32 * x_step,
                rect.bottom(),
            ));
            painter.add(egui::Shape::convex_polygon(
                fill_points,
                fill_color,
                egui::Stroke::NONE,
            ));

            // Draw the line on top
            let line_points: Vec<egui::Pos2> = values
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    let x = rect.left() + i as f32 * x_step;
                    let y = rect.bottom() - (v / max_val) * rect.height() * 0.9;
                    egui::Pos2::new(x, y)
                })
                .collect();
            painter.add(egui::Shape::line(
                line_points,
                egui::Stroke::new(2.0, color),
            ));
        });
}
