use egui::{Color32, Rounding, Stroke, Visuals};
use hollow_core::color::Color;

pub fn configure_hollow_style(ctx: &egui::Context, accent: Color) {
    let mut style = (*ctx.style()).clone();
    let [r, g, b, _] = accent.to_rgba8();
    let accent_c32 = Color32::from_rgb(r, g, b);
    let accent_dim_c32 = Color32::from_rgba_unmultiplied(r, g, b, 45);

    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(Color32::from_rgb(205, 213, 237));
    visuals.panel_fill = Color32::from_rgba_unmultiplied(5, 8, 20, 235);
    visuals.window_fill = Color32::from_rgba_unmultiplied(5, 8, 20, 245);
    visuals.faint_bg_color = Color32::from_rgba_unmultiplied(8, 11, 26, 200);

    visuals.widgets.noninteractive.bg_fill = Color32::from_rgba_unmultiplied(12, 16, 32, 180);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(168, 159, 216, 36));
    visuals.widgets.noninteractive.rounding = Rounding::same(4.0);

    visuals.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 6);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(168, 159, 216, 36));
    visuals.widgets.inactive.rounding = Rounding::same(4.0);

    visuals.widgets.hovered.bg_fill = accent_dim_c32;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent_c32);
    visuals.widgets.hovered.rounding = Rounding::same(4.0);

    visuals.widgets.active.bg_fill = accent_dim_c32;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, accent_c32);
    visuals.widgets.active.rounding = Rounding::same(4.0);

    visuals.selection.bg_fill = accent_dim_c32;
    visuals.selection.stroke = Stroke::new(1.0_f32, accent_c32);

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 5.0);

    ctx.set_style(style);
}
