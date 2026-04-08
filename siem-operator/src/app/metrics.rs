use eframe::egui;

pub(super) fn average_hours(values: impl Iterator<Item = i64>) -> i64 {
    let v: Vec<i64> = values.collect();
    if v.is_empty() {
        return 0;
    }
    v.iter().sum::<i64>() / i64::try_from(v.len()).unwrap_or(1)
}

pub(super) fn kpi_card(ui: &mut egui::Ui, label: &str, value: &str, accent: egui::Color32) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(24, 30, 42))
        .rounding(egui::Rounding::same(12.0))
        .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.85)))
        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
        .show(ui, |ui| {
            ui.set_min_width(150.0);
            ui.label(
                egui::RichText::new(label)
                    .small()
                    .color(egui::Color32::from_rgb(165, 178, 198)),
            );
            ui.label(egui::RichText::new(value).strong().size(24.0).color(accent));
        });
}

pub(super) fn sparkline_card(ui: &mut egui::Ui, title: &str, values: &[f32], color: egui::Color32) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(24, 30, 42))
        .rounding(egui::Rounding::same(12.0))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.7)))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_min_width(250.0);
            ui.label(egui::RichText::new(title).small());
            let desired = egui::vec2(240.0, 52.0);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            if values.len() < 2 {
                return;
            }
            let max = values.iter().copied().fold(0.0_f32, f32::max).max(1.0);
            let mut points: Vec<egui::Pos2> = Vec::with_capacity(values.len());
            for (i, v) in values.iter().enumerate() {
                let x = rect.left() + (i as f32 / (values.len() - 1) as f32) * rect.width();
                let y = rect.bottom() - (v / max) * rect.height();
                points.push(egui::pos2(x, y));
            }
            ui.painter().line_segment(
                [
                    egui::pos2(rect.left(), rect.bottom()),
                    egui::pos2(rect.right(), rect.bottom()),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
            );
            ui.painter()
                .add(egui::Shape::line(points, egui::Stroke::new(2.0, color)));
        });
}
