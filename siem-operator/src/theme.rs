use eframe::egui;

pub fn setup_theme(ctx: &egui::Context) {
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
    style
        .text_styles
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
