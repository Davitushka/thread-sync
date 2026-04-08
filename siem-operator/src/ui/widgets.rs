use eframe::egui;

pub fn severity_color(sev: &str) -> egui::Color32 {
    match sev.to_lowercase().as_str() {
        "critical" | "crit" => egui::Color32::from_rgb(235, 75, 85),
        "high" => egui::Color32::from_rgb(245, 140, 70),
        "medium" | "med" => egui::Color32::from_rgb(235, 195, 80),
        "low" => egui::Color32::from_rgb(90, 200, 140),
        "info" | "informational" => egui::Color32::from_rgb(110, 165, 235),
        _ => egui::Color32::from_rgb(150, 158, 175),
    }
}

pub fn pill_label(ui: &mut egui::Ui, text: impl Into<String>, color: egui::Color32) {
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
    ui.painter()
        .galley(rect.left_top() + pad, galley, egui::Color32::WHITE);
}

pub fn section_nav_button(ui: &mut egui::Ui, label: &str, subtitle: &str, selected: bool) -> bool {
    let desired = ui.available_width();
    let mut clicked = false;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(desired, 56.0), egui::Sense::click());
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

pub fn stack_action_card(ui: &mut egui::Ui, title: &str, url: &str, description: &str) {
    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgb(30, 36, 48))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 55, 72)))
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
