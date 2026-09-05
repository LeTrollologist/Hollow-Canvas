use crate::blend::BlendMode;
use crate::brush::{BrushPoint, BrushSettings, EraserMode, GradientType, ShapeFillMode, TextAlign, ToolType};
use crate::color::Color;
use crate::document::Document;
use crate::selection::SelectionMask;
use crate::symmetry::SymmetryConfig;
use ab_glyph::{point, Font, FontArc, PxScale, ScaleFont};
use glam::Vec2;

#[inline]
fn hash21(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = seed ^ (x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h as f32) / (u32::MAX as f32)
}

#[inline]
pub fn sample_bilinear_rgba(pixels: &[u8], width: u32, height: u32, x: f32, y: f32) -> [u8; 4] {
    if width == 0 || height == 0 || pixels.is_empty() {
        return [0, 0, 0, 0];
    }
    // Clamp coordinates before computing floor/frac to avoid edge artifacts
    let cx = x.clamp(0.0, (width as f32) - 1.0);
    let cy = y.clamp(0.0, (height as f32) - 1.0);
    let x0 = cx.floor() as u32;
    let y0 = cy.floor() as u32;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);

    let fx = cx - cx.floor();
    let fy = cy - cy.floor();

    let idx00 = ((y0 * width + x0) * 4) as usize;
    let idx10 = ((y0 * width + x1) * 4) as usize;
    let idx01 = ((y1 * width + x0) * 4) as usize;
    let idx11 = ((y1 * width + x1) * 4) as usize;

    if idx11 + 3 >= pixels.len() {
        return [0, 0, 0, 0];
    }

    let mut out = [0u8; 4];
    for c in 0..4 {
        let v00 = pixels[idx00 + c] as f32;
        let v10 = pixels[idx10 + c] as f32;
        let v01 = pixels[idx01 + c] as f32;
        let v11 = pixels[idx11 + c] as f32;

        let top = v00 * (1.0 - fx) + v10 * fx;
        let bot = v01 * (1.0 - fx) + v11 * fx;
        let val = top * (1.0 - fy) + bot * fy;
        out[c] = val.round().clamp(0.0, 255.0) as u8;
    }
    out
}

pub struct StrokeRasterizer;

impl StrokeRasterizer {
    fn blend_stamp(
        doc_w: u32,
        doc_h: u32,
        pixels: &mut [u8],
        selection: Option<&SelectionMask>,
        stamp_center: Vec2,
        prev_center: Option<Vec2>,
        radius: f32,
        brush: &BrushSettings,
        blend_mode: BlendMode,
        alpha_locked: bool,
        bg_color: Color,
        step_index: usize,
    ) {
        let tool = brush.tool;
        let color = brush.primary_color;
        let opacity = brush.opacity;
        let hardness = brush.hardness;

        let active_mask = selection.filter(|s| s.has_selection());

        if tool == ToolType::Spray {
            let density = (radius * radius * brush.spray_density * 0.4).max(6.0) as usize;
            let seed = (step_index as u32).wrapping_mul(1013904223);
            for i in 0..density {
                let r1 = hash21(i as u32, 1, seed);
                let r2 = hash21(i as u32, 2, seed);
                let angle = r1 * std::f32::consts::TAU;
                let dist = r2.sqrt() * radius;
                let px = stamp_center.x + angle.cos() * dist;
                let py = stamp_center.y + angle.sin() * dist;

                let ix = px.floor() as i32;
                let iy = py.floor() as i32;

                if ix >= 0 && ix < doc_w as i32 && iy >= 0 && iy < doc_h as i32 {
                    let uix = ix as u32;
                    let uiy = iy as u32;

                    let mut mask_factor = 1.0_f32;
                    if let Some(mask) = active_mask {
                        let mv = mask.get_value(uix, uiy);
                        if mv < 8 {
                            continue;
                        }
                        mask_factor = mv as f32 / 255.0;
                    }

                    let idx = ((uiy * doc_w + uix) * 4) as usize;
                    let dst = [pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]];
                    if alpha_locked && dst[3] == 0 {
                        continue;
                    }

                    let p_alpha = (0.25 + 0.75 * hash21(i as u32, 3, seed)) * opacity * 0.45 * mask_factor;
                    let src = color.to_rgba8();
                    let mut blended = blend_mode.composite_pixel(dst, src, p_alpha);
                    if alpha_locked {
                        blended[3] = dst[3];
                    }
                    pixels[idx..idx + 4].copy_from_slice(&blended);
                }
            }
            return;
        }

        let radius_sq = radius * radius;
        let clamped_hardness = hardness.clamp(0.0, 0.999);
        // Antialiasing fringe: guarantee at least a 1.0-subpixel smooth falloff transition at the perimeter
        let aa_fringe = 1.0_f32.min(radius * 0.5);
        let inner_r = (radius * clamped_hardness).min(radius - aa_fringe).max(0.0);
        let inner_radius_sq = inner_r * inner_r;

        let min_x = ((stamp_center.x - radius - 0.5).floor() as i32).max(0);
        let max_x = ((stamp_center.x + radius + 0.5).ceil() as i32).min(doc_w as i32 - 1);
        let min_y = ((stamp_center.y - radius - 0.5).floor() as i32).max(0);
        let max_y = ((stamp_center.y + radius + 0.5).ceil() as i32).min(doc_h as i32 - 1);

