//! Semantic colors and egui `Visuals` — «noir + teal» dark, warm paper light.
use eframe::egui::{self, Color32, Context, Frame, Margin, Rounding, Stroke, Visuals};

/// Shared tokens for shell, cards, and egui widget layers.
#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub bg_canvas: Color32,
    pub bg_sidebar: Color32,
    pub stroke_sidebar: Color32,
    pub bg_top_bar: Color32,
    pub stroke_top_bar: Color32,
    pub bg_status: Color32,
    pub nav_item_base: Color32,
    pub nav_item_hover: Color32,
    pub nav_selected_fill: Color32,
    pub nav_selected_stroke: Color32,
    pub accent: Color32,
    pub text_on_sidebar: Color32,
    pub text_muted: Color32,
    pub text_subtle: Color32,
    pub text_footer: Color32,
    pub text_toolbar_secondary: Color32,
    pub text_toolbar_tertiary: Color32,
    pub card_fill: Color32,
    pub card_stroke: Color32,
    pub card_label: Color32,
    pub link: Color32,
    /// Widget / panel layers (keep in sync with `apply_theme`).
    pub weak_bg: Color32,
    pub inactive_bg: Color32,
    pub hovered_bg: Color32,
    pub active_bg: Color32,
    pub open_bg: Color32,
    pub selection_bg: Color32,
    pub radius_card: f32,
    pub radius_nav: f32,
}

impl ThemePalette {
    /// Near-black surfaces + teal accent (не «дефолтный» сине-серый egui).
    pub fn dark() -> Self {
        let accent = Color32::from_rgb(64, 230, 198);
        Self {
            bg_canvas: Color32::from_rgb(9, 10, 13),
            bg_sidebar: Color32::from_rgb(12, 13, 17),
            stroke_sidebar: Color32::from_rgb(34, 36, 44),
            bg_top_bar: Color32::from_rgb(11, 12, 16),
            stroke_top_bar: Color32::from_rgb(38, 40, 50),
            bg_status: Color32::from_rgb(8, 9, 12),
            nav_item_base: Color32::from_rgb(18, 20, 26),
            nav_item_hover: Color32::from_rgb(26, 30, 40),
            nav_selected_fill: Color32::from_rgb(22, 48, 44),
            nav_selected_stroke: accent,
            accent,
            text_on_sidebar: Color32::from_rgb(244, 245, 247),
            text_muted: Color32::from_rgb(138, 145, 158),
            text_subtle: Color32::from_rgb(98, 106, 120),
            text_footer: Color32::from_rgb(72, 78, 92),
            text_toolbar_secondary: Color32::from_rgb(152, 160, 175),
            text_toolbar_tertiary: Color32::from_rgb(178, 186, 200),
            card_fill: Color32::from_rgb(17, 18, 24),
            card_stroke: Color32::from_rgb(38, 40, 52),
            card_label: Color32::from_rgb(130, 138, 152),
            link: Color32::from_rgb(110, 235, 210),
            weak_bg: Color32::from_rgb(15, 16, 22),
            inactive_bg: Color32::from_rgb(28, 30, 40),
            hovered_bg: Color32::from_rgb(38, 42, 56),
            active_bg: Color32::from_rgb(48, 54, 72),
            open_bg: Color32::from_rgb(32, 36, 48),
            selection_bg: Color32::from_rgb(26, 58, 52),
            radius_card: 14.0,
            radius_nav: 10.0,
        }
    }

