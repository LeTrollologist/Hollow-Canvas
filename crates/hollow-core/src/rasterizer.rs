use crate::blend::BlendMode;
use crate::brush::{BrushPoint, BrushSettings, EraserMode, GradientType, ShapeFillMode, ToolType};
use crate::color::Color;
use crate::document::Document;
use crate::selection::SelectionMask;
use crate::symmetry::SymmetryConfig;
use glam::Vec2;

#[inline]
fn hash21(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = seed ^ (x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h as f32) / (u32::MAX as f32)
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
        bg_color: Color,
        step_index: usize,
    ) {
        let tool = brush.tool;
        let color = brush.primary_color;
        let opacity = brush.opacity;
        let hardness = brush.hardness;

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

                    if let Some(mask) = selection {
                        if mask.has_selection() && mask.get_value(uix, uiy) < 8 {
                            continue;
                        }
                    }

                    let p_alpha = (0.25 + 0.75 * hash21(i as u32, 3, seed)) * opacity * 0.45;
                    let idx = ((uiy * doc_w + uix) * 4) as usize;
                    let dst = [pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]];
                    let src = color.to_rgba8();
                    let blended = blend_mode.composite_pixel(dst, src, p_alpha);
                    pixels[idx..idx + 4].copy_from_slice(&blended);
                }
            }
            return;
        }

        let min_x = ((stamp_center.x - radius).floor().max(0.0) as u32).min(doc_w);
        let max_x = ((stamp_center.x + radius).ceil().max(0.0) as u32).min(doc_w);
        let min_y = ((stamp_center.y - radius).floor().max(0.0) as u32).min(doc_h);
        let max_y = ((stamp_center.y + radius).ceil().max(0.0) as u32).min(doc_h);

        let r_sq = radius * radius;
        let clamped_hardness = hardness.clamp(0.05, 1.0);
        let inner_radius_sq = (radius * clamped_hardness) * (radius * clamped_hardness);
        let seed = (step_index as u32).wrapping_mul(2654435761);

        for y in min_y..max_y {
            let dy = y as f32 + 0.5 - stamp_center.y;
            let dy_sq = dy * dy;

            for x in min_x..max_x {
                let dx = x as f32 + 0.5 - stamp_center.x;
                let d_sq = dx * dx + dy_sq;

                if d_sq <= r_sq {
                    let d = d_sq.sqrt();
                    let mut alpha = match tool {
                        ToolType::Pencil => {
                            if d <= radius {
                                let edge_fade = (1.0 - (d / radius).powi(4)).clamp(0.0, 1.0);
                                let noise = 0.85 + 0.15 * hash21(x, y, 777);
                                edge_fade * noise * opacity
                            } else {
                                0.0
                            }
                        }
                        ToolType::Watercolor => {
                            let wetness = brush.watercolor_wetness.clamp(0.1, 1.0);
                            let normalized_d = d / radius;
                            let wet_edge = 1.0 + 0.4 * (normalized_d - 0.7).max(0.0);
                            let falloff = (1.0 - normalized_d).powf(0.85) * wet_edge;
                            falloff.clamp(0.0, 1.0) * opacity * (0.35 + 0.45 * wetness)
                        }
                        ToolType::Chalk => {
                            let grain = brush.chalk_grain.clamp(0.1, 1.0);
                            let noise = hash21(x, y, seed);
                            if noise < (1.0 - grain * 0.7) {
                                let falloff = (1.0 - (d / radius)).clamp(0.0, 1.0);
                                falloff * opacity * (0.5 + 0.5 * noise)
                            } else {
                                0.0
                            }
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
                                    let inner_r = radius * clamped_hardness;
                                    (1.0 - (d - inner_r) / (radius - inner_r)).clamp(0.0, 1.0)
                                };
                                base_alpha * opacity
                            }
                        },
                        _ => {
                            let base_alpha = if d_sq <= inner_radius_sq {
                                1.0
                            } else {
                                let inner_r = radius * clamped_hardness;
                                (1.0 - (d - inner_r) / (radius - inner_r)).clamp(0.0, 1.0)
                            };
                            base_alpha * opacity
                        }
                    };

                    if let Some(mask) = selection {
                        if mask.has_selection() {
                            let mask_val = mask.get_value(x, y) as f32 / 255.0;
                            alpha *= mask_val;
                        }
                    }

                    if alpha <= 0.001 {
                        continue;
                    }

                    let idx = ((y * doc_w + x) * 4) as usize;
                    let dst = [pixels[idx], pixels[idx + 1], pixels[idx + 2], pixels[idx + 3]];

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
                                let sx = (x as f32 + (prev.x - stamp_center.x)).round() as i32;
                                let sy = (y as f32 + (prev.y - stamp_center.y)).round() as i32;
                                if sx >= 0 && sx < doc_w as i32 && sy >= 0 && sy < doc_h as i32 {
                                    let s_idx = ((sy as u32 * doc_w + sx as u32) * 4) as usize;
                                    [pixels[s_idx], pixels[s_idx + 1], pixels[s_idx + 2], pixels[s_idx + 3]]
                                } else {
                                    dst
                                }
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
                                    let blended = blend_mode.composite_pixel(dst, src, alpha);
                                    pixels[idx..idx + 4].copy_from_slice(&blended);
                                }
                            }
                        }
                    } else if tool == ToolType::Smudge {
                        let strength = brush.smudge_strength.clamp(0.1, 1.0);
                        let blended = blend_mode.composite_pixel(dst, src, alpha * strength);
                        pixels[idx..idx + 4].copy_from_slice(&blended);
                    } else {
                        let blended = blend_mode.composite_pixel(dst, src, alpha);
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
        let dist = p0.position.distance(p1.position);
        let avg_size = (brush.effective_size(p0.pressure) + brush.effective_size(p1.pressure)) * 0.5;
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
            let radius = brush.effective_size(pressure) * 0.5;

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
        let approx_len = p1.position.distance(p2.position);
        let avg_size = (brush.effective_size(p1.pressure) + brush.effective_size(p2.pressure)) * 0.5;
        let spacing = (avg_size * brush.spacing).max(0.5);
        let steps = (approx_len / spacing).ceil().max(2.0) as usize;

        let mut prev_pt = p1;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let pos = Self::catmull_rom_eval(p0.position, p1.position, p2.position, p3.position, t);
            let pressure = p1.pressure + (p2.pressure - p1.pressure) * t;
            let cur_pt = BrushPoint::new(pos, pressure);

            Self::paint_segment(doc, prev_pt, cur_pt, brush, symmetry, selection);
            prev_pt = cur_pt;
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
                        if let Some(mask) = selection {
                            if mask.has_selection() && mask.get_value(x, y) < 8 {
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

        let mode = brush.shape_fill_mode;
        let stroke_w = (brush.size * 0.5).max(1.0);
        let sym_centers = symmetry.transform_points(center, doc_w as f32, doc_h as f32);

        for sc in sym_centers {
            let min_x = ((sc.x - rx - stroke_w).floor().max(0.0) as u32).min(doc_w);
            let max_x = ((sc.x + rx + stroke_w).ceil().max(0.0) as u32).min(doc_w);
            let min_y = ((sc.y - ry - stroke_w).floor().max(0.0) as u32).min(doc_h);
            let max_y = ((sc.y + ry + stroke_w).ceil().max(0.0) as u32).min(doc_h);

            for y in min_y..max_y {
                let dy = (y as f32 + 0.5 - sc.y) / ry;
                let dy_sq = dy * dy;

                for x in min_x..max_x {
                    let dx = (x as f32 + 0.5 - sc.x) / rx;
                    let norm_dist = (dx * dx + dy_sq).sqrt();

                    let (should_paint, color, alpha) = match mode {
                        ShapeFillMode::Fill => {
                            if norm_dist <= 1.0 {
                                let aa = ((1.0 - norm_dist) * rx.min(ry)).clamp(0.0, 1.0);
                                (true, brush.secondary_color, aa * brush.opacity)
                            } else {
                                (false, brush.secondary_color, 0.0)
                            }
                        }
                        ShapeFillMode::Stroke => {
                            let dist_pixels = (norm_dist - 1.0).abs() * rx.min(ry);
                            if dist_pixels <= stroke_w * 0.5 {
                                let aa = (1.0 - (dist_pixels / (stroke_w * 0.5))).clamp(0.0, 1.0);
                                (true, brush.primary_color, aa * brush.opacity)
                            } else {
                                (false, brush.primary_color, 0.0)
                            }
                        }
                        ShapeFillMode::Both => {
                            if norm_dist <= 1.0 {
                                (true, brush.secondary_color, brush.opacity)
                            } else {
                                let dist_pixels = (norm_dist - 1.0).abs() * rx.min(ry);
                                if dist_pixels <= stroke_w * 0.5 {
                                    let aa = (1.0 - (dist_pixels / (stroke_w * 0.5))).clamp(0.0, 1.0);
                                    (true, brush.primary_color, aa * brush.opacity)
                                } else {
                                    (false, brush.primary_color, 0.0)
                                }
                            }
                        }
                    };

                    if should_paint && alpha > 0.001 {
                        if let Some(mask) = selection {
                            if mask.has_selection() && mask.get_value(x, y) < 8 {
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
                        if let Some(sel) = selection {
                            if sel.has_selection() && sel.get_value(x, y) < 8 {
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

        let col0 = brush.primary_color;
        let col1 = brush.secondary_color;
        let diff = end - start;
        let length_sq = diff.length_squared().max(1.0);
        let length = length_sq.sqrt();
        let grad_type = brush.gradient_type;
        let dither = brush.gradient_dither;

        for y in 0..doc_h {
            for x in 0..doc_w {
                if let Some(mask) = selection {
                    if mask.has_selection() && mask.get_value(x, y) < 8 {
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

        while let Some((x, y)) = queue.pop() {
            let pos = (y * doc_w + x) as usize;
            if visited[pos] {
                continue;
            }
            visited[pos] = true;

            if let Some(mask) = selection {
                if mask.has_selection() && mask.get_value(x, y) < 8 {
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
}
