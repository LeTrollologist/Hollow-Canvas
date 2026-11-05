//! Design tokens and the surface / gloss primitives the chrome is built from.
//!
//! Keeping colours, spacing and the shared painting helpers here is what stops the
//! panels from drifting apart (and from clipping their own controls) as features
//! are added. The look aims for "warm ink and lamplight": deep neutral-brown
//! surfaces, a single gold accent, hairline edges and a restrained top sheen that
//! reads as gloss without noise.

use egui::{
    Color32, FontId, Mesh, Painter, Rect, Response, RichText, Rounding, Sense, Stroke, Ui,
};

use crate::fonts;

// ── Palette: warm ink + golden hour ─────────────────────────────────────
pub const INK_BASE: Color32 = Color32::from_rgb(18, 14, 10);
pub const INK_DEEP: Color32 = Color32::from_rgb(14, 11, 8);
pub const INK_SUNKEN: Color32 = Color32::from_rgb(23, 18, 13);
pub const INK_PANEL: Color32 = Color32::from_rgb(27, 22, 16);
/// Panels are painted as a very shallow vertical gradient between these two.
pub const INK_PANEL_TOP: Color32 = Color32::from_rgb(36, 29, 21);
pub const INK_PANEL_BOTTOM: Color32 = Color32::from_rgb(23, 18, 13);
pub const INK_RAISED: Color32 = Color32::from_rgb(42, 33, 23);
pub const INK_RAISED_TOP: Color32 = Color32::from_rgb(58, 46, 32);

pub const HAIRLINE: Color32 = Color32::from_rgb(58, 46, 36);
/// Low-contrast rule for section captions (fades out to the right).
pub const HAIRLINE_SOFT: Color32 = Color32::from_rgb(96, 76, 56);
/// 1 px highlight along the top inside edge of a raised surface.
pub const INNER_GLOW: Color32 = Color32::from_rgba_premultiplied(255, 236, 198, 22);

pub const TEXT: Color32 = Color32::from_rgb(243, 233, 216);
pub const TEXT_DIM: Color32 = Color32::from_rgb(200, 181, 158);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(168, 148, 124);
/// Warm champagne used for section captions; reads as gold without shouting.
pub const CAPTION: Color32 = Color32::from_rgb(188, 162, 124);

pub const GOLD: Color32 = Color32::from_rgb(217, 164, 65);
pub const GOLD_BRIGHT: Color32 = Color32::from_rgb(240, 200, 120);
pub const GOLD_DEEP: Color32 = Color32::from_rgb(140, 94, 36);
pub const AMBER: Color32 = Color32::from_rgb(210, 142, 67);
pub const CORAL: Color32 = Color32::from_rgb(228, 112, 95);
pub const TEAL: Color32 = Color32::from_rgb(95, 179, 163);

// ── Metrics ─────────────────────────────────────────────────────────────
pub const PANEL_PAD: f32 = 10.0;
pub const GAP: f32 = 6.0;
pub const ROW_H: f32 = 20.0;
/// Width reserved for the caption column of a property row.
pub const LABEL_W: f32 = 92.0;
pub const SWATCH: f32 = 22.0;
/// Corner radius for raised controls.
pub const RADIUS: f32 = 7.0;

/// Width/height of the ruler strips when they are visible.
pub const RULER_PX: f32 = 24.0;

/// Straight linear blend between two colours (alpha follows `a`).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

/// Paints a vertical gradient across `rect` (two vertex colours, no banding).
pub fn vgradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// A 1 px rule that fades from `color` on the left to nothing on the right.
pub fn fade_rule(painter: &Painter, rect: Rect, color: Color32) {
    let y = rect.center().y;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(egui::pos2(rect.left(), y - 0.5), color);
    mesh.colored_vertex(egui::pos2(rect.right(), y - 0.5), Color32::TRANSPARENT);
    mesh.colored_vertex(egui::pos2(rect.right(), y + 0.5), Color32::TRANSPARENT);
    mesh.colored_vertex(egui::pos2(rect.left(), y + 0.5), color);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// Glossy raised surface: soft drop shadow, base fill, top sheen, hairline edges.
pub fn gloss(painter: &Painter, rect: Rect, rounding: Rounding, base: Color32, sheen: Color32) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let lift = Rect::from_min_max(
        rect.min + egui::vec2(0.0, 1.5),
        rect.max + egui::vec2(0.0, 2.5),
    );
    painter.rect_filled(lift, rounding, Color32::from_black_alpha(60));
    painter.rect_filled(rect, rounding, base);

    let sheen_h = (rect.height() * 0.52).max(2.0);
    let sheen_rect = Rect::from_min_max(
        rect.min,
        egui::pos2(rect.max.x, (rect.min.y + sheen_h).min(rect.max.y)),
    );
    painter.rect_filled(
        sheen_rect,
        Rounding {
            nw: rounding.nw,
            ne: rounding.ne,
            sw: 0.0,
            se: 0.0,
        },
        sheen,
    );

    let inset = rounding.nw.min(4.0);
    painter.hline(
        egui::Rangef::new(rect.left() + inset, rect.right() - inset),
        rect.top() + 0.5,
        Stroke::new(1.0, INNER_GLOW),
    );
    painter.hline(
        egui::Rangef::new(rect.left() + inset, rect.right() - inset),
        rect.bottom() - 0.5,
        Stroke::new(1.0, Color32::from_black_alpha(70)),
    );
}