    /// Тёплый фон «бумага», тёмный teal как акцент.
    pub fn light() -> Self {
        let accent = Color32::from_rgb(13, 122, 108);
        Self {
            bg_canvas: Color32::from_rgb(246, 244, 240),
            bg_sidebar: Color32::from_rgb(255, 254, 252),
            stroke_sidebar: Color32::from_rgb(226, 222, 214),
            bg_top_bar: Color32::from_rgb(255, 254, 252),
            stroke_top_bar: Color32::from_rgb(220, 216, 208),
            bg_status: Color32::from_rgb(238, 236, 232),
            nav_item_base: Color32::from_rgb(240, 238, 234),
            nav_item_hover: Color32::from_rgb(230, 236, 234),
            nav_selected_fill: Color32::from_rgb(220, 244, 236),
            nav_selected_stroke: accent,
            accent,
            text_on_sidebar: Color32::from_rgb(28, 30, 34),
            text_muted: Color32::from_rgb(88, 92, 102),
            text_subtle: Color32::from_rgb(120, 124, 134),
            text_footer: Color32::from_rgb(130, 134, 144),
            text_toolbar_secondary: Color32::from_rgb(82, 86, 96),
            text_toolbar_tertiary: Color32::from_rgb(60, 64, 74),
            card_fill: Color32::from_rgb(255, 254, 252),
            card_stroke: Color32::from_rgb(218, 214, 206),
            card_label: Color32::from_rgb(96, 100, 110),
            link: Color32::from_rgb(18, 140, 124),
            weak_bg: Color32::from_rgb(242, 240, 236),
            inactive_bg: Color32::from_rgb(232, 230, 226),
            hovered_bg: Color32::from_rgb(220, 226, 224),
            active_bg: Color32::from_rgb(200, 218, 212),
            open_bg: Color32::from_rgb(228, 234, 232),
            selection_bg: Color32::from_rgb(200, 236, 224),
            radius_card: 14.0,
            radius_nav: 10.0,
        }
    }
}

pub fn palette(is_dark: bool) -> ThemePalette {
    if is_dark {
        ThemePalette::dark()
    } else {
        ThemePalette::light()
    }
}

pub fn sidebar_panel_frame(p: &ThemePalette) -> Frame {
    Frame::none()
        .fill(p.bg_sidebar)
        .inner_margin(Margin::same(20.0))
        .stroke(Stroke::new(1.0, p.stroke_sidebar))
}

pub fn top_bar_panel_frame(p: &ThemePalette) -> Frame {
    Frame::none()
        .fill(p.bg_top_bar)
        .stroke(Stroke::new(1.0, p.stroke_top_bar))
        .inner_margin(Margin::symmetric(16.0, 10.0))
}

pub fn status_panel_frame(p: &ThemePalette) -> Frame {
    Frame::none()
        .fill(p.bg_status)
        .inner_margin(Margin::symmetric(16.0, 7.0))
}

pub fn elevated_card_frame(p: &ThemePalette) -> Frame {
    Frame::none()
        .fill(p.card_fill)
        .rounding(Rounding::same(p.radius_card))
        .stroke(Stroke::new(1.0, p.card_stroke))
        .inner_margin(Margin::symmetric(16.0, 14.0))
}

pub fn apply_theme(ctx: &Context, is_dark: bool) {
    let p = palette(is_dark);
    let mut visuals = if is_dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.panel_fill = p.bg_sidebar;
    visuals.window_fill = p.card_fill;
    visuals.extreme_bg_color = p.bg_status;
    visuals.widgets.noninteractive.bg_fill = p.card_fill;
    visuals.widgets.noninteractive.weak_bg_fill = p.weak_bg;
    visuals.widgets.inactive.bg_fill = p.inactive_bg;
    visuals.widgets.hovered.bg_fill = p.hovered_bg;
    visuals.widgets.active.bg_fill = p.active_bg;
    visuals.widgets.open.bg_fill = p.open_bg;
    visuals.selection.bg_fill = p.selection_bg;
    visuals.selection.stroke = Stroke::new(1.0, p.nav_selected_stroke);
    visuals.hyperlink_color = p.link;
    visuals.warn_fg_color = Color32::from_rgb(245, 180, 72);
    visuals.error_fg_color = Color32::from_rgb(255, 108, 112);
    visuals.window_rounding = Rounding::same(p.radius_card);
    visuals.menu_rounding = Rounding::same(8.0);
    visuals.window_shadow = egui::Shadow {
        offset: egui::vec2(0.0, 10.0),
        blur: 22.0,
        spread: 0.0,
        color: if is_dark {
            Color32::from_black_alpha(88)
        } else {
            Color32::from_black_alpha(22)
        },
    };

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 9.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = Margin::same(16.0);
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(20.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.5));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.5));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.5));
    ctx.set_style(style);
}

pub fn setup_theme(ctx: &Context) {
    apply_theme(ctx, true);
}
