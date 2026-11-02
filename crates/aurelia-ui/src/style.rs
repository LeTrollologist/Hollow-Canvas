use aurelia_core::color::Color;
use egui::{Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Visuals};

use crate::design;
use crate::fonts;

/// Applies the Aurelia chrome: warm-ink surfaces, a single type scale, damped
/// animations and the gloss primitives in [`crate::design`].
pub fn configure_aurelia_style(ctx: &egui::Context, accent: Color) {
    // Typography first: everything below assumes Segoe UI is available (with
    // egui's bundled faces kept as fallbacks).
    fonts::install(ctx);

    let mut style = (*ctx.style()).clone();
    let [r, g, b, _] = accent.to_rgba8();
    let accent_c32 = Color32::from_rgb(r, g, b);
    let accent_dim_c32 = Color32::from_rgba_unmultiplied(r, g, b, 64);
    let accent_glow_c32 = Color32::from_rgba_unmultiplied(r, g, b, 190);
    let accent_soft = Color32::from_rgba_unmultiplied(r, g, b, 120);

    // ── Type scale ── one ladder for the whole app.
    style.text_styles = [
        (TextStyle::Heading, FontId::new(15.5, fonts::display())),
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
    visuals.hyperlink_color = design::GOLD;

    let hairline = Stroke::new(1.0_f32, design::HAIRLINE);
    let hairline_lit = Stroke::new(1.0_f32, accent_soft);
    let active_ring = Stroke::new(1.0_f32, accent_c32);

    // Rails and scroll handles read from `bg_fill`; surfaces from `weak_bg_fill`.
    visuals.widgets.noninteractive.bg_fill = design::INK_PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = design::INK_PANEL;
    visuals.widgets.noninteractive.bg_stroke = hairline;
    visuals.widgets.noninteractive.rounding = Rounding::same(design::RADIUS);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(59, 47, 35);
    visuals.widgets.inactive.weak_bg_fill = design::INK_RAISED;
    visuals.widgets.inactive.bg_stroke = hairline;
    visuals.widgets.inactive.rounding = Rounding::same(design::RADIUS);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, design::TEXT_DIM);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(74, 58, 42);
    visuals.widgets.hovered.weak_bg_fill = design::INK_RAISED_TOP;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent_glow_c32);
    visuals.widgets.hovered.rounding = Rounding::same(design::RADIUS);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, design::TEXT);
    visuals.widgets.hovered.expansion = 0.5;

    visuals.widgets.active.bg_fill = accent_dim_c32;
    visuals.widgets.active.weak_bg_fill = accent_dim_c32;
    visuals.widgets.active.bg_stroke = active_ring;
    visuals.widgets.active.rounding = Rounding::same(design::RADIUS);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, accent_c32);

    visuals.widgets.open.bg_fill = design::INK_PANEL;
    visuals.widgets.open.weak_bg_fill = design::INK_RAISED;
    visuals.widgets.open.bg_stroke = active_ring;
    visuals.widgets.open.rounding = Rounding::same(design::RADIUS);

    visuals.selection.bg_fill = accent_dim_c32;
    visuals.selection.stroke = Stroke::new(1.0_f32, accent_c32);
    visuals.window_stroke = hairline_lit;
    visuals.window_rounding = Rounding::same(10.0);
    visuals.menu_rounding = Rounding::same(8.0);

    visuals.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 8.0),
        blur: 20.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 170),
    };
    visuals.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 10.0),
        blur: 24.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 190),
    };

    style.visuals = visuals;

    // ── Spacing ── generous enough that captions and controls stop colliding.
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(11.0, 5.0);
    style.spacing.interact_size = egui::vec2(26.0, 22.0);
    style.spacing.slider_width = 96.0;
    style.spacing.indent = 14.0;
    style.spacing.window_margin = egui::Margin::same(11.0);
    style.spacing.menu_margin = egui::Margin::same(8.0);
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.bar_inner_margin = 2.0;
    // Floating bars fade in over the content instead of carving a gutter.
    style.spacing.scroll.floating = true;

    // Damped motion: hover/press settle in ~120 ms instead of snapping.
    style.animation_time = 0.12;

    ctx.set_style(style);
}

