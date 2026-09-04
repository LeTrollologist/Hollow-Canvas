use egui::{pos2, Color32, Painter, Rect, Stroke, Vec2};
use hollow_core::brush::ToolType;

/// Draws a crisp, vector-rendered studio tool icon inside the given bounding rectangle.
pub fn draw_tool_icon(painter: &Painter, rect: Rect, tool: ToolType, color: Color32, is_active: bool) {
    let stroke = Stroke::new(1.4_f32, color);
    let fill = if is_active {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 60)
    } else {
        Color32::TRANSPARENT
    };

    let cx = rect.center().x;
    let cy = rect.center().y;
    let r = rect.width().min(rect.height()) * 0.42_f32;

    match tool {
        ToolType::Brush => {
            // Tapered artist brush tip
            let p0 = pos2(cx - r * 0.7_f32, cy + r * 0.7_f32);
            let p1 = pos2(cx - r * 0.2_f32, cy + r * 0.2_f32);
            let p2 = pos2(cx + r * 0.7_f32, cy - r * 0.7_f32);
            painter.line_segment([p0, p1], Stroke::new(2.5_f32, color));
            painter.line_segment([p1, p2], Stroke::new(1.8_f32, color));
            painter.circle_filled(p2, 1.6_f32, color);
        }

        ToolType::Pencil => {
            // 45-degree angled precision pencil
            let p_back = pos2(cx - r * 0.65_f32, cy + r * 0.65_f32);
            let p_tip = pos2(cx + r * 0.65_f32, cy - r * 0.65_f32);
            let p_nib = pos2(cx + r * 0.8_f32, cy - r * 0.8_f32);
            painter.line_segment([p_back, p_tip], Stroke::new(2.4_f32, color));
            painter.line_segment([p_tip, p_nib], Stroke::new(1.2_f32, color));
            painter.circle_filled(p_nib, 1.2_f32, color);
        }

        ToolType::Watercolor => {
            // Fluid teardrop
            let top = pos2(cx, cy - r * 0.8_f32);
            let b_left = pos2(cx - r * 0.65_f32, cy + r * 0.4_f32);
            let b_right = pos2(cx + r * 0.65_f32, cy + r * 0.4_f32);
            let b_center = pos2(cx, cy + r * 0.8_f32);

            let points = vec![top, b_right, b_center, b_left];
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
            painter.circle_filled(pos2(cx - r * 0.2_f32, cy + r * 0.1_f32), 1.2_f32, color);
        }

        ToolType::Chalk => {
            // Textured chalk rectangle with stippling
            let chalk_rect = Rect::from_center_size(rect.center(), Vec2::new(r * 1.5_f32, r * 1.0_f32));
            painter.rect(chalk_rect, 2.0_f32, fill, stroke);
            painter.circle_filled(pos2(cx - r * 0.3_f32, cy), 1.0_f32, color);
            painter.circle_filled(pos2(cx + r * 0.3_f32, cy), 1.0_f32, color);
        }

        ToolType::Spray => {
            // Spray nozzle cone + particle dots
            let nozzle = pos2(cx - r * 0.6_f32, cy + r * 0.6_f32);
            painter.line_segment([nozzle, pos2(cx - r * 0.2_f32, cy + r * 0.2_f32)], Stroke::new(2.5_f32, color));

            // Radial spray dots
            painter.circle_filled(pos2(cx + r * 0.2_f32, cy - r * 0.1_f32), 1.2_f32, color);
            painter.circle_filled(pos2(cx + r * 0.6_f32, cy - r * 0.4_f32), 1.4_f32, color);
            painter.circle_filled(pos2(cx + r * 0.1_f32, cy - r * 0.6_f32), 1.1_f32, color);
            painter.circle_filled(pos2(cx + r * 0.7_f32, cy + r * 0.1_f32), 1.3_f32, color);
            painter.circle_filled(pos2(cx + r * 0.45_f32, cy - r * 0.7_f32), 1.0_f32, color);
        }

        ToolType::Smudge => {
            // Three flowing wavy smudge lines
            for i in -1..=1 {
                let dy = i as f32 * (r * 0.45_f32);
                let p0 = pos2(cx - r * 0.7_f32, cy + dy + r * 0.2_f32);
                let p1 = pos2(cx - r * 0.2_f32, cy + dy - r * 0.2_f32);
                let p2 = pos2(cx + r * 0.3_f32, cy + dy + r * 0.2_f32);
                let p3 = pos2(cx + r * 0.7_f32, cy + dy - r * 0.2_f32);
                painter.line_segment([p0, p1], Stroke::new(1.3_f32, color));
                painter.line_segment([p1, p2], Stroke::new(1.3_f32, color));
                painter.line_segment([p2, p3], Stroke::new(1.3_f32, color));
            }
        }

        ToolType::Gradient => {
            // Gradient swatch card with diagonal partition
            let g_rect = Rect::from_center_size(rect.center(), Vec2::new(r * 1.5_f32, r * 1.3_f32));
            painter.rect(g_rect, 2.0_f32, Color32::TRANSPARENT, stroke);
            let p_half = vec![
                g_rect.left_top(),
                g_rect.right_top(),
                g_rect.left_bottom(),
            ];
            painter.add(egui::Shape::convex_polygon(p_half, color, Stroke::NONE));
        }

        ToolType::Wand => {
            // Magic wand: stick with 4-point glowing star
            let handle_start = pos2(cx - r * 0.7_f32, cy + r * 0.7_f32);
            let star_center = pos2(cx + r * 0.3_f32, cy - r * 0.3_f32);
            painter.line_segment([handle_start, star_center], Stroke::new(2.0_f32, color));

            let sr = r * 0.55_f32;
            let star_pts = vec![
                pos2(star_center.x, star_center.y - sr),
                pos2(star_center.x + sr * 0.35_f32, star_center.y - sr * 0.35_f32),
                pos2(star_center.x + sr, star_center.y),
                pos2(star_center.x + sr * 0.35_f32, star_center.y + sr * 0.35_f32),
                pos2(star_center.x, star_center.y + sr),
                pos2(star_center.x - sr * 0.35_f32, star_center.y + sr * 0.35_f32),
                pos2(star_center.x - sr, star_center.y),
                pos2(star_center.x - sr * 0.35_f32, star_center.y - sr * 0.35_f32),
            ];
            painter.add(egui::Shape::convex_polygon(star_pts, color, Stroke::NONE));
        }

        ToolType::Eraser => {
            // Angled beveled eraser block
            let p0 = pos2(cx - r * 0.7_f32, cy - r * 0.2_f32);
            let p1 = pos2(cx + r * 0.2_f32, cy - r * 0.8_f32);
            let p2 = pos2(cx + r * 0.8_f32, cy + r * 0.2_f32);
            let p3 = pos2(cx - r * 0.1_f32, cy + r * 0.8_f32);
            let eraser_pts = vec![p0, p1, p2, p3];
            painter.add(egui::Shape::convex_polygon(eraser_pts, fill, stroke));
            let mid_top = pos2(cx - r * 0.25_f32, cy - r * 0.5_f32);
            let mid_bot = pos2(cx + r * 0.35_f32, cy + r * 0.5_f32);
            painter.line_segment([mid_top, mid_bot], stroke);
        }

        ToolType::Fill => {
            // Paint bucket tipping paint
            let p0 = pos2(cx - r * 0.6_f32, cy - r * 0.5_f32);
            let p1 = pos2(cx + r * 0.2_f32, cy - r * 0.8_f32);
            let p2 = pos2(cx + r * 0.7_f32, cy + r * 0.2_f32);
            let p3 = pos2(cx - r * 0.1_f32, cy + r * 0.5_f32);
            painter.add(egui::Shape::convex_polygon(vec![p0, p1, p2, p3], fill, stroke));
            let drip_center = pos2(cx + r * 0.6_f32, cy + r * 0.65_f32);
            painter.circle_filled(drip_center, 1.8_f32, color);
        }

        ToolType::Line => {
            // Diagonal line with endpoint rings
            let p0 = pos2(cx - r * 0.7_f32, cy + r * 0.7_f32);
            let p1 = pos2(cx + r * 0.7_f32, cy - r * 0.7_f32);
            painter.line_segment([p0, p1], Stroke::new(1.8_f32, color));
            painter.circle_filled(p0, 2.0_f32, color);
            painter.circle_filled(p1, 2.0_f32, color);
        }

        ToolType::Rect => {
            // Rectangle outline
            let box_rect = Rect::from_center_size(rect.center(), Vec2::new(r * 1.5_f32, r * 1.2_f32));
            painter.rect(box_rect, 1.5_f32, fill, stroke);
        }

        ToolType::Ellipse => {
            // Ellipse circle
            painter.circle(rect.center(), r * 0.75_f32, fill, stroke);
        }

        ToolType::Polygon => {
            // Pentagon
            let mut poly_pts = Vec::new();
            for i in 0..5 {
                let angle = -std::f32::consts::FRAC_PI_2 + (i as f32) * (std::f32::consts::TAU / 5.0_f32);
                poly_pts.push(pos2(cx + angle.cos() * r * 0.8_f32, cy + angle.sin() * r * 0.8_f32));
            }
            painter.add(egui::Shape::convex_polygon(poly_pts, fill, stroke));
        }

        ToolType::Marquee => {
            // Dashed selection rectangle
            let m_rect = Rect::from_center_size(rect.center(), Vec2::new(r * 1.5_f32, r * 1.2_f32));
            let dash_stroke = Stroke::new(1.3_f32, color);
            painter.line_segment([m_rect.left_top(), pos2(cx - r * 0.1_f32, m_rect.top())], dash_stroke);
            painter.line_segment([pos2(cx + r * 0.2_f32, m_rect.top()), m_rect.right_top()], dash_stroke);
            painter.line_segment([m_rect.right_top(), pos2(m_rect.right(), cy)], dash_stroke);
            painter.line_segment([m_rect.left_bottom(), pos2(cx, m_rect.bottom())], dash_stroke);
            painter.line_segment([m_rect.left_top(), pos2(m_rect.left(), cy)], dash_stroke);
        }

        ToolType::Lasso => {
            // Freeform lasso loop
            let p0 = pos2(cx - r * 0.5_f32, cy + r * 0.5_f32);
            let p1 = pos2(cx - r * 0.7_f32, cy - r * 0.3_f32);
            let p2 = pos2(cx + r * 0.1_f32, cy - r * 0.7_f32);
            let p3 = pos2(cx + r * 0.7_f32, cy + r * 0.1_f32);
            let p4 = pos2(cx + r * 0.2_f32, cy + r * 0.6_f32);
            painter.line_segment([p0, p1], stroke);
            painter.line_segment([p1, p2], stroke);
            painter.line_segment([p2, p3], stroke);
            painter.line_segment([p3, p4], stroke);
            painter.line_segment([p4, p0], stroke);
        }

        ToolType::Text => {
            // Capital "T" typography icon
            let top_left = pos2(cx - r * 0.6_f32, cy - r * 0.6_f32);
            let top_right = pos2(cx + r * 0.6_f32, cy - r * 0.6_f32);
            let top_mid = pos2(cx, cy - r * 0.6_f32);
            let bot_mid = pos2(cx, cy + r * 0.7_f32);
            painter.line_segment([top_left, top_right], Stroke::new(2.2_f32, color));
            painter.line_segment([top_mid, bot_mid], Stroke::new(2.2_f32, color));
        }

        ToolType::Move => {
            // 4-way arrow crosshair
            painter.line_segment([pos2(cx - r * 0.75_f32, cy), pos2(cx + r * 0.75_f32, cy)], stroke);
            painter.line_segment([pos2(cx, cy - r * 0.75_f32), pos2(cx, cy + r * 0.75_f32)], stroke);
            painter.line_segment([pos2(cx - r * 0.75_f32, cy), pos2(cx - r * 0.45_f32, cy - r * 0.3_f32)], stroke);
            painter.line_segment([pos2(cx - r * 0.75_f32, cy), pos2(cx - r * 0.45_f32, cy + r * 0.3_f32)], stroke);
            painter.line_segment([pos2(cx + r * 0.75_f32, cy), pos2(cx + r * 0.45_f32, cy - r * 0.3_f32)], stroke);
            painter.line_segment([pos2(cx + r * 0.75_f32, cy), pos2(cx + r * 0.45_f32, cy + r * 0.3_f32)], stroke);
            painter.line_segment([pos2(cx, cy - r * 0.75_f32), pos2(cx - r * 0.3_f32, cy - r * 0.45_f32)], stroke);
            painter.line_segment([pos2(cx, cy - r * 0.75_f32), pos2(cx + r * 0.3_f32, cy - r * 0.45_f32)], stroke);
            painter.line_segment([pos2(cx, cy + r * 0.75_f32), pos2(cx - r * 0.3_f32, cy + r * 0.45_f32)], stroke);
            painter.line_segment([pos2(cx, cy + r * 0.75_f32), pos2(cx + r * 0.3_f32, cy + r * 0.45_f32)], stroke);
        }

        ToolType::Transform => {
            // Transform bounding box with corner anchor nodes
            let box_rect = Rect::from_center_size(rect.center(), Vec2::new(r * 1.3_f32, r * 1.3_f32));
            painter.rect(box_rect, 1.0_f32, fill, stroke);
            let hs = 3.0_f32;
            painter.rect_filled(Rect::from_center_size(box_rect.left_top(), Vec2::splat(hs)), 0.5_f32, color);
            painter.rect_filled(Rect::from_center_size(box_rect.right_top(), Vec2::splat(hs)), 0.5_f32, color);
            painter.rect_filled(Rect::from_center_size(box_rect.right_bottom(), Vec2::splat(hs)), 0.5_f32, color);
            painter.rect_filled(Rect::from_center_size(box_rect.left_bottom(), Vec2::splat(hs)), 0.5_f32, color);
        }

        ToolType::Crop => {
            // Framing crop corners
            let c_rect = Rect::from_center_size(rect.center(), Vec2::new(r * 1.4_f32, r * 1.4_f32));
            let cl = r * 0.5_f32;
            painter.line_segment([c_rect.left_top(), pos2(c_rect.left() + cl, c_rect.top())], stroke);
            painter.line_segment([c_rect.left_top(), pos2(c_rect.left(), c_rect.top() + cl)], stroke);
            painter.line_segment([c_rect.right_bottom(), pos2(c_rect.right() - cl, c_rect.bottom())], stroke);
            painter.line_segment([c_rect.right_bottom(), pos2(c_rect.right(), c_rect.bottom() - cl)], stroke);
        }

        ToolType::Eyedropper => {
            // Precision pipette
            let p_top = pos2(cx - r * 0.65_f32, cy - r * 0.65_f32);
            let p_bulb = pos2(cx - r * 0.35_f32, cy - r * 0.35_f32);
            let p_tip = pos2(cx + r * 0.65_f32, cy + r * 0.65_f32);
            painter.line_segment([p_bulb, p_tip], Stroke::new(2.0_f32, color));
            painter.circle_filled(p_top, 2.5_f32, color);
            painter.circle_filled(p_tip, 1.2_f32, color);
        }

        ToolType::SelectionBrush => {
            // Brush with dashed selection ring
            let p1 = pos2(cx - r * 0.6_f32, cy + r * 0.6_f32);
            let p2 = pos2(cx + r * 0.5_f32, cy - r * 0.5_f32);
            painter.line_segment([p1, p2], Stroke::new(2.5_f32, color));
            let ring_c = pos2(cx - r * 0.45_f32, cy + r * 0.45_f32);
            painter.circle_stroke(ring_c, 3.5_f32, Stroke::new(1.0_f32, color));
        }

        ToolType::SelectionEraser => {
            // Angled eraser block with dashed ring
            let e_rect = Rect::from_center_size(pos2(cx, cy), Vec2::new(r * 1.3_f32, r * 0.8_f32));
            painter.rect_stroke(e_rect, 2.0_f32, stroke);
            painter.line_segment([pos2(e_rect.left() + 4.0, e_rect.top()), pos2(e_rect.left() + 4.0, e_rect.bottom())], stroke);
        }
    }
}
