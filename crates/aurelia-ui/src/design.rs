//! Design tokens and layout helpers for the studio chrome.
//!
//! Keeping colours, spacing and the two common row shapes here is what stops the
//! panels from drifting apart (and from clipping their own controls) as features
//! are added.

use egui::{Color32, RichText, Stroke, Ui};

// ── Palette: warm ink + golden hour ─────────────────────────────────────
pub const INK_DEEP: Color32 = Color32::from_rgb(11, 8, 6);
pub const INK_SUNKEN: Color32 = Color32::from_rgb(15, 12, 9);
pub const INK_PANEL: Color32 = Color32::from_rgb(23, 19, 14);
pub const INK_RAISED: Color32 = Color32::from_rgb(33, 26, 19);
pub const HAIRLINE: Color32 = Color32::from_rgb(58, 45, 35);

pub const TEXT: Color32 = Color32::from_rgb(243, 231, 214);
pub const TEXT_DIM: Color32 = Color32::from_rgb(198, 178, 154);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(150, 131, 110);

pub const GOLD: Color32 = Color32::from_rgb(246, 196, 90);
pub const AMBER: Color32 = Color32::from_rgb(232, 150, 63);
pub const CORAL: Color32 = Color32::from_rgb(228, 112, 95);
pub const TEAL: Color32 = Color32::from_rgb(63, 167, 150);

// ── Metrics ─────────────────────────────────────────────────────────────
pub const PANEL_PAD: f32 = 10.0;
pub const GAP: f32 = 6.0;
pub const ROW_H: f32 = 20.0;
/// Width reserved for the caption column of a property row.
pub const LABEL_W: f32 = 92.0;
pub const SWATCH: f32 = 22.0;

pub fn panel_frame(fill: Color32) -> egui::Frame {
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, HAIRLINE))
        .inner_margin(PANEL_PAD)
}

pub fn bar_frame(fill: Color32) -> egui::Frame {
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, HAIRLINE))
        .inner_margin(egui::Margin::symmetric(PANEL_PAD, 5.0))
}

/// Section caption with a hairline rule that fills the remaining width.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(10.0).strong().color(TEXT_MUTED));
        let remaining = ui.available_width().max(0.0);
        if remaining > 8.0 {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(remaining, 1.0), egui::Sense::hover());
            ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, HAIRLINE));
        }
    });
    ui.add_space(3.0);
}

/// Caption on the left, control on the right; the caption truncates instead of
/// pushing the control past the panel edge.
pub fn row(ui: &mut Ui, caption: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_W, ROW_H],
            egui::Label::new(RichText::new(caption).size(11.0).color(TEXT_DIM)).truncate(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            add(ui);
        });
    });
}

/// Slider plus a compact numeric field, sized to fit inside a 260 px panel.
pub fn slider_row(
    ui: &mut Ui,
    caption: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    speed: f32,
    decimals: usize,
    suffix: &str,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_W, ROW_H],
            egui::Label::new(RichText::new(caption).size(11.0).color(TEXT_DIM)).truncate(),
        );
        let field_w = 54.0;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            changed |= ui
                .add_sized(
                    [field_w, ROW_H],
                    egui::DragValue::new(value)
                        .speed(speed)
                        .max_decimals(decimals)
                        .suffix(suffix),
                )
                .changed();
            let slider_w = (ui.available_width() - 4.0).clamp(40.0, 120.0);
            changed |= ui
                .add_sized(
                    [slider_w, ROW_H],
                    egui::Slider::new(value, range).show_value(false),
                )
                .changed();
        });
    });
    changed
}

/// Equal-width button, used for the tool shelf, presets and canvas ops grids.
pub fn grid_button(ui: &mut Ui, text: &str, active: bool, accent: Color32, width: f32) -> bool {
    let (fill, stroke, colour) = if active {
        (accent.linear_multiply(0.30), Stroke::new(1.0, accent), TEXT)
    } else {
        (INK_RAISED, Stroke::new(1.0, HAIRLINE), TEXT_DIM)
    };
    ui.add(
        egui::Button::new(RichText::new(text).size(11.5).color(colour))
            .fill(fill)
            .stroke(stroke)
            .rounding(egui::Rounding::same(5.0))
            .min_size(egui::vec2(width, 24.0)),
    )
    .clicked()
}
