use eframe::egui;

use crate::theme::{ThemePalette, u8_radius};

pub fn severity_color(sev: &str) -> egui::Color32 {
    match sev.to_lowercase().as_str() {
        "critical" | "crit" => egui::Color32::from_rgb(248, 86, 96),
        "high" => egui::Color32::from_rgb(245, 150, 72),
        "medium" | "med" => egui::Color32::from_rgb(232, 196, 88),
        "low" => egui::Color32::from_rgb(72, 198, 150),
        "info" | "informational" => egui::Color32::from_rgb(64, 210, 188),
        _ => egui::Color32::from_rgb(132, 140, 154),
    }
}

fn pill_text_on_accent(accent: egui::Color32) -> egui::Color32 {
    let luma = accent.r() as u32 * 299 + accent.g() as u32 * 587 + accent.b() as u32 * 114;
    if luma > 150_000 {
        egui::Color32::from_rgb(22, 24, 28)
    } else {
        egui::Color32::WHITE
    }
}

pub fn pill_label(ui: &mut egui::Ui, text: impl Into<String>, color: egui::Color32) {
    let text = text.into();
    let fg = pill_text_on_accent(color);
    let galley = ui
        .painter()
        .layout_no_wrap(text.clone(), egui::FontId::proportional(12.0), fg);
    let pad = egui::vec2(8.0, 3.0);
    let size = galley.size() + pad * 2.0;
    let (rect, _resp) = ui.allocate_at_least(size, egui::Sense::hover());
    let fill = color.gamma_multiply(0.35);
    let stroke = color.gamma_multiply(0.85);
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        fill,
        egui::Stroke::new(1.0, stroke),
        egui::StrokeKind::Middle,
    );
    ui.painter().galley(rect.left_top() + pad, galley, fg);
}

pub fn section_nav_button(
    ui: &mut egui::Ui,
    p: &ThemePalette,
    label: &str,
    subtitle: &str,
    selected: bool,
) -> bool {
    let desired = ui.available_width();
    let mut clicked = false;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(desired, 56.0), egui::Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    let fill = if selected {
        p.nav_selected_fill
    } else if response.hovered() {
        p.nav_item_hover
    } else {
        p.nav_item_base
    };
    let r = p.radius_nav;
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(u8_radius(r)),
        fill,
        if selected {
            egui::Stroke::new(1.0, p.nav_selected_stroke.gamma_multiply(0.45))
        } else {
            egui::Stroke::NONE
        },
        egui::StrokeKind::Middle,
    );
    if selected {
        let inset = 10.0;
        let bar = egui::Rect::from_min_max(
            rect.left_top() + egui::vec2(5.0, inset),
            rect.left_bottom() + egui::vec2(8.0, -inset),
        );
        ui.painter()
            .rect_filled(bar, egui::CornerRadius::same(2), p.nav_selected_stroke);
    }
    let title_color = if selected {
        p.text_on_sidebar
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
        p.text_muted,
    );
    if response.clicked() {
        clicked = true;
    }
    ui.add_space(6.0);
    clicked
}

pub fn stack_action_card(
    ui: &mut egui::Ui,
    p: &ThemePalette,
    title: &str,
    url: &str,
    description: &str,
) {
    let frame = egui::Frame::new()
        .fill(p.card_fill)
        .stroke(egui::Stroke::new(1.0, p.card_stroke))
        .corner_radius(egui::CornerRadius::same(u8_radius(p.radius_card)))
        .inner_margin(egui::Margin::symmetric(18, 15));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .strong()
                        .size(16.0)
                        .color(p.text_on_sidebar),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(description)
                        .size(13.0)
                        .color(p.text_muted),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(url)
                        .monospace()
                        .size(12.0)
                        .color(p.link),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized(
                        [100.0, 36.0],
                        egui::Button::new(egui::RichText::new("Открыть").color(p.text_on_sidebar)),
                    )
                    .clicked()
                {
                    let _ = webbrowser::open(url);
                }
            });
        });
    });
}
