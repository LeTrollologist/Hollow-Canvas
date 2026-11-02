use egui::{Color32, Rounding, Stroke, Visuals};
use aurelia_core::color::Color;

pub fn configure_aurelia_style(ctx: &egui::Context, accent: Color) {
    let mut style = (*ctx.style()).clone();
    let [r, g, b, _] = accent.to_rgba8();
    let accent_c32 = Color32::from_rgb(r, g, b);
    let accent_dim_c32 = Color32::from_rgba_unmultiplied(r, g, b, 50);
    let accent_glow_c32 = Color32::from_rgba_unmultiplied(r, g, b, 180);

    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(Color32::from_rgb(243, 231, 214));
    visuals.panel_fill = Color32::from_rgba_unmultiplied(16, 12, 9, 248);
    visuals.window_fill = Color32::from_rgba_unmultiplied(21, 16, 12, 252);
    visuals.faint_bg_color = Color32::from_rgba_unmultiplied(30, 23, 18, 200);
    visuals.extreme_bg_color = Color32::from_rgba_unmultiplied(9, 7, 5, 255);

    // Sleek studio border strokes (warm ink hairline)
    let border_subtle = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(150, 112, 74, 55));
    let border_hover = Stroke::new(1.0_f32, accent_glow_c32);
    let border_active = Stroke::new(1.5_f32, accent_c32);

    visuals.widgets.noninteractive.bg_fill = Color32::from_rgba_unmultiplied(26, 20, 15, 180);
    visuals.widgets.noninteractive.bg_stroke = border_subtle;
    visuals.widgets.noninteractive.rounding = Rounding::same(4.0);

    // Inactive buttons: warm charcoal
    visuals.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(35, 27, 20, 160);
    visuals.widgets.inactive.bg_stroke = border_subtle;
    visuals.widgets.inactive.rounding = Rounding::same(4.0);

    // Hovered buttons: lit parchment glow
    visuals.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(58, 45, 29, 220);
    visuals.widgets.hovered.bg_stroke = border_hover;
    visuals.widgets.hovered.rounding = Rounding::same(4.0);

    // Active buttons: illuminated with accent glow
    visuals.widgets.active.bg_fill = accent_dim_c32;
    visuals.widgets.active.bg_stroke = border_active;
    visuals.widgets.active.rounding = Rounding::same(4.0);

    // Open dropdown menus
    visuals.widgets.open.bg_fill = Color32::from_rgba_unmultiplied(30, 23, 17, 250);
    visuals.widgets.open.bg_stroke = border_active;
    visuals.widgets.open.rounding = Rounding::same(4.0);

    visuals.selection.bg_fill = accent_dim_c32;
    visuals.selection.stroke = Stroke::new(1.0_f32, accent_c32);

    visuals.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur: 8.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 120),
    };
    visuals.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 12.0,
        spread: 0.0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 160),
    };

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.menu_margin = egui::Margin::same(6.0);
    style.spacing.slider_width = 110.0;

    ctx.set_style(style);
}