        for py_i in min_y..=max_y {
            let y = py_i as u32;
            let dy_f = (y as f32 + 0.5) - stamp_center.y;
            let dy_sq = dy_f * dy_f;

            if dy_sq > radius_sq {
                continue;
            }

            for px_i in min_x..=max_x {
                let x = px_i as u32;
                let dx_f = (x as f32 + 0.5) - stamp_center.x;
                let d_sq = dx_f * dx_f + dy_sq;

                if d_sq <= radius_sq {
                    let d = d_sq.sqrt();
                    let mut alpha = match tool {
                        ToolType::Pencil => {
                            if d_sq <= inner_radius_sq {
                                opacity
                            } else {
                                let edge = (1.0 - (d - inner_r) / (radius - inner_r).max(0.001)).clamp(0.0, 1.0);
                                edge * opacity
                            }
                        }
                        ToolType::Chalk => {
                            let noise = hash21(x, y, (step_index as u32).wrapping_mul(2654435761));
                            if noise > 0.45 {
                                let edge_fade = 1.0 - (d / radius).powi(2);
                                (noise * 0.9 + 0.1) * edge_fade * opacity
                            } else {
                                0.0
                            }
                        }
                        ToolType::Watercolor => {
                            let t = d / radius;
                            let wet_profile = if t < 0.7 {
                                0.35 + 0.25 * (t / 0.7)
                            } else {
                                0.6 + 0.4 * ((t - 0.7) / 0.3)
                            };
                            let noise = hash21(x, y, (step_index as u32).wrapping_mul(1234567));
                            wet_profile * (0.8 + 0.2 * noise) * opacity * 0.45
                        }
                        ToolType::Smudge => {
                            let t = d / radius;
                            let falloff = (1.0 - t * t).max(0.0);
                            let factor = falloff * falloff * (3.0 - 2.0 * falloff);
                            (factor * brush.smudge_strength).clamp(0.0, 1.0) * opacity
                        }
                        ToolType::Eraser => match brush.eraser_mode {
                            EraserMode::HardPixel => {
                                if d <= radius {
                                    opacity
                                } else {
                                    0.0
                                }
                            }
                            _ => {
                                let base_alpha = if d_sq <= inner_radius_sq {
                                    1.0
                                } else {
                                    (1.0 - (d - inner_r) / (radius - inner_r).max(0.001)).clamp(0.0, 1.0)
                                };
                                base_alpha * opacity
                            }
                        },
                        _ => {
                            let base_alpha = if d_sq <= inner_radius_sq {
                                1.0
                            } else {
                                (1.0 - (d - inner_r) / (radius - inner_r).max(0.001)).clamp(0.0, 1.0)
                            };
                            base_alpha * opacity
                        }
                    };

                    // Apply Wet Edge Watercolor Pigment Pooling effect
                    if brush.wet_edge_strength > 0.001 && tool != ToolType::Eraser && tool != ToolType::Smudge {
                        let t = (d / radius).clamp(0.0, 1.0);
                        let fringe_w = brush.wet_edge_fringe_width.clamp(0.05, 0.5);
                        let fringe_start = 1.0 - fringe_w;
                        let pooling = if t >= fringe_start {
                            let ft = (t - fringe_start) / fringe_w;
                            ft * (2.0 - ft) * brush.wet_edge_strength * 1.75
                        } else {
                            0.0
                        };
                        let center_fade = 1.0 - (brush.wet_edge_strength * 0.35) * (1.0 - t * t);
                        alpha = (alpha * center_fade * (1.0 + pooling)).clamp(0.0, 1.0);
                    }

                    if let Some(mask) = active_mask {
                        let mask_val = mask.get_value(x, y);
                        if mask_val <= 8 {
                            continue;
                        }
                        alpha *= mask_val as f32 / 255.0;
                    }

                    if alpha <= 0.001 {
                        continue;
                    }

                    let idx = ((y * doc_w + x) * 4) as usize;
                    let dst = [pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]];

                    if alpha_locked && dst[3] == 0 {
                        continue;
                    }

                    let src = match tool {
                        ToolType::Eraser => {
                            if bg_color.a > 0.0 {
                                bg_color.to_rgba8()
                            } else {
                                [0, 0, 0, 0]
                            }
                        }
                        ToolType::Smudge => {
                            if let Some(prev) = prev_center {
                                let drag_vec = stamp_center - prev;
                                let drag_dist = drag_vec.length();
                                let sample_pos = if drag_dist > 0.001 {
                                    let dir = drag_vec / drag_dist;
                                    let shift = (drag_dist * 1.5).min(radius * 0.9);
                                    Vec2::new(x as f32 - dir.x * shift, y as f32 - dir.y * shift)
                                } else {
                                    Vec2::new(x as f32, y as f32)
                                };
                                sample_bilinear_rgba(pixels, doc_w, doc_h, sample_pos.x, sample_pos.y)
                            } else {
                                dst
                            }
                        }
                        _ => color.to_rgba8(),
                    };

                    if tool == ToolType::Eraser {
                        match brush.eraser_mode {
                            EraserMode::HardPixel => {
                                if alpha >= 0.5 {
                                    if bg_color.a > 0.0 {
                                        pixels[idx..idx + 4].copy_from_slice(&bg_color.to_rgba8());
                                    } else {
                                        pixels[idx..idx + 4].copy_from_slice(&[0, 0, 0, 0]);
                                    }
                                }
                            }
                            EraserMode::ColorErase => {
                                let target = brush.secondary_color.to_rgba8();
                                let tol = brush.color_erase_tolerance as i32;
                                let matches = (dst[0] as i32 - target[0] as i32).abs() <= tol
                                    && (dst[1] as i32 - target[1] as i32).abs() <= tol
                                    && (dst[2] as i32 - target[2] as i32).abs() <= tol;
                                if matches {
                                    let current_a = pixels[idx + 3] as f32 / 255.0;
                                    let new_a = (current_a * (1.0 - alpha)).clamp(0.0, 1.0);
                                    pixels[idx + 3] = (new_a * 255.0).round() as u8;
                                }
                            }
                            EraserMode::Soft => {
                                if bg_color.a <= 0.0 {
                                    let current_a = pixels[idx + 3] as f32 / 255.0;
                                    let new_a = (current_a * (1.0 - alpha)).clamp(0.0, 1.0);
                                    pixels[idx + 3] = (new_a * 255.0).round() as u8;
                                } else {
                                    let mut blended = blend_mode.composite_pixel(dst, src, alpha);
                                    if alpha_locked {
                                        blended[3] = dst[3];
                                    }
                                    pixels[idx..idx + 4].copy_from_slice(&blended);
                                }
                            }
                        }
                    } else if tool == ToolType::Smudge {
                        let strength = brush.smudge_strength.clamp(0.1, 1.0);
                        let mut blended = blend_mode.composite_pixel(dst, src, alpha * strength);
                        if alpha_locked {
                            blended[3] = dst[3];
                        }
                        pixels[idx..idx + 4].copy_from_slice(&blended);
                    } else {
                        let mut blended = blend_mode.composite_pixel(dst, src, alpha);
                        if alpha_locked {
                            blended[3] = dst[3];
                        }
                        pixels[idx..idx + 4].copy_from_slice(&blended);
                    }
                }
            }
        }
    }

    pub fn paint_dot(
        doc: &mut Document,
        point: BrushPoint,
        brush: &BrushSettings,
        symmetry: &SymmetryConfig,
        selection: Option<&SelectionMask>,
    ) {
        let (doc_w, doc_h, active_id, bg_col) = (
            doc.width,
            doc.height,
            doc.active_layer_id,
            doc.background_color(),
        );

        let layer = match doc.get_layer_mut(active_id) {
            Some(l) if !l.locked => l,
            _ => return,
        };

        let radius = brush.effective_size(point.pressure) * 0.5;
        let points = symmetry.transform_points(point.position, doc_w as f32, doc_h as f32);
        let layer_offset = Vec2::new(layer.offset_x as f32, layer.offset_y as f32);
        let alpha_locked = layer.alpha_locked;

        for (i, p) in points.into_iter().enumerate() {
            let local_p = p - layer_offset;
            Self::blend_stamp(
                doc_w,
                doc_h,
                &mut layer.pixels,
                selection,
                local_p,
                None,
                radius,
                brush,
                layer.blend_mode,
                alpha_locked,
                if brush.eraser_to_background { bg_col } else { Color::TRANSPARENT },
                i,
            );
        }
    }

    pub fn paint_segment(
        doc: &mut Document,
        p0: BrushPoint,
        p1: BrushPoint,
        brush: &BrushSettings,
        symmetry: &SymmetryConfig,
        selection: Option<&SelectionMask>,
    ) {
        let (doc_w, doc_h, active_id, bg_col) = (
            doc.width,
            doc.height,
            doc.active_layer_id,
            doc.background_color(),
        );

        let layer = match doc.get_layer_mut(active_id) {
            Some(l) if !l.locked => l,
            _ => return,
        };

        let layer_offset = Vec2::new(layer.offset_x as f32, layer.offset_y as f32);
        let alpha_locked = layer.alpha_locked;
        let dist = p0.position.distance(p1.position);
        let tangent = if dist > 0.001 { Some(p1.position - p0.position) } else { None };
        let cal_factor = brush.calligraphy_factor(tangent);
        let avg_size = (brush.effective_size(p0.pressure) + brush.effective_size(p1.pressure)) * 0.5 * cal_factor;
        let spacing = (avg_size * brush.spacing).max(0.5);
        let steps = (dist / spacing).ceil().max(1.0) as usize;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let pos = p0.position.lerp(p1.position, t);
            let prev_pos = if i > 0 {
                let prev_t = (i - 1) as f32 / steps as f32;
                Some(p0.position.lerp(p1.position, prev_t))
            } else {
                Some(p0.position)
            };
            let pressure = p0.pressure + (p1.pressure - p0.pressure) * t;
            let radius = brush.effective_size(pressure) * 0.5 * cal_factor;

            let sym_points = symmetry.transform_points(pos, doc_w as f32, doc_h as f32);
            let prev_sym_points = prev_pos.map(|pp| symmetry.transform_points(pp, doc_w as f32, doc_h as f32));

            for (s_idx, sp) in sym_points.into_iter().enumerate() {
                let prev_sp = prev_sym_points.as_ref().map(|psp| psp[s_idx] - layer_offset);
                let local_sp = sp - layer_offset;
                Self::blend_stamp(
                    doc_w,
                    doc_h,
                    &mut layer.pixels,
                    selection,
                    local_sp,
                    prev_sp,
                    radius,
                    brush,
                    layer.blend_mode,
                    alpha_locked,
                    if brush.eraser_to_background { bg_col } else { Color::TRANSPARENT },
                    i.wrapping_add(s_idx * 1000),
                );
            }
        }
    }

    #[inline]
    fn catmull_rom_eval(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * (
            (2.0 * p1)
                + (-p0 + p2) * t
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3
        )
    }

    pub fn paint_spline(
        doc: &mut Document,
        p0: BrushPoint,
        p1: BrushPoint,
        p2: BrushPoint,
        p3: BrushPoint,
        brush: &BrushSettings,
        symmetry: &SymmetryConfig,
        selection: Option<&SelectionMask>,
    ) {
        let (doc_w, doc_h, active_id, bg_col) = (
            doc.width,
            doc.height,
            doc.active_layer_id,
            doc.background_color(),
        );

        let layer = match doc.get_layer_mut(active_id) {
            Some(l) if !l.locked => l,
            _ => return,
        };

        let layer_offset = Vec2::new(layer.offset_x as f32, layer.offset_y as f32);
        let alpha_locked = layer.alpha_locked;

        let approx_len = p1.position.distance(p2.position);
        let avg_size = (brush.effective_size(p1.pressure) + brush.effective_size(p2.pressure)) * 0.5;
        let spacing = (avg_size * brush.spacing).max(0.5);
        let steps = (approx_len / spacing).ceil().max(1.0) as usize;

        let mut prev_pos = p1.position;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let pos = Self::catmull_rom_eval(p0.position, p1.position, p2.position, p3.position, t);
            let tangent = if pos.distance(prev_pos) > 0.001 {
                Some(pos - prev_pos)
            } else {
                Some(p2.position - p1.position)
            };
            let cal_factor = brush.calligraphy_factor(tangent);
            let pressure = p1.pressure + (p2.pressure - p1.pressure) * t;
            let radius = brush.effective_size(pressure) * 0.5 * cal_factor;

            let sym_points = symmetry.transform_points(pos, doc_w as f32, doc_h as f32);
            let prev_sym_points = symmetry.transform_points(prev_pos, doc_w as f32, doc_h as f32);

            for (s_idx, sp) in sym_points.into_iter().enumerate() {
                let prev_sp = Some(prev_sym_points[s_idx] - layer_offset);
                let local_sp = sp - layer_offset;
                Self::blend_stamp(
                    doc_w,
                    doc_h,
                    &mut layer.pixels,
                    selection,
                    local_sp,
                    prev_sp,
                    radius,
                    brush,
                    layer.blend_mode,
                    alpha_locked,
                    if brush.eraser_to_background { bg_col } else { Color::TRANSPARENT },
                    i.wrapping_add(s_idx * 1000),
                );
            }
            prev_pos = pos;
        }
    }

    pub fn rasterize_line(
        doc: &mut Document,
        p0: Vec2,
        p1: Vec2,
        brush: &BrushSettings,
        symmetry: &SymmetryConfig,
        selection: Option<&SelectionMask>,
    ) {
        let pt0 = BrushPoint::new(p0, 1.0);
        let pt1 = BrushPoint::new(p1, 1.0);
        Self::paint_segment(doc, pt0, pt1, brush, symmetry, selection);
    }

    pub fn rasterize_rect(
        doc: &mut Document,
        start: Vec2,
        end: Vec2,
        brush: &BrushSettings,
        symmetry: &SymmetryConfig,
        selection: Option<&SelectionMask>,
    ) {
        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        let min_y = start.y.min(end.y);
        let max_y = start.y.max(end.y);

        let (doc_w, doc_h, active_id) = (doc.width, doc.height, doc.active_layer_id);
        let layer = match doc.get_layer_mut(active_id) {
            Some(l) if !l.locked => l,
            _ => return,
        };

        let active_mask = selection.filter(|s| s.has_selection());

        let mode = brush.shape_fill_mode;
        let stroke_w = (brush.size * 0.5).max(1.0);

        if mode == ShapeFillMode::Fill || mode == ShapeFillMode::Both {
            let sym_boxes = symmetry.transform_points(Vec2::new(min_x, min_y), doc_w as f32, doc_h as f32);
            let sym_ends = symmetry.transform_points(Vec2::new(max_x, max_y), doc_w as f32, doc_h as f32);

            for (b0, b1) in sym_boxes.into_iter().zip(sym_ends.into_iter()) {
                let bx0 = b0.x.min(b1.x).floor().max(0.0) as u32;
                let bx1 = b0.x.max(b1.x).ceil().min(doc_w as f32) as u32;
                let by0 = b0.y.min(b1.y).floor().max(0.0) as u32;
                let by1 = b0.y.max(b1.y).ceil().min(doc_h as f32) as u32;

                let fill_col = brush.secondary_color.to_rgba8();
                for y in by0..by1 {
                    for x in bx0..bx1 {
                        if let Some(mask) = active_mask {
                            if mask.get_value(x, y) < 8 {
                                continue;
                            }
                        }
                        let idx = ((y * doc_w + x) * 4) as usize;
                        let dst = [layer.pixels[idx], layer.pixels[idx + 1], layer.pixels[idx + 2], layer.pixels[idx + 3]];
                        let blended = layer.blend_mode.composite_pixel(dst, fill_col, brush.opacity);
                        layer.pixels[idx..idx + 4].copy_from_slice(&blended);
                    }
                }
            }
        }

        if mode == ShapeFillMode::Stroke || mode == ShapeFillMode::Both {
            let p0 = Vec2::new(min_x, min_y);
            let p1 = Vec2::new(max_x, min_y);
            let p2 = Vec2::new(max_x, max_y);
            let p3 = Vec2::new(min_x, max_y);

            let mut stroke_brush = brush.clone();
            stroke_brush.size = stroke_w;
            Self::paint_segment(doc, BrushPoint::new(p0, 1.0), BrushPoint::new(p1, 1.0), &stroke_brush, symmetry, selection);
            Self::paint_segment(doc, BrushPoint::new(p1, 1.0), BrushPoint::new(p2, 1.0), &stroke_brush, symmetry, selection);
            Self::paint_segment(doc, BrushPoint::new(p2, 1.0), BrushPoint::new(p3, 1.0), &stroke_brush, symmetry, selection);
            Self::paint_segment(doc, BrushPoint::new(p3, 1.0), BrushPoint::new(p0, 1.0), &stroke_brush, symmetry, selection);
        }
    }

    pub fn rasterize_ellipse(
        doc: &mut Document,
        start: Vec2,
        end: Vec2,
        brush: &BrushSettings,
        symmetry: &SymmetryConfig,
        selection: Option<&SelectionMask>,
    ) {
        let center = (start + end) * 0.5;
        let rx = (end.x - start.x).abs() * 0.5;
        let ry = (end.y - start.y).abs() * 0.5;

        if rx < 0.5 || ry < 0.5 {
            return;
        }

        let (doc_w, doc_h, active_id) = (doc.width, doc.height, doc.active_layer_id);
        let layer = match doc.get_layer_mut(active_id) {
            Some(l) if !l.locked => l,
            _ => return,
        };

        let active_mask = selection.filter(|s| s.has_selection());
        let mode = brush.shape_fill_mode;
        let stroke_w = brush.size.max(1.0);
        let half_w = stroke_w * 0.5;
        let sym_centers = symmetry.transform_points(center, doc_w as f32, doc_h as f32);

        let rx_sq = rx * rx;
        let ry_sq = ry * ry;

        for sc in sym_centers {
            let pad = stroke_w + 2.0;
            let min_x = ((sc.x - rx - pad).floor().max(0.0) as u32).min(doc_w);
            let max_x = ((sc.x + rx + pad).ceil().max(0.0) as u32).min(doc_w);
            let min_y = ((sc.y - ry - pad).floor().max(0.0) as u32).min(doc_h);
            let max_y = ((sc.y + ry + pad).ceil().max(0.0) as u32).min(doc_h);

            for y in min_y..max_y {
                let py = (y as f32 + 0.5) - sc.y;
                let py_term = (py * py) / ry_sq;
                let gy = 2.0 * py / ry_sq;

                for x in min_x..max_x {
                    let px = (x as f32 + 0.5) - sc.x;
                    let px_term = (px * px) / rx_sq;
                    let gx = 2.0 * px / rx_sq;

                    let d_norm = px_term + py_term;
                    let grad_len = (gx * gx + gy * gy).sqrt().max(1e-6);
                    let dist_to_boundary = (d_norm - 1.0) / grad_len;

                    let (should_paint, color, alpha) = match mode {
                        ShapeFillMode::Fill => {
                            let aa = (0.5 - dist_to_boundary).clamp(0.0, 1.0);
                            (aa > 0.001, brush.secondary_color, aa * brush.opacity)
                        }
                        ShapeFillMode::Stroke => {
                            let d_stroke = dist_to_boundary.abs() - half_w;
                            let aa = (0.5 - d_stroke).clamp(0.0, 1.0);
                            (aa > 0.001, brush.primary_color, aa * brush.opacity)
                        }
                        ShapeFillMode::Both => {
                            let fill_aa = (0.5 - dist_to_boundary).clamp(0.0, 1.0);
                            let d_stroke = dist_to_boundary.abs() - half_w;
                            let stroke_aa = (0.5 - d_stroke).clamp(0.0, 1.0);

                            if stroke_aa > 0.001 {
                                (true, brush.primary_color, stroke_aa * brush.opacity)
                            } else if fill_aa > 0.001 {
                                (true, brush.secondary_color, fill_aa * brush.opacity)
                            } else {
                                (false, brush.primary_color, 0.0)
                            }
                        }
                    };

                    if should_paint && alpha > 0.001 {
                        if let Some(mask) = active_mask {
                            if mask.get_value(x, y) < 8 {
                                continue;
                            }
                        }

                        let idx = ((y * doc_w + x) * 4) as usize;
                        let dst = [layer.pixels[idx], layer.pixels[idx + 1], layer.pixels[idx + 2], layer.pixels[idx + 3]];
                        let blended = layer.blend_mode.composite_pixel(dst, color.to_rgba8(), alpha);
                        layer.pixels[idx..idx + 4].copy_from_slice(&blended);
                    }
                }
            }
        }
    }

    pub fn rasterize_polygon(
        doc: &mut Document,
        points: &[Vec2],
        brush: &BrushSettings,
        symmetry: &SymmetryConfig,
        selection: Option<&SelectionMask>,
    ) {
        if points.len() < 2 {
            return;
        }

        let active_mask = selection.filter(|s| s.has_selection());
        let mode = brush.shape_fill_mode;
        if (mode == ShapeFillMode::Fill || mode == ShapeFillMode::Both) && points.len() >= 3 {
            let mask = SelectionMask::from_polygon(doc.width, doc.height, points);
            let (doc_w, doc_h, active_id) = (doc.width, doc.height, doc.active_layer_id);
            let layer = match doc.get_layer_mut(active_id) {
                Some(l) if !l.locked => l,
                _ => return,
            };

            let fill_col = brush.secondary_color.to_rgba8();
            for y in 0..doc_h {
                for x in 0..doc_w {
                    if mask.get_value(x, y) > 8 {
                        if let Some(sel) = active_mask {
                            if sel.get_value(x, y) < 8 {
                                continue;
                            }
                        }
                        let idx = ((y * doc_w + x) * 4) as usize;
                        let dst = [layer.pixels[idx], layer.pixels[idx + 1], layer.pixels[idx + 2], layer.pixels[idx + 3]];
                        let blended = layer.blend_mode.composite_pixel(dst, fill_col, brush.opacity);
                        layer.pixels[idx..idx + 4].copy_from_slice(&blended);
                    }
                }
            }
        }

        if mode == ShapeFillMode::Stroke || mode == ShapeFillMode::Both {
            for i in 0..points.len() {
                let p0 = points[i];
                let p1 = points[(i + 1) % points.len()];
                Self::paint_segment(doc, BrushPoint::new(p0, 1.0), BrushPoint::new(p1, 1.0), brush, symmetry, selection);
            }
        }
    }

    pub fn rasterize_gradient(
        doc: &mut Document,
        start: Vec2,
        end: Vec2,
        brush: &BrushSettings,
        selection: Option<&SelectionMask>,
    ) {
        let (doc_w, doc_h, active_id) = (doc.width, doc.height, doc.active_layer_id);
        let layer = match doc.get_layer_mut(active_id) {
            Some(l) if !l.locked => l,
            _ => return,
        };

        let active_mask = selection.filter(|s| s.has_selection());
        let col0 = brush.primary_color;
        let col1 = brush.secondary_color;
        let diff = end - start;
        let length_sq = diff.length_squared().max(1.0);
        let length = length_sq.sqrt();
        let grad_type = brush.gradient_type;
        let dither = brush.gradient_dither;

        for y in 0..doc_h {
            for x in 0..doc_w {
                if let Some(mask) = active_mask {
                    if mask.get_value(x, y) < 8 {
                        continue;
                    }
                }

                let cur = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let mut t = match grad_type {
                    GradientType::Linear => {
                        let to_cur = cur - start;
                        (to_cur.dot(diff) / length_sq).clamp(0.0, 1.0)
                    }
                    GradientType::Radial => {
                        (cur.distance(start) / length).clamp(0.0, 1.0)
                    }
                };

                if dither {
                    let noise = (hash21(x, y, 987654) - 0.5) * (1.0 / 255.0);
                    t = (t + noise).clamp(0.0, 1.0);
                }

                let color = col0.lerp(col1, t);
                let idx = ((y * doc_w + x) * 4) as usize;
                let dst = [layer.pixels[idx], layer.pixels[idx + 1], layer.pixels[idx + 2], layer.pixels[idx + 3]];
                let blended = layer.blend_mode.composite_pixel(dst, color.to_rgba8(), brush.opacity);
                layer.pixels[idx..idx + 4].copy_from_slice(&blended);
            }
        }
    }

    pub fn rasterize_magic_wand(
        doc: &Document,
        start_x: u32,
        start_y: u32,
        tolerance: u8,
        contiguous: bool,
        sample_all_layers: bool,
    ) -> SelectionMask {
        let (doc_w, doc_h) = (doc.width, doc.height);
        let mut mask = SelectionMask::new(doc_w, doc_h);
        if start_x >= doc_w || start_y >= doc_h {
            return mask;
        }

        let source_buffer = if sample_all_layers {
            doc.composite_layers(false)
        } else if let Some(ref_layer) = doc.reference_layer() {
            ref_layer.pixels.clone()
        } else if let Some(layer) = doc.active_layer() {
            layer.pixels.clone()
        } else {
            return mask;
        };

        let start_idx = ((start_y * doc_w + start_x) * 4) as usize;
        let target_px = [
            source_buffer[start_idx],
            source_buffer[start_idx + 1],
            source_buffer[start_idx + 2],
            source_buffer[start_idx + 3],
        ];

        let tol = tolerance as i32;
        let matches_target = |px: [u8; 4]| -> bool {
            (px[0] as i32 - target_px[0] as i32).abs() <= tol
                && (px[1] as i32 - target_px[1] as i32).abs() <= tol
                && (px[2] as i32 - target_px[2] as i32).abs() <= tol
                && (px[3] as i32 - target_px[3] as i32).abs() <= tol
        };

        if !contiguous {
            // Global color selection across entire image
            for y in 0..doc_h {
                for x in 0..doc_w {
                    let idx = ((y * doc_w + x) * 4) as usize;
                    let current = [
                        source_buffer[idx],
                        source_buffer[idx + 1],
                        source_buffer[idx + 2],
                        source_buffer[idx + 3],
                    ];
                    if matches_target(current) {
                        let m_idx = (y * doc_w + x) as usize;
                        mask.mask[m_idx] = 255;
                    }
                }
            }
        } else {
            // Connected component flood fill
            let mut queue = Vec::with_capacity(4096);
            queue.push((start_x, start_y));
            let mut visited = vec![false; (doc_w * doc_h) as usize];

            while let Some((x, y)) = queue.pop() {
                let pos = (y * doc_w + x) as usize;
                if visited[pos] {
                    continue;
                }
                visited[pos] = true;

                let idx = pos * 4;
                let current = [
                    source_buffer[idx],
                    source_buffer[idx + 1],
                    source_buffer[idx + 2],
                    source_buffer[idx + 3],
                ];

                if matches_target(current) {
                    mask.mask[pos] = 255;

                    if x > 0 {
                        queue.push((x - 1, y));
                    }
                    if x + 1 < doc_w {
                        queue.push((x + 1, y));
                    }
                    if y > 0 {
                        queue.push((x, y - 1));
                    }
                    if y + 1 < doc_h {
                        queue.push((x, y + 1));
                    }
                }
            }
        }

        mask.recompute_metadata();
        mask
    }

    pub fn flood_fill(
        doc: &mut Document,
        start_x: u32,
        start_y: u32,
        fill_color: Color,
        selection: Option<&SelectionMask>,
        tolerance: u8,
    ) {
        let (doc_w, doc_h, active_id) = (doc.width, doc.height, doc.active_layer_id);
        if start_x >= doc_w || start_y >= doc_h {
            return;
        }

        let ref_pixels = doc.reference_layer().map(|l| l.pixels.clone());

        let layer = match doc.get_layer_mut(active_id) {
            Some(l) if !l.locked => l,
            _ => return,
        };

        let has_ref = ref_pixels.is_some();
        let source_buffer = ref_pixels.unwrap_or_else(|| layer.pixels.clone());

        let target_idx = ((start_y * doc_w + start_x) * 4) as usize;
        let target_px = [
            source_buffer[target_idx],
            source_buffer[target_idx + 1],
            source_buffer[target_idx + 2],
            source_buffer[target_idx + 3],
        ];
        let fill_px = fill_color.to_rgba8();

        if target_px == fill_px && !has_ref {
            return;
        }

        let mut queue = Vec::with_capacity(2048);
        queue.push((start_x, start_y));
        let mut visited = vec![false; (doc_w * doc_h) as usize];

        let matches_target = |px: [u8; 4]| -> bool {
            (px[0] as i32 - target_px[0] as i32).abs() <= tolerance as i32
                && (px[1] as i32 - target_px[1] as i32).abs() <= tolerance as i32
                && (px[2] as i32 - target_px[2] as i32).abs() <= tolerance as i32
                && (px[3] as i32 - target_px[3] as i32).abs() <= tolerance as i32
        };

        let active_mask = selection.filter(|s| s.has_selection());

        while let Some((x, y)) = queue.pop() {
            let pos = (y * doc_w + x) as usize;
            if visited[pos] {
                continue;
            }
            visited[pos] = true;

            if let Some(mask) = active_mask {
                if mask.get_value(x, y) < 8 {
                    continue;
                }
            }

            let idx = pos * 4;
            let current = [
                source_buffer[idx],
                source_buffer[idx + 1],
                source_buffer[idx + 2],
                source_buffer[idx + 3],
            ];

            if matches_target(current) {
                layer.pixels[idx..idx + 4].copy_from_slice(&fill_px);

                if x > 0 {
                    queue.push((x - 1, y));
                }
                if x + 1 < doc_w {
                    queue.push((x + 1, y));
                }
                if y > 0 {
                    queue.push((x, y - 1));
                }
                if y + 1 < doc_h {
                    queue.push((x, y + 1));
                }
            }
        }
    }

    /// Attempt to load a font from optional custom font bytes, file path, or common system font paths.
    pub fn load_font(custom_bytes: Option<&[u8]>, custom_path: Option<&str>) -> Option<FontArc> {
        if let Some(bytes) = custom_bytes {
            if let Ok(font) = FontArc::try_from_vec(bytes.to_vec()) {
                return Some(font);
            }
        }
        if let Some(path_str) = custom_path {
            if let Ok(data) = std::fs::read(path_str) {
                if let Ok(font) = FontArc::try_from_vec(data) {
                    return Some(font);
                }
            }
        }

        // Standard OS font search paths
        let system_paths = [
            // Windows
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\calibri.ttf",
            r"C:\Windows\Fonts\consola.ttf",
            r"C:\Windows\Fonts\tahoma.ttf",
            // Linux
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
            // macOS
            "/System/Library/Fonts/SFNS.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial.ttf",
        ];

        for path in &system_paths {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(font) = FontArc::try_from_vec(data) {
                    return Some(font);
                }
            }
        }
        None
    }

    /// Measure text bounding box (width, height) without rendering.
    pub fn measure_text(
        text: &str,
        font: &FontArc,
        font_size: f32,
        line_spacing_mult: f32,
        letter_spacing: f32,
    ) -> (f32, f32) {
        let scale = PxScale::from(font_size.max(4.0));
        let scaled_font = font.as_scaled(scale);
        let line_height = (scaled_font.ascent() - scaled_font.descent() + scaled_font.line_gap()) * line_spacing_mult.max(0.5);

        let lines: Vec<&str> = text.split('\n').collect();
        let mut max_width: f32 = 0.0;

        for line in &lines {
            let mut line_w: f32 = 0.0;
            for c in line.chars() {
                let glyph = scaled_font.scaled_glyph(c);
                line_w += scaled_font.h_advance(glyph.id) + letter_spacing;
            }
            if line_w > max_width {
                max_width = line_w;
            }
        }
        let total_h = (lines.len() as f32 * line_height).max(font_size);
        (max_width, total_h)
    }

    /// Rasterize formatted text directly onto the active layer.
    /// Returns bounding box (min_x, min_y, max_x, max_y) in canvas coordinates if anything was rendered.
    pub fn rasterize_text(
        doc: &mut Document,
        pos: Vec2,
        text: &str,
        font: Option<&FontArc>,
        font_size: f32,
        color: Color,
        opacity: f32,
        line_spacing_mult: f32,
        letter_spacing: f32,
        align: TextAlign,
        selection: Option<&SelectionMask>,
    ) -> Option<(f32, f32, f32, f32)> {
        if text.is_empty() {
            return None;
        }

        let (doc_w, doc_h, active_id) = (doc.width, doc.height, doc.active_layer_id);
        let layer = match doc.get_layer_mut(active_id) {
            Some(l) if !l.locked => l,
            _ => return None,
        };

        let loaded_font_storage;
        let font_ref = match font {
            Some(f) => f,
            None => {
                loaded_font_storage = Self::load_font(None, None);
                match &loaded_font_storage {
                    Some(f) => f,
                    None => return None,
                }
            }
        };

        let scale = PxScale::from(font_size.max(4.0));
        let scaled_font = font_ref.as_scaled(scale);
        let ascent = scaled_font.ascent();
        let descent = scaled_font.descent();
        let line_gap = scaled_font.line_gap();
        let line_height = (ascent - descent + line_gap) * line_spacing_mult.max(0.5);

        let lines: Vec<&str> = text.split('\n').collect();
        let active_mask = selection.filter(|s| s.has_selection());
        let src_col = color.to_rgba8();
        let alpha_locked = layer.alpha_locked;
        let blend_mode = layer.blend_mode;

        let mut overall_min_x = f32::MAX;
        let mut overall_min_y = f32::MAX;
        let mut overall_max_x = f32::MIN;
        let mut overall_max_y = f32::MIN;
        let mut drawn_any = false;

        for (line_idx, line) in lines.iter().enumerate() {
            let mut line_w: f32 = 0.0;
            for c in line.chars() {
                let glyph = scaled_font.scaled_glyph(c);
                line_w += scaled_font.h_advance(glyph.id) + letter_spacing;
            }

            let start_x = match align {
                TextAlign::Left => pos.x,
                TextAlign::Center => pos.x - line_w * 0.5,
                TextAlign::Right => pos.x - line_w,
            };

            let baseline_y = pos.y + ascent + (line_idx as f32 * line_height);
            let mut current_x = start_x;

            for c in line.chars() {
                let mut glyph = scaled_font.scaled_glyph(c);
                glyph.position = point(current_x, baseline_y);
                let glyph_id = glyph.id;

                if let Some(outlined) = font_ref.outline_glyph(glyph) {
                    let bounds = outlined.px_bounds();
                    outlined.draw(|gx, gy, coverage| {
                        if coverage <= 0.005 {
                            return;
                        }
                        let cx = bounds.min.x as i32 + gx as i32;
                        let cy = bounds.min.y as i32 + gy as i32;

                        if cx >= 0 && cx < doc_w as i32 && cy >= 0 && cy < doc_h as i32 {
                            let ux = cx as u32;
                            let uy = cy as u32;

                            let mut mask_factor = 1.0_f32;
                            if let Some(mask) = active_mask {
                                let mv = mask.get_value(ux, uy);
                                if mv < 8 {
                                    return;
                                }
                                mask_factor = mv as f32 / 255.0;
                            }

                            let idx = ((uy * doc_w + ux) * 4) as usize;
                            let dst = [
                                layer.pixels[idx],
                                layer.pixels[idx + 1],
                                layer.pixels[idx + 2],
                                layer.pixels[idx + 3],
                            ];

                            if alpha_locked && dst[3] == 0 {
                                return;
                            }

                            let effective_alpha = coverage * opacity * color.a * mask_factor;
                            if effective_alpha <= 0.001 {
                                return;
                            }

                            let mut blended = blend_mode.composite_pixel(dst, src_col, effective_alpha);
                            if alpha_locked {
                                blended[3] = dst[3];
                            }
                            layer.pixels[idx..idx + 4].copy_from_slice(&blended);

                            drawn_any = true;
                            if (cx as f32) < overall_min_x { overall_min_x = cx as f32; }
                            if (cx as f32) > overall_max_x { overall_max_x = cx as f32; }
                            if (cy as f32) < overall_min_y { overall_min_y = cy as f32; }
                            if (cy as f32) > overall_max_y { overall_max_y = cy as f32; }
                        }
                    });
                }
                current_x += scaled_font.h_advance(glyph_id) + letter_spacing;
            }
        }

        if drawn_any {
            Some((overall_min_x, overall_min_y, overall_max_x, overall_max_y))
        } else {
            None
        }
    }
}
