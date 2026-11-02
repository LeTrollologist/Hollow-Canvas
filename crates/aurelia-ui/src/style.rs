use aurelia_core::color::Color;
use egui::{Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Visuals};

use crate::design;

/// Applies the Aurelia chrome: warm-ink surfaces, a single type scale and the
/// spacing that keeps every panel's controls inside its own bounds.
pub fn configure_aurelia_style(ctx: &egui::Context, accent: Color) {
    let mut style = (*ctx.style()).clone();
    let [r, g, b, _] = accent.to_rgba8();
    let accent_c32 = Color32::from_rgb(r, g, b);
    let accent_dim_c32 = Color32::from_rgba_unmultiplied(r, g, b, 60);
    let accent_glow_c32 = Color32::from_rgba_unmultiplied(r, g, b, 190);

    // ── Type scale ──  one ladder for the whole app (no more 8.5 px labels).
    style.text_styles = [
        (TextStyle::Heading, FontId::new(15.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(12.5, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(12.5, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(11.5, FontFamily::Monospace)),
    ]
    .into();

    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(design::TEXT);
    visuals.panel_fill = design::INK_PANEL;
    visuals.window_fill = design::INK_PANEL;
    visuals.faint_bg_color = design::INK_RAISED;
    visuals.extreme_bg_color = design::INK_DEEP;

    let border_subtle = Stroke::new(1.0_f32, design::HAIRLINE);
    let border_hover = Stroke::new(1.0_f32, accent_glow_c32);
    let border_active = Stroke::new(1.5_f32, accent_c32);

    visuals.widgets.noninteractive.bg_fill = design::INK_PANEL;
    visuals.widgets.noninteractive.bg_stroke = border_subtle;
    visuals.widgets.noninteractive.rounding = Rounding::same(5.0);

    visuals.widgets.inactive.bg_fill = design::INK_RAISED;
    visuals.widgets.inactive.bg_stroke = border_subtle;
    visuals.widgets.inactive.rounding = Rounding::same(5.0);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 41, 28);
    visuals.widgets.hovered.bg_stroke = border_hover;
    visuals.widgets.hovered.rounding = Rounding::same(5.0);

    visuals.widgets.active.bg_fill = accent_dim_c32;
    visuals.widgets.active.bg_stroke = border_active;
    visuals.widgets.active.rounding = Rounding::same(5.0);

    visuals.widgets.open.bg_fill = design::INK_PANEL;
    visuals.widgets.open.bg_stroke = border_active;
    visuals.widgets.open.rounding = Rounding::same(5.0);

    visuals.selection.bg_fill = accent_dim_c32;
    visuals.selection.stroke = Stroke::new(1.0_f32, accent_c32);
    visuals.window_stroke = border_subtle;
    visuals.window_rounding = Rounding::same(8.0);
    visuals.menu_rounding = Rounding::same(7.0);

    visuals.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 14.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 140),
    };
    visuals.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 8.0),
        blur: 18.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 170),
    };

    style.visuals = visuals;

    // ── Spacing ── generous enough that captions and controls stop colliding.
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size = egui::vec2(26.0, 22.0);
    style.spacing.slider_width = 96.0;
    style.spacing.indent = 14.0;
    style.spacing.window_margin = egui::Margin::same(10.0);
    style.spacing.menu_margin = egui::Margin::same(8.0);
    style.spacing.scroll.bar_width = 9.0;
    style.spacing.scroll.floating = false;

    ctx.set_style(style);
}