/// Letterspaced caption text ("TOOLS" -> "T O O L S" with hair spaces).
pub fn spaced(text: &str) -> String {
    let upper = text.to_uppercase();
    let mut out = String::with_capacity(upper.len() * 2);
    for (i, ch) in upper.chars().enumerate() {
        if i > 0 {
            out.push('\u{2009}');
        }
        out.push(ch);
    }
    out
}

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

/// Section caption: gold tick, letterspaced semibold caps and a rule that fades out.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(7.0);
    ui.horizontal(|ui| {
        let (tick, _) = ui.allocate_exact_size(egui::vec2(2.0, 12.0), Sense::hover());
        ui.painter().rect_filled(tick, 1.0, GOLD.linear_multiply(0.85));
        ui.add_space(3.0);
        ui.label(
            RichText::new(spaced(title))
                .size(10.0)
                .family(fonts::display())
                .color(CAPTION),
        );
        let remaining = ui.available_width() - 2.0;
        if remaining > 12.0 {
            let (rule, _) = ui.allocate_exact_size(egui::vec2(remaining, 1.0), Sense::hover());
            fade_rule(ui.painter(), rule, HAIRLINE_SOFT);
        }
    });
    ui.add_space(4.0);
}

/// A small pill, used for the header's brush/version badges.
pub fn chip(ui: &mut Ui, text: RichText, accent: Color32) -> Response {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(
            accent.r(),
            accent.g(),
            accent.b(),
            20,
        ))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 110),
        ))
        .rounding(Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(9.0, 3.0))
        .show(ui, |ui| ui.label(text))
        .inner
}

/// A glossy, animated button. Used where a control deserves weight: presets,
/// canvas operations and the layer actions.
///
/// Hover and press are animated over ~120 ms so the chrome feels damped rather
/// than twitchy, and the active state keeps a permanent accent ring.
pub fn gloss_button(
    ui: &mut Ui,
    id_salt: &str,
    label: &str,
    width: f32,
    height: f32,
    active: bool,
    accent: Color32,
) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), Sense::click_and_drag());
    if !ui.is_rect_visible(rect) {
        return response;
    }

    let id = ui.id().with(id_salt);
    let hovered = ui
        .ctx()
        .animate_bool_with_time(id.with("hover"), response.hovered(), 0.12);
    let pressed = ui
        .ctx()
        .animate_bool_with_time(id.with("press"), response.is_pointer_button_down_on(), 0.05);

    // Idle -> hover lightens the fill; pressed sinks it back slightly.
    let base = if active {
        mix(INK_RAISED_TOP, accent.linear_multiply(0.30), 0.55)
    } else {
        mix(
            INK_RAISED,
            INK_RAISED_TOP,
            (0.30 + 0.70 * hovered) * (1.0 - 0.55 * pressed),
        )
    };
    let sheen = if active {
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 26)
    } else {
        Color32::from_white_alpha(if hovered > 0.5 { 12 } else { 7 })
    };
    let rounding = Rounding::same(RADIUS);
    let painter = ui.painter();
    gloss(painter, rect, rounding, base, sheen);

    let ring = if active {
        0.85
    } else {
        0.55 * hovered
    };
    if ring > 0.01 {
        painter.rect_stroke(
            rect,
            rounding,
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(
                    accent.r(),
                    accent.g(),
                    accent.b(),
                    (255.0 * ring) as u8,
                ),
            ),
        );
    } else {
        painter.rect_stroke(rect, rounding, Stroke::new(1.0, HAIRLINE));
    }

    let text_color = if active { accent } else { mix(TEXT_DIM, TEXT, hovered) };
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.5),
        text_color,
    );

    response
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
