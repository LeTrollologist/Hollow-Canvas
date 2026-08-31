use egui::{Color32, Rounding, Stroke, Visuals};
use hollow_core::color::Color;

pub fn configure_hollow_style(ctx: &egui::Context, accent: Color) {
    let mut style = (*ctx.style()).clone();
    let [r, g, b, _] = accent.to_rgba8();
    let accent_c32 = Color32::from_rgb(r, g, b);
    let accent_dim_c32 = Color32::from_rgba_unmultiplied(r, g, b, 45);

    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(Color32::from_rgb(215, 222, 245));
    visuals.panel_fill = Color32::from_rgba_unmultiplied(8, 12, 24, 245);
    visuals.window_fill = Color32::from_rgba_unmultiplied(10, 14, 28, 250);
    visuals.faint_bg_color = Color32::from_rgba_unmultiplied(14, 18, 34, 220);
    visuals.extreme_bg_color = Color32::from_rgba_unmultiplied(4, 6, 14, 255);

    // Subtle crisp border styling
    let border_stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(140, 155, 200, 40));

    visuals.widgets.noninteractive.bg_fill = Color32::from_rgba_unmultiplied(14, 18, 34, 180);
    visuals.widgets.noninteractive.bg_stroke = border_stroke;
    visuals.widgets.noninteractive.rounding = Rounding::same(3.0);

    visuals.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(22, 28, 50, 140);
    visuals.widgets.inactive.bg_stroke = border_stroke;
    visuals.widgets.inactive.rounding = Rounding::same(3.0);

    visuals.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(35, 45, 75, 220);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent_c32);
    visuals.widgets.hovered.rounding = Rounding::same(3.0);

    visuals.widgets.active.bg_fill = accent_dim_c32;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, accent_c32);
    visuals.widgets.active.rounding = Rounding::same(3.0);

    visuals.widgets.open.bg_fill = Color32::from_rgba_unmultiplied(16, 22, 40, 240);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, accent_c32);
    visuals.widgets.open.rounding = Rounding::same(3.0);

    visuals.selection.bg_fill = accent_dim_c32;
    visuals.selection.stroke = Stroke::new(1.0_f32, accent_c32);

    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals.window_shadow = egui::epaint::Shadow::NONE;

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.menu_margin = egui::Margin::same(6.0);

    ctx.set_style(style);
}
