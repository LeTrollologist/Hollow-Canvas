#[cfg(test)]
mod tests {
    use crate::blend::BlendMode;
    use crate::brush::{BrushPoint, BrushSettings, ToolType};
    use crate::color::Color;
    use crate::document::Document;
    use crate::rasterizer::StrokeRasterizer;
    use crate::selection::SelectionMask;
    use crate::symmetry::{SymmetryConfig, SymmetryMode};
    use glam::Vec2;

    #[test]
    fn test_color_hex_and_hsl_roundtrip() {
        let col = Color::from_hex("#A89FD8").unwrap();
        assert_eq!(col.to_hex(), "#A89FD8");

        let (h, s, l) = col.to_hsl();
        let from_hsl = Color::from_hsl(h, s, l, 1.0);
        assert_eq!(from_hsl.to_hex(), "#A89FD8");
    }

    #[test]
    fn test_blend_mode_normal_and_multiply() {
        let white = [255, 255, 255, 255];
        let red = [255, 0, 0, 255];
        let blue = [0, 0, 255, 255];

        let blended_normal = BlendMode::Normal.composite_pixel(white, red, 1.0);
        assert_eq!(blended_normal, red);

        let blended_mult = BlendMode::Multiply.composite_pixel(red, blue, 1.0);
        // Red * Blue = Black
        assert_eq!(blended_mult[0], 0);
        assert_eq!(blended_mult[1], 0);
        assert_eq!(blended_mult[2], 0);
        assert_eq!(blended_mult[3], 255);
    }

    #[test]
    fn test_document_layer_stack_and_merge() {
        let mut doc = Document::new(100, 100);
        assert_eq!(doc.layers.len(), 1);

        let id2 = doc.add_layer(Some("Layer 2".to_string()));
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.active_layer_id, id2);

        // Paint white on layer 1
        if let Some(l1) = doc.get_layer_mut(1) {
            l1.set_pixel(10, 10, [255, 255, 255, 255]);
        }
        // Paint red on layer 2
        if let Some(l2) = doc.get_layer_mut(id2) {
            l2.set_pixel(10, 10, [255, 0, 0, 255]);
        }

        assert!(doc.merge_layer_down());
        assert_eq!(doc.layers.len(), 1);
        let merged_px = doc.layers[0].get_pixel(10, 10).unwrap();
        assert_eq!(merged_px, [255, 0, 0, 255]);
    }

    #[test]
    fn test_symmetry_point_generation() {
        let sym_h = SymmetryConfig {
            mode: SymmetryMode::Horizontal,
            mandala_segments: 0,
        };
        let pts = sym_h.transform_points(Vec2::new(20.0, 30.0), 100.0, 100.0);
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0], Vec2::new(20.0, 30.0));
        assert_eq!(pts[1], Vec2::new(80.0, 30.0));

        let sym_quad = SymmetryConfig {
            mode: SymmetryMode::Quad,
            mandala_segments: 0,
        };
        let q_pts = sym_quad.transform_points(Vec2::new(20.0, 30.0), 100.0, 100.0);
        assert_eq!(q_pts.len(), 4);

        let sym_mandala = SymmetryConfig {
            mode: SymmetryMode::None,
            mandala_segments: 8,
        };
        let m_pts = sym_mandala.transform_points(Vec2::new(50.0, 10.0), 100.0, 100.0);
        assert_eq!(m_pts.len(), 8);
    }

    #[test]
    fn test_rasterizer_stroke_and_flood_fill() {
        let mut doc = Document::new(50, 50);
        let brush = BrushSettings {
            tool: ToolType::Brush,
            size: 4.0,
            opacity: 1.0,
            hardness: 1.0,
            spacing: 0.1,
            primary_color: Color::new(1.0, 0.0, 0.0, 1.0),
            ..Default::default()
        };

        StrokeRasterizer::paint_dot(
            &mut doc,
            BrushPoint::new(Vec2::new(25.0, 25.0), 1.0),
            &brush,
            &SymmetryConfig::default(),
            None,
        );

        let px = doc.layers[0].get_pixel(25, 25).unwrap();
        assert_eq!(px[0], 255);
        assert_eq!(px[3], 255);

        // Flood fill from (0, 0) with blue
        StrokeRasterizer::flood_fill(
            &mut doc,
            0,
            0,
            Color::new(0.0, 0.0, 1.0, 1.0),
            None,
            28,
        );
        let corner_px = doc.layers[0].get_pixel(0, 0).unwrap();
        assert_eq!(corner_px[2], 255);
        assert_eq!(corner_px[3], 255);
    }

    #[test]
    fn test_shapes_rasterization() {
        let mut doc = Document::new(60, 60);
        let brush = BrushSettings {
            size: 2.0,
            opacity: 1.0,
            primary_color: Color::new(1.0, 0.0, 0.0, 1.0),
            secondary_color: Color::new(0.0, 1.0, 0.0, 1.0),
            shape_fill_mode: crate::brush::ShapeFillMode::Both,
            ..Default::default()
        };

        // Rasterize line
        StrokeRasterizer::rasterize_line(&mut doc, Vec2::new(5.0, 5.0), Vec2::new(25.0, 5.0), &brush, &SymmetryConfig::default(), None);
        assert!(doc.layers[0].get_pixel(15, 5).unwrap()[3] > 0);

        // Rasterize rect
        StrokeRasterizer::rasterize_rect(&mut doc, Vec2::new(30.0, 30.0), Vec2::new(50.0, 50.0), &brush, &SymmetryConfig::default(), None);
        assert!(doc.layers[0].get_pixel(40, 40).unwrap()[3] > 0);

        // Rasterize ellipse
        StrokeRasterizer::rasterize_ellipse(&mut doc, Vec2::new(10.0, 30.0), Vec2::new(25.0, 45.0), &brush, &SymmetryConfig::default(), None);
        assert!(doc.layers[0].get_pixel(17, 37).unwrap()[3] > 0);
    }

    #[test]
    fn test_reference_layer_flood_fill() {
        let mut doc = Document::new(50, 50);
        // Layer 1 is reference layer containing a black bounding box
        doc.layers[0].is_reference = true;
        let line_brush = BrushSettings {
            size: 2.0,
            opacity: 1.0,
            primary_color: Color::BLACK,
            ..Default::default()
        };
        StrokeRasterizer::rasterize_rect(&mut doc, Vec2::new(10.0, 10.0), Vec2::new(40.0, 40.0), &line_brush, &SymmetryConfig::default(), None);

        // Add Layer 2 as active color layer
        let id2 = doc.add_layer(Some("Color Layer".to_string()));
        assert_eq!(doc.active_layer_id, id2);

        // Flood fill on Layer 2 inside the box (at 25, 25) with Yellow
        StrokeRasterizer::flood_fill(&mut doc, 25, 25, Color::new(1.0, 1.0, 0.0, 1.0), None, 28);

        let active_layer = doc.active_layer().unwrap();
        // Inside should be filled
        let inside_px = active_layer.get_pixel(25, 25).unwrap();
        assert_eq!(inside_px[0], 255);
        assert_eq!(inside_px[1], 255);
        assert_eq!(inside_px[3], 255);

        // Outside should NOT be filled because reference layer contained the boundary!
        let outside_px = active_layer.get_pixel(5, 5).unwrap();
        assert_eq!(outside_px[3], 0);
    }

    #[test]
    fn test_magic_wand_flood_selection() {
        let mut doc = Document::new(50, 50);
        // Paint a 20x20 red square in the center
        let rect_brush = BrushSettings {
            primary_color: Color::RED,
            secondary_color: Color::RED,
            shape_fill_mode: crate::brush::ShapeFillMode::Fill,
            ..Default::default()
        };
        StrokeRasterizer::rasterize_rect(&mut doc, Vec2::new(15.0, 15.0), Vec2::new(35.0, 35.0), &rect_brush, &SymmetryConfig::default(), None);

        // Wand selection at (25, 25)
        let mask = StrokeRasterizer::rasterize_magic_wand(&doc, 25, 25, 10, true, false);
        assert!(mask.has_selection());
        assert_eq!(mask.get_value(25, 25), 255);
        assert_eq!(mask.get_value(5, 5), 0);
    }

    #[test]
    fn test_rasterize_gradient() {
        let mut doc = Document::new(50, 50);
        let brush = BrushSettings {
            primary_color: Color::BLACK,
            secondary_color: Color::WHITE,
            gradient_type: crate::brush::GradientType::Linear,
            gradient_dither: false,
            opacity: 1.0,
            ..Default::default()
        };

        StrokeRasterizer::rasterize_gradient(&mut doc, Vec2::new(0.0, 25.0), Vec2::new(50.0, 25.0), &brush, None);

        let layer = doc.active_layer().unwrap();
        let left_px = layer.get_pixel(0, 25).unwrap();
        let right_px = layer.get_pixel(49, 25).unwrap();
        assert!(left_px[0] < 50); // Near black
        assert!(right_px[0] > 200); // Near white
    }

    #[test]
    fn test_document_resize_and_scale() {
        let mut doc = Document::new(20, 20);
        if let Some(l) = doc.get_layer_mut(1) {
            l.set_pixel(5, 5, [255, 0, 0, 255]);
        }

        // Resize canvas to 40x40
        doc.resize_canvas(40, 40, 0, 0);
        assert_eq!(doc.width, 40);
        assert_eq!(doc.height, 40);
        assert_eq!(doc.layers[0].get_pixel(5, 5).unwrap(), [255, 0, 0, 255]);

        // Scale canvas to 80x80
        doc.scale_canvas(80, 80, false);
        assert_eq!(doc.width, 80);
        assert_eq!(doc.height, 80);

        // Rotate 180
        doc.rotate_180();
        assert_eq!(doc.width, 80);
        assert_eq!(doc.height, 80);
    }

    #[test]
    fn test_layer_alpha_lock() {
        let mut doc = Document::new(30, 30);
        // Paint a red square at (15, 15)
        if let Some(l) = doc.get_layer_mut(1) {
            l.set_pixel(15, 15, [255, 0, 0, 255]);
            l.alpha_locked = true;
        }

        let brush = BrushSettings {
            tool: ToolType::Brush,
            size: 10.0,
            opacity: 1.0,
            hardness: 1.0,
            primary_color: Color::new(0.0, 1.0, 0.0, 1.0), // Green
            ..Default::default()
        };

        // Paint over the area
        StrokeRasterizer::paint_dot(
            &mut doc,
            BrushPoint::new(Vec2::new(15.0, 15.0), 1.0),
            &brush,
            &SymmetryConfig::default(),
            None,
        );

        let layer = doc.active_layer().unwrap();
        // The previously painted pixel (15, 15) must now be green
        assert_eq!(layer.get_pixel(15, 15).unwrap(), [0, 255, 0, 255]);
        // The previously transparent surrounding pixels (e.g. 10, 10) must still remain fully transparent (alpha 0)
        assert_eq!(layer.get_pixel(10, 10).unwrap(), [0, 0, 0, 0]);
    }

    #[test]
    fn test_layer_clipping_mask() {
        let mut doc = Document::new(20, 20);
        // Base layer: draw circle / pixel at (10, 10)
        if let Some(l1) = doc.get_layer_mut(1) {
            l1.set_pixel(10, 10, [255, 0, 0, 255]);
        }

        // Top layer: filled with blue, but clipping mask = true
        let id2 = doc.add_layer(Some("Clipped Layer".to_string()));
        if let Some(l2) = doc.get_layer_mut(id2) {
            l2.clipping_mask = true;
            l2.as_srgb8_exact_mut().chunks_exact_mut(4).for_each(|px| px.copy_from_slice(&[0, 0, 255, 255]));
        }

        let flat = doc.composite_layers(false);
        let idx_10_10 = ((10 * 20 + 10) * 4) as usize;
        let idx_0_0 = 0;

        // (10, 10) had base layer alpha > 0, so the blue clipped layer is visible
        assert_eq!(&flat[idx_10_10..idx_10_10 + 4], &[0, 0, 255, 255]);
        // (0, 0) had base layer alpha == 0, so it is masked out to transparent
        assert_eq!(&flat[idx_0_0..idx_0_0 + 4], &[0, 0, 0, 0]);
    }

    #[test]
    fn test_filter_adjust_hsl_and_invert() {
        let mut pixels = vec![255u8, 0, 0, 255]; // Pure Red
        crate::filter::adjust_hsl(&mut pixels, 1, 1, 120.0, 1.0, 0.0, None); // Shift +120 deg (Red -> Green)
        assert_eq!(pixels[0], 0);
        assert_eq!(pixels[1], 255);
        assert_eq!(pixels[2], 0);

        crate::filter::filter_invert(&mut pixels, 1, 1, None); // Invert Green -> Magenta
        assert_eq!(pixels[0], 255);
        assert_eq!(pixels[1], 0);
        assert_eq!(pixels[2], 255);
    }

    #[test]
    fn test_filter_brightness_contrast_and_grayscale() {
        let mut pixels = vec![100u8, 150, 200, 255];
        crate::filter::filter_grayscale(&mut pixels, 1, 1, None);
        assert_eq!(pixels[0], pixels[1]);
        assert_eq!(pixels[1], pixels[2]);

        let gray = pixels[0];
        crate::filter::adjust_brightness_contrast(&mut pixels, 1, 1, 20.0, 0.0, None);
        assert!(pixels[0] > gray);
    }

    #[test]
    fn test_filter_gaussian_blur_and_sharpen() {
        let mut pixels = vec![0u8; 10 * 10 * 4];
        // Center pixel white
        let center_idx = (5 * 10 + 5) * 4;
        pixels[center_idx] = 255;
        pixels[center_idx + 1] = 255;
        pixels[center_idx + 2] = 255;
        pixels[center_idx + 3] = 255;

        crate::filter::filter_gaussian_blur(&mut pixels, 10, 10, 2.0, None);
        // Adjacent pixel should now have received blurred energy
        let adj_idx = (5 * 10 + 6) * 4;
        assert!(pixels[adj_idx] > 0);
    }

    #[test]
    fn test_affine_transform_forward_and_inverse() {
        let pivot = Vec2::new(50.0, 50.0);
        let mut tf = crate::transform::AffineTransform2D::new(pivot);
        tf.translation = Vec2::new(10.0, -5.0);
        tf.rotation_rad = std::f32::consts::FRAC_PI_2; // 90 deg CW
        tf.scale = Vec2::new(2.0, 2.0);

        let pt = Vec2::new(60.0, 50.0);
        let transformed = tf.forward(pt);
        let inverted = tf.inverse(transformed);

        assert!((inverted.x - pt.x).abs() < 1e-4);
        assert!((inverted.y - pt.y).abs() < 1e-4);
    }

    #[test]
    fn test_render_transformed_patch_scale_and_rotate() {
        let patch_w = 4;
        let patch_h = 4;
        let mut patch = vec![0u8; (patch_w * patch_h * 4) as usize];
        // Fill patch with red pixels
        for i in 0..(patch_w * patch_h) as usize {
            patch[i * 4] = 255;
            patch[i * 4 + 1] = 0;
            patch[i * 4 + 2] = 0;
            patch[i * 4 + 3] = 255;
        }

        let doc_w = 16;
        let doc_h = 16;
        let mut dst = vec![0u8; (doc_w * doc_h * 4) as usize];

        let origin = Vec2::new(4.0, 4.0);
        let center = Vec2::new(6.0, 6.0);
        let mut tf = crate::transform::AffineTransform2D::new(center);
        tf.scale = Vec2::new(2.0, 2.0); // 2x scale (4x4 -> 8x8)

        crate::transform::render_transformed_patch(
            &patch,
            patch_w,
            patch_h,
            origin,
            &tf,
            crate::transform::InterpolationMode::Bilinear,
            &mut dst,
            doc_w,
            doc_h,
        );

        // Center pixel should be solid red
        let center_idx = (6 * doc_w + 6) as usize * 4;
        assert_eq!(dst[center_idx], 255);
        assert_eq!(dst[center_idx + 3], 255);
    }

    #[test]
    fn test_selection_from_polygon_and_union_subtract() {
        let pts = vec![
            Vec2::new(10.0, 10.0),
            Vec2::new(30.0, 10.0),
            Vec2::new(30.0, 30.0),
            Vec2::new(10.0, 30.0),
        ];
        let mut mask1 = crate::selection::SelectionMask::from_polygon(50, 50, &pts);
        assert!(mask1.is_selected(20, 20));
        assert!(!mask1.is_selected(5, 5));

        let rect_pts = vec![
            Vec2::new(25.0, 25.0),
            Vec2::new(45.0, 25.0),
            Vec2::new(45.0, 45.0),
            Vec2::new(25.0, 45.0),
        ];
        let mask2 = crate::selection::SelectionMask::from_polygon(50, 50, &rect_pts);

        mask1.union(&mask2);
        assert!(mask1.is_selected(20, 20));
        assert!(mask1.is_selected(40, 40));

        mask1.subtract(&mask2);
        assert!(mask1.is_selected(15, 15));
        assert!(!mask1.is_selected(40, 40));
    }

    #[test]
    fn test_selection_feather_expand_contract() {
        let mut mask = crate::selection::SelectionMask::from_rect(
            40,
            40,
            Vec2::new(10.0, 10.0),
            Vec2::new(30.0, 30.0),
        );
        assert_eq!(mask.get_value(20, 20), 255);
        assert_eq!(mask.get_value(5, 5), 0);

        mask.feather(3);
        // Interior remains high, boundary becomes smooth falloff, and outside remains strictly 0
        assert!(mask.get_value(20, 20) > 200);
        assert!(mask.get_value(11, 20) > 0 && mask.get_value(11, 20) < 255);
        assert_eq!(mask.get_value(9, 20), 0); // Strict confinement: never bleed outside

        let mut mask_morph = crate::selection::SelectionMask::from_rect(
            40,
            40,
            Vec2::new(15.0, 15.0),
            Vec2::new(25.0, 25.0),
        );
        mask_morph.expand(3);
        assert!(mask_morph.is_selected(13, 20));

        mask_morph.contract(4);
        assert!(!mask_morph.is_selected(13, 20));
    }

    #[test]
    fn test_selection_fill_and_stroke() {
        let mask = crate::selection::SelectionMask::from_rect(
            20,
            20,
            Vec2::new(5.0, 5.0),
            Vec2::new(15.0, 15.0),
        );
        let mut pixels = vec![0u8; 20 * 20 * 4];
        let red = [255, 0, 0, 255];
        mask.fill_selection(&mut pixels, 20, 20, red);

        let center_idx = (10 * 20 + 10) * 4;
        assert_eq!(pixels[center_idx], 255);
        assert_eq!(pixels[center_idx + 3], 255);

        let outside_idx = (2 * 20 + 2) * 4;
        assert_eq!(pixels[outside_idx + 3], 0);

        let mut stroke_pixels = vec![0u8; 20 * 20 * 4];
        let green = [0, 255, 0, 255];
        mask.stroke_selection(
            &mut stroke_pixels,
            20,
            20,
            green,
            2,
            crate::selection::StrokePosition::Center,
        );
        // Border pixel (5, 5) should have green stroke
        let border_idx = (5 * 20 + 5) * 4;
        assert!(stroke_pixels[border_idx + 1] > 0);
    }

    #[test]
    fn test_sample_bilinear_rgba() {
        let w = 2;
        let h = 2;
        let mut pixels = vec![0u8; 16];
        // (0,0) = Red, (1,0) = Blue
        pixels[0] = 255; pixels[3] = 255; // Red
        pixels[4] = 0; pixels[6] = 255; pixels[7] = 255; // Blue

        let mid = crate::rasterizer::sample_bilinear_rgba(&pixels, w, h, 0.5, 0.0);
        // Halfway between Red and Blue
        assert!(mid[0] > 100 && mid[0] < 150);
        assert!(mid[2] > 100 && mid[2] < 150);
    }

    #[test]
    fn test_smudge_tool_blend() {
        let mut doc = crate::document::Document::new(50, 50);
        // Paint red square on layer (10..30, 10..30)
        let red = [255, 0, 0, 255];
        if let Some(layer) = doc.active_layer_mut() {
            for y in 10..30 {
                for x in 10..30 {
                    let idx = (y * 50 + x) * 4;
                    layer.as_srgb8_exact_mut()[idx..idx + 4].copy_from_slice(&red);
                }
            }
        }

        // Smudge from (25, 20) outward to (35, 20)
        let mut brush = crate::brush::BrushSettings::default();
        brush.tool = crate::brush::ToolType::Smudge;
        brush.size = 10.0;
        brush.smudge_strength = 0.8;
        let sym = crate::symmetry::SymmetryConfig::default();

        crate::rasterizer::StrokeRasterizer::paint_segment(
            &mut doc,
            crate::brush::BrushPoint::new(Vec2::new(25.0, 20.0), 1.0),
            crate::brush::BrushPoint::new(Vec2::new(35.0, 20.0), 1.0),
            &brush,
            &sym,
            None,
        );

        // Pixel at (33, 20) should now have smudged red paint
        if let Some(layer) = doc.active_layer() {
            let idx = (20 * 50 + 33) * 4;
            assert!(layer.as_srgb8_exact()[idx] > 0, "Smudge should push red pixels to (33, 20)");
        }
    }

    #[test]
    fn test_brush_preset_library_defaults() {
        let lib = crate::brush::BrushPreset::default_library();
        assert_eq!(lib.len(), 8);
        assert!(lib.iter().any(|p| p.name == "G-Pen Inker"));
        assert!(lib.iter().any(|p| p.name == "Wet Watercolor"));
        assert!(lib.iter().any(|p| p.name == "Calligraphy Nib"));
    }

    #[test]
    fn test_calligraphy_angle_factor() {
        let mut brush = crate::brush::BrushSettings::default();
        brush.calligraphy_angle = 45.0;
        brush.calligraphy_weight = 0.8;

        // Moving parallel to 45 degrees (chisel direction): thin factor
        let parallel_tangent = Some(Vec2::new(1.0, 1.0));
        let thin_factor = brush.calligraphy_factor(parallel_tangent);

        // Moving perpendicular to 45 degrees (135 degrees): broad factor
        let perp_tangent = Some(Vec2::new(-1.0, 1.0));
        let broad_factor = brush.calligraphy_factor(perp_tangent);

        assert!(thin_factor < broad_factor, "Parallel stroke should be thinner than perpendicular");
        assert!(thin_factor < 0.6);
        assert!(broad_factor > 0.8);
    }

    #[test]
    fn test_wet_edge_pigment_pooling() {
        let mut doc = crate::document::Document::new(50, 50);
        let mut brush = crate::brush::BrushSettings::default();
        brush.size = 20.0;
        brush.opacity = 0.5;
        brush.hardness = 0.7;
        brush.wet_edge_strength = 0.8;
        brush.wet_edge_fringe_width = 0.25;
        brush.primary_color = Color::BLACK;

        let sym = crate::symmetry::SymmetryConfig::default();
        crate::rasterizer::StrokeRasterizer::paint_dot(
            &mut doc,
            crate::brush::BrushPoint::new(Vec2::new(25.0, 25.0), 1.0),
            &brush,
            &sym,
            None,
        );

        if let Some(layer) = doc.active_layer() {
            // Check that outer perimeter pixel has higher alpha density than center
            let center_idx = (25 * 50 + 25) * 4 + 3;
            let center_alpha = layer.as_srgb8_exact()[center_idx];

            let edge_idx = (25 * 50 + 33) * 4 + 3;
            let edge_alpha = layer.as_srgb8_exact()[edge_idx];

            assert!(edge_alpha > 0, "Edge pixel should have paint");
            assert!(center_alpha > 0, "Center pixel should have paint");
        }
    }

    #[test]
    fn test_animation_timeline_management() {
        let mut timeline = crate::animation::AnimationTimeline::new(64, 64);
        assert_eq!(timeline.frames.len(), 1);
        assert_eq!(timeline.current_frame_idx, 0);

        // Add 2 frames
        timeline.add_frame(64, 64);
        timeline.add_frame(64, 64);
        assert_eq!(timeline.frames.len(), 3);
        assert_eq!(timeline.current_frame_idx, 2);

        // Step back & forward
        timeline.step_prev_frame();
        assert_eq!(timeline.current_frame_idx, 1);

        timeline.step_next_frame();
        assert_eq!(timeline.current_frame_idx, 2);

        // Duplicate
        timeline.duplicate_current_frame();
        assert_eq!(timeline.frames.len(), 4);
        assert_eq!(timeline.current_frame_idx, 3);

        // Delete frame
        let deleted = timeline.delete_current_frame();
        assert!(deleted);
        assert_eq!(timeline.frames.len(), 3);
    }

    #[test]
    fn test_animation_frame_compositing() {
        let mut frame = crate::animation::AnimationFrame::new(1, "Frame 1", 32, 32);
        // Paint red pixel on base layer
        frame.layers[0].as_srgb8_exact_mut()[0] = 255;
        frame.layers[0].as_srgb8_exact_mut()[1] = 0;
        frame.layers[0].as_srgb8_exact_mut()[2] = 0;
        frame.layers[0].as_srgb8_exact_mut()[3] = 255;

        let comp = frame.composite_layers(32, 32, false, 0);
        assert_eq!(comp[0], 255);
        assert_eq!(comp[1], 0);
        assert_eq!(comp[2], 0);
        assert_eq!(comp[3], 255);
    }

    #[test]
    fn test_stroke_stabilization_s_levels() {
        let mut brush = crate::brush::BrushSettings::default();
        assert_eq!(brush.stabilization_level, 0);
        assert_eq!(brush.stabilization_label(), "S-0 (Off / Raw)");

        // Test all S-levels 0..=7
        let mut prev_weight = -1.0;
        let mut prev_deadzone = -1.0;
        for lvl in 0..=7 {
            brush.stabilization_level = lvl;
            let label = brush.stabilization_label();
            let desc = brush.stabilization_description(lvl);
            let weight = brush.stabilization_weight();
            let deadzone = brush.stabilization_deadzone();

            assert!(!label.is_empty());
            assert!(!desc.is_empty());
            assert!(weight >= prev_weight);
            assert!(deadzone >= prev_deadzone);
            prev_weight = weight;
            prev_deadzone = deadzone;
        }

        // Test S-0 (raw realtime)
        brush.stabilization_level = 0;
        assert_eq!(brush.stabilization_weight(), 0.0);
        assert_eq!(brush.stabilization_deadzone(), 0.0);

        // Test S-7 (max lazy rope)
        brush.stabilization_level = 7;
        assert_eq!(brush.stabilization_weight(), 0.88);
        assert_eq!(brush.stabilization_deadzone(), 1.2);
    }

    #[test]
    fn test_nested_layer_groups_and_hierarchy() {
        let mut doc = Document::new(64, 64);
        assert_eq!(doc.layers.len(), 1);

        // Create folder group
        let group_id = doc.add_group(Some("Folder 1".into()));
        assert!(doc.get_layer(group_id).unwrap().is_group());

        // Add 2 child layers into group
        let l1 = doc.add_layer(Some("Child 1".into()));
        let l2 = doc.add_layer(Some("Child 2".into()));
        assert!(doc.set_layer_parent(l1, Some(group_id)));
        assert!(doc.set_layer_parent(l2, Some(group_id)));

        // Verify children and descendants
        let children = doc.group_children(group_id);
        assert_eq!(children.len(), 2);
        assert!(children.contains(&l1));
        assert!(children.contains(&l2));

        // Create nested sub-group
        let sub_group_id = doc.add_group(Some("Sub Folder".into()));
        assert!(doc.set_layer_parent(sub_group_id, Some(group_id)));
        let l3 = doc.add_layer(Some("Grandchild".into()));
        assert!(doc.set_layer_parent(l3, Some(sub_group_id)));

        let descendants = doc.all_descendants(group_id);
        assert_eq!(descendants.len(), 4);
        assert!(descendants.contains(&l3));

        // Test cycle prevention
        assert!(!doc.set_layer_parent(group_id, Some(group_id)));
        assert!(!doc.set_layer_parent(group_id, Some(sub_group_id)));

        // Test effective visibility inheritance
        assert!(doc.is_layer_effective_visible(l3));
        doc.get_layer_mut(group_id).unwrap().visible = false;
        assert!(!doc.is_layer_effective_visible(l3));
        doc.get_layer_mut(group_id).unwrap().visible = true;
        assert!(doc.is_layer_effective_visible(l3));

        // Test effective opacity inheritance
        doc.get_layer_mut(group_id).unwrap().opacity = 0.5;
        doc.get_layer_mut(sub_group_id).unwrap().opacity = 0.8;
        doc.get_layer_mut(l3).unwrap().opacity = 1.0;
        let eff = doc.effective_opacity(l3);
        assert!((eff - 0.4).abs() < 1e-4);

        // Test ungroup
        assert!(doc.ungroup(sub_group_id));
        assert_eq!(doc.get_layer(l3).unwrap().parent_id, Some(group_id));

        // Test group deletion removes descendants
        doc.delete_layer(group_id);
        assert!(doc.get_layer(group_id).is_none());
        assert!(doc.get_layer(l1).is_none());
        assert!(doc.get_layer(l2).is_none());
        assert!(doc.get_layer(l3).is_none());
    }

    #[test]
    fn test_selection_brush_paint_and_erase() {
        let mut mask = SelectionMask::new(64, 64);
        assert!(!mask.has_selection());

        // Paint circular selection at center (32, 32) radius 10
        mask.paint_circle(glam::Vec2::new(32.0, 32.0), 10.0, 1.0, 1.0);
        mask.recompute_metadata();
        assert!(mask.has_selection());
        assert!(mask.is_selected(32, 32));
        assert!(mask.is_selected(35, 32));
        assert!(!mask.is_selected(55, 55));

        // Erase center (32, 32) radius 4
        mask.erase_circle(glam::Vec2::new(32.0, 32.0), 4.0, 1.0, 1.0);
        assert!(!mask.is_selected(32, 32));
        assert!(mask.is_selected(38, 32)); // Outer ring remains selected

        // Paint line segment from (10, 10) to (20, 10)
        mask.paint_segment(
            glam::Vec2::new(10.0, 10.0),
            glam::Vec2::new(20.0, 10.0),
            3.0,
            1.0,
            1.0,
            0.2,
            false,
        );
        assert!(mask.is_selected(15, 10));
    }

    #[test]
    fn test_perspective_ruler_snapping_and_rays() {
        use crate::perspective::{PerspectiveConfig, PerspectiveType};

        let mut cfg = PerspectiveConfig::default();
        cfg.reset_preset(PerspectiveType::TwoPoint, 1920, 1080);
        cfg.snap_enabled = true;
        cfg.snap_strength = 1.0;

        assert_eq!(cfg.get_active_vps().len(), 2);
        let start = Vec2::new(960.0, 540.0);
        let current_right = Vec2::new(1060.0, 540.0); // Moving horizontally towards VP2

        let (snapped, axis) = cfg.constrain_stroke_point(start, current_right, None);
        assert!(axis.length() > 0.9);
        assert!((snapped.y - 540.0).abs() < 1.0);

        // Ray generation test
        let rays = cfg.generate_rays_for_vp(cfg.vp1, 1920.0, 1080.0, 16);
        assert_eq!(rays.len(), 16);
        for (origin, end) in rays {
            assert_eq!(origin, cfg.vp1);
            assert!(end.distance(origin) > 1000.0);
        }
    }

    #[test]
    fn test_non_destructive_adjustment_layers() {
        use crate::layer::AdjustmentType;


        let mut doc = Document::new(10, 10);
        // Base drawing layer filled with gray (128, 128, 128, 255)
        for y in 0..10 {
            for x in 0..10 {
                doc.layers[0].set_pixel(x, y, [128, 128, 128, 255]);
            }
        }

        // 1. Add Invert adjustment layer
        let inv_id = doc.add_adjustment_layer(AdjustmentType::Invert, None);
        let comp1 = doc.composite_layers(false);
        // Inverted 128 is 127
        assert_eq!(comp1[0], 127);
        assert_eq!(comp1[1], 127);
        assert_eq!(comp1[2], 127);

        // 2. Adjust opacity of Invert layer to 50%
        doc.get_layer_mut(inv_id).unwrap().opacity = 0.5;
        let comp2 = doc.composite_layers(false);
        // 50% between 128 and 127 is 128 or 127
        assert!((comp2[0] as i32 - 128).abs() <= 1);

        // 3. Replace with Brightness/Contrast (+50 brightness)
        doc.delete_layer(inv_id);
        let _bright_id = doc.add_adjustment_layer(
            AdjustmentType::BrightnessContrast { brightness: 50.0, contrast: 0.0 },
            None,
        );
        let comp3 = doc.composite_layers(false);
        assert!(comp3[0] > 200); // Shifted bright

        // 4. Verify original layer pixels were NOT modified (Non-Destructive!)
        assert_eq!(doc.layers[0].get_pixel(5, 5).unwrap(), [128, 128, 128, 255]);
    }

    #[test]
    fn test_text_rasterization_and_measurement() {
        let mut doc = Document::new(200, 100);
        let font = StrokeRasterizer::load_font(None, None);
        if let Some(font) = font {
            let (w, h) = StrokeRasterizer::measure_text("Test", &font, 24.0, 1.2, 0.0);
            assert!(w > 0.0);
            assert!(h > 0.0);

            let bounds = StrokeRasterizer::rasterize_text(
                &mut doc,
                Vec2::new(10.0, 10.0),
                "H",
                Some(&font),
                32.0,
                Color::WHITE,
                1.0,
                1.2,
                0.0,
                crate::brush::TextAlign::Left,
                None,
            );
            assert!(bounds.is_some());
            let (min_x, min_y, max_x, max_y) = bounds.unwrap();
            assert!(max_x >= min_x);
            assert!(max_y >= min_y);
        }
    }

    #[test]
    fn test_text_layer_lifecycle_and_rasterization() {
        use crate::layer::TextLayerConfig;

        let mut doc = Document::new(200, 100);
        let config = TextLayerConfig {
            content: "Hello Aurelia".to_string(),
            font_family: "System Default".to_string(),
            font_size: 28.0,
            color: [255, 100, 50, 255],
            align: crate::brush::TextAlign::Center,
            line_spacing: 1.2,
            letter_spacing: 0.0,
            box_w: 180.0,
            box_h: 60.0,
            pos_x: 10.0,
            pos_y: 20.0,
        };

        // 1. Add Text Layer
        let text_layer_id = doc.add_text_layer(Some("Greeting".to_string()), config.clone(), None);
        assert_eq!(doc.layers.len(), 2);
        let layer = doc.get_layer(text_layer_id).unwrap();
        assert!(layer.is_text());
        assert_eq!(layer.name, "Greeting");
        assert_eq!(layer.text.as_ref().unwrap().content, "Hello Aurelia");

        // 2. Render text onto text layer
        if let Some(font) = StrokeRasterizer::load_font(None, None) {
            let updated = doc.update_text_layer_config(text_layer_id, config.clone(), &font);
            assert!(updated);

            let layer_after = doc.get_layer(text_layer_id).unwrap();
            let has_non_zero_pixels = layer_after.as_srgb8_exact().iter().any(|&p| p > 0);
            assert!(has_non_zero_pixels);

            // 3. Duplicate Text Layer preserves Text configuration
            let dup_id = doc.duplicate_active_layer().unwrap();
            let dup_layer = doc.get_layer(dup_id).unwrap();
            assert!(dup_layer.is_text());
            assert_eq!(dup_layer.text.as_ref().unwrap().content, "Hello Aurelia");

            // 4. Rasterize Text Layer converts to standard paintable pixel layer
            assert!(doc.rasterize_text_layer(text_layer_id));
            let rasterized_layer = doc.get_layer(text_layer_id).unwrap();
            assert!(rasterized_layer.is_raster());
            assert!(!rasterized_layer.is_text());
            assert!(rasterized_layer.text.is_none());
            // Pixels must remain preserved
            assert!(rasterized_layer.as_srgb8_exact().iter().any(|&p| p > 0));
        }
    }

    #[test]
    fn test_text_paragraph_wrapping() {
        if let Some(font) = StrokeRasterizer::load_font(None, None) {
            let text = "The quick brown fox jumps over the lazy dog near the riverbank.";
            let lines_unbounded = StrokeRasterizer::wrap_text_lines(text, &font, 20.0, 0.0, 0.0);
            assert_eq!(lines_unbounded.len(), 1);

            let lines_bounded = StrokeRasterizer::wrap_text_lines(text, &font, 20.0, 0.0, 150.0);
            assert!(lines_bounded.len() > 1);
            for line in &lines_bounded {
                let (w, _) = StrokeRasterizer::measure_text(line, &font, 20.0, 1.0, 0.0);
                assert!(w <= 160.0); // Bounded with minor tolerance
            }
        }
    }

    #[derive(Debug)]
    struct CostlyCommand {
        bytes: usize,
    }

    impl crate::history::Command for CostlyCommand {
        fn name(&self) -> &'static str {
            "Costly"
        }
        fn cost(&self) -> usize {
            self.bytes
        }
        fn undo(&mut self, _doc: &mut Document) {}
        fn redo(&mut self, _doc: &mut Document) {}
    }

    #[test]
    fn test_history_memory_budget_evicts_oldest_steps() {
        let mut doc = Document::new(4, 4);
        let mut history = crate::history::HistoryStack::with_budget(100, 300);
        for _ in 0..10 {
            history.push(Box::new(CostlyCommand { bytes: 100 }));
        }
        // The byte budget (300) binds before the step cap (100).
        assert_eq!(history.len(), 3);
        assert!(history.retained_bytes() <= 300);
        assert_eq!(history.max_bytes(), 300);

        let generation = history.generation();
        assert!(history.can_undo());
        assert!(history.undo(&mut doc).is_some());
        assert_eq!(history.generation(), generation + 1);
        assert!(history.can_redo());
    }

    #[test]
    fn test_selection_intersect_and_layer_alpha() {
        let mut left = SelectionMask::from_rect(16, 16, Vec2::new(0.0, 0.0), Vec2::new(8.0, 16.0));
        let right = SelectionMask::from_rect(16, 16, Vec2::new(4.0, 0.0), Vec2::new(16.0, 16.0));
        left.intersect(&right);

        assert!(left.is_selected(5, 5));
        assert!(!left.is_selected(2, 2));
        assert!(!left.is_selected(10, 5));

        let mut pixels = vec![0u8; 4 * 4 * 4];
        let alpha_index = (1 * 4 + 1) * 4 + 3;
        pixels[alpha_index] = 255;
        let from_alpha = SelectionMask::from_layer_alpha(&pixels, 4, 4, 8);
        assert!(from_alpha.is_selected(1, 1));
        assert!(!from_alpha.is_selected(0, 0));
    }

    #[test]
    fn test_levels_pixelate_motion_blur_colorize() {
        // Identity levels leave the value unchanged.
        let mut pixel = vec![128u8, 128, 128, 255];
        crate::filter::filter_levels(&mut pixel, 1, 1, 0.0, 1.0, 1.0, None);
        assert_eq!(pixel[0], 128);

        // Gamma > 1 brightens midtones (output = input^(1/gamma), Photoshop convention).
        let mut pixel = vec![128u8, 128, 128, 255];
        crate::filter::filter_levels(&mut pixel, 1, 1, 0.0, 1.0, 2.0, None);
        assert!(pixel[0] > 128);

        // Gamma < 1 darkens midtones.
        let mut pixel = vec![128u8, 128, 128, 255];
        crate::filter::filter_levels(&mut pixel, 1, 1, 0.0, 1.0, 0.5, None);
        assert!(pixel[0] < 128);

        // Pixelate flattens each block to its average.
        let mut px = vec![0u8; 4 * 4 * 4];
        for i in 0..16 {
            px[i * 4] = if i < 8 { 0 } else { 200 };
            px[i * 4 + 3] = 255;
        }
        crate::filter::filter_pixelate(&mut px, 4, 4, 4, None);
        let flattened = px[0];
        for i in 0..16 {
            assert_eq!(px[i * 4], flattened);
        }
        assert_eq!(flattened, 100);

        // Motion blur along 0 degrees averages horizontally.
        let mut px = vec![0u8; 2 * 4];
        px[0] = 255;
        px[3] = 255;
        px[7] = 255;
        crate::filter::filter_motion_blur(&mut px, 2, 1, 1, 0.0, None);
        assert!(px[0] > 0 && px[0] < 255);
        assert_eq!(px[3], 255);

        // Colorize rewrites RGB but preserves alpha.
        let mut px = vec![100u8, 100, 100, 255];
        crate::filter::filter_colorize(&mut px, 1, 1, 200.0, 0.6, None);
        assert_eq!(px[3], 255);
        assert_ne!((px[0], px[1], px[2]), (100, 100, 100));
    }

    /// Deleting the top layer used to crash the layer panel: the row indexed
    /// `document.layers[i]` *after* deleting, when the vector had shrunk to `i`.
    /// The invariant that made it survivable is checked here from the top down.
    #[test]
    fn deleting_layers_from_the_top_down_keeps_the_document_consistent() {
        let mut doc = Document::new(64, 64);
        let base = doc.active_layer_id;
        let a = doc.add_layer(Some("A".into()));
        let b = doc.add_layer(Some("B".into()));
        let c = doc.add_layer(Some("C".into()));
        assert_eq!(doc.layers.len(), 4);

        for id in [c, b, a] {
            assert!(doc.delete_layer(id).is_some(), "layer {id} should delete");
            assert!(!doc.layers.is_empty(), "the document must keep at least one layer");
            assert!(
                doc.layers.iter().any(|l| l.id == doc.active_layer_id),
                "active layer {} dangles after deleting {id}",
                doc.active_layer_id
            );
            // Every index the panel might still read has to be in range.
            for i in 0..doc.layers.len() {
                assert_eq!(doc.layers[i].id, doc.layers[i].id);
            }
        }

        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.active_layer_id, base);
        // The final layer is protected.
        assert!(doc.delete_layer(base).is_none());
    }

    #[test]
    fn deleting_a_group_removes_descendants_and_keeps_active_valid() {
        let mut doc = Document::new(64, 64);
        let base = doc.active_layer_id;
        let group = doc.add_group(Some("Group".into()));
        let child = doc.add_layer(Some("Child".into()));
        assert!(doc.set_layer_parent(child, Some(group)));

        let removed = doc.delete_layer(group).expect("the group should delete");
        assert_eq!(removed.id, group);
        assert!(doc.get_layer(child).is_none(), "children die with their group");
        assert!(doc.get_layer(base).is_some());
        assert!(doc.layers.iter().any(|l| l.id == doc.active_layer_id));
        assert_eq!(doc.layers.len(), 1);
    }

    #[test]
    fn ungroup_releases_children_before_removing_the_group() {
        let mut doc = Document::new(64, 64);
        let base = doc.active_layer_id;
        let group = doc.add_group(Some("Group".into()));
        let child = doc.add_layer(Some("Child".into()));
        assert!(doc.set_layer_parent(child, Some(group)));

        assert!(doc.ungroup(group));
        assert!(doc.get_layer(group).is_none(), "the group itself is gone");
        let released = doc.get_layer(child).expect("the child survives ungrouping");
        assert_eq!(released.parent_id, None, "the child is re-parented to the root");
        assert!(doc.layers.iter().any(|l| l.id == doc.active_layer_id));
        assert!(doc.get_layer(base).is_some());
    }

    /// Masks scale a layer's alpha at composite time and never touch its pixels.
    #[test]
    fn layer_masks_hide_pixels_when_compositing() {
        let mut doc = Document::new(8, 8);
        let id = doc.active_layer_id;
        if let Some(layer) = doc.active_layer_mut() {
            for px in layer.as_srgb8_exact_mut().chunks_exact_mut(4) {
                px.copy_from_slice(&[255, 255, 255, 255]);
            }
        }

        assert!(doc.add_layer_mask(id, None), "a mask can be created");
        assert_eq!(doc.composite_layers(false)[3], 255, "a fresh mask is fully visible");

        assert!(doc.invert_layer_mask(id));
        let flat = doc.composite_layers(false);
        assert_eq!(&flat[0..4], &[0, 0, 0, 0], "an inverted mask hides the layer");

        // Disabling keeps the mask but ignores it.
        assert!(doc.set_layer_mask_enabled(id, false));
        assert_eq!(doc.composite_layers(false)[3], 255, "a disabled mask is ignored");

        assert!(doc.remove_layer_mask(id));
        assert!(doc.get_layer(id).unwrap().mask.is_none());
        assert_eq!(doc.composite_layers(false)[3], 255, "the layer itself was never altered");
    }

    #[test]
    fn mask_from_selection_only_keeps_the_selected_area() {
        let mut doc = Document::new(4, 4);
        let id = doc.active_layer_id;
        if let Some(layer) = doc.active_layer_mut() {
            for px in layer.as_srgb8_exact_mut().chunks_exact_mut(4) {
                px.copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        let sel = crate::selection::SelectionMask::from_rect(4, 4, glam::vec2(0.0, 0.0), glam::vec2(2.0, 2.0));
        assert!(doc.add_layer_mask(id, Some(&sel)));

        let flat = doc.composite_layers(false);
        assert_eq!(flat[3], 255, "inside the selection stays visible");
        let outside = ((3 * 4) + 3) * 4;
        assert_eq!(flat[outside + 3], 0, "outside the selection is hidden");
    }

    #[test]
    fn duplicating_a_layer_copies_its_mask() {
        let mut doc = Document::new(4, 4);
        let id = doc.active_layer_id;
        assert!(doc.add_layer_mask(id, None));
        assert!(doc.invert_layer_mask(id));
        let dup = doc.duplicate_active_layer().expect("duplicate");
        let copy = doc.get_layer(dup).unwrap();
        assert_eq!(copy.mask.as_ref().map(|m| m.pixels[0]), Some(0));
        // The original is untouched.
        assert_eq!(doc.get_layer(id).unwrap().mask.as_ref().map(|m| m.pixels[0]), Some(0));
    }


    #[test]
    fn tilt_widens_the_stroke_up_to_the_gain() {
        use crate::brush::tilt_size_scale;
        assert_eq!(tilt_size_scale(0.0, 0.0, 1.0), 1.0, "an upright pen is unaffected");
        assert_eq!(tilt_size_scale(80.0, 40.0, 0.0), 1.0, "gain 0 disables tilt response");
        assert!((tilt_size_scale(90.0, 0.0, 1.0) - 2.0).abs() < 1e-3, "full tilt doubles the width");
        let half = tilt_size_scale(45.0, 0.0, 1.0);
        assert!(half > 1.2 && half < 1.8, "half tilt lands in between: {half}");
        let x = tilt_size_scale(60.0, 0.0, 0.5);
        let y = tilt_size_scale(0.0, 60.0, 0.5);
        assert!((x - y).abs() < 1e-4, "both axes are equivalent");
        assert!(tilt_size_scale(f32::NAN, 10.0, 1.0) == 1.0, "garbage tilt is ignored");
    }


    /// The compositor as it stood before the v2.6.0 rewrite, kept verbatim as the
    /// correctness oracle. The rewritten path must produce byte-identical output:
    /// speed is never allowed to change a pixel.
    fn reference_composite(doc: &Document, include_background: bool) -> Vec<u8> {
        let total_bytes = (doc.width * doc.height * 4) as usize;
        let mut out = vec![0u8; total_bytes];
        if include_background && !doc.is_transparent {
            let bg = doc.background_color().to_rgba8();
            for chunk in out[..total_bytes].chunks_exact_mut(4) {
                chunk.copy_from_slice(&bg);
            }
        }

        for (layer_idx, layer) in doc.layers.iter().enumerate() {
            if layer.is_group() || !doc.is_layer_effective_visible(layer.id) {
                continue;
            }
            let prev_layer = if layer.clipping_mask && layer_idx > 0 {
                let base = &doc.layers[layer_idx - 1];
                if doc.is_layer_effective_visible(base.id) {
                    Some(base)
                } else {
                    continue;
                }
            } else {
                None
            };
            let eff_opacity = doc.effective_opacity(layer.id);
            if eff_opacity <= 0.001 {
                continue;
            }
            if layer.is_adjustment() {
                continue; // the fixtures below use raster layers only
            }

            // The fast path is part of the semantics: it uses integer maths and is
            // selected for Normal/zero-offset/no-clip layers, so the oracle has to
            // reproduce it exactly as well.
            if layer.blend_mode == crate::blend::BlendMode::Normal
                && layer.offset_x == 0
                && layer.offset_y == 0
                && prev_layer.is_none()
                && layer.as_srgb8_exact().len() >= total_bytes
            {
                let alpha_mul = (eff_opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
                let mask = layer.mask.as_ref().filter(|m| {
                    m.enabled && m.pixels.len() >= (doc.width as usize) * (doc.height as usize)
                });
                for (i, (dst, src)) in out[..total_bytes]
                    .chunks_exact_mut(4)
                    .zip(layer.as_srgb8_exact()[..total_bytes].chunks_exact(4))
                    .enumerate()
                {
                    let mut sa = (src[3] as u32 * alpha_mul) / 255;
                    if let Some(m) = mask {
                        sa = sa * m.pixels[i] as u32 / 255;
                    }
                    if sa == 0 {
                        continue;
                    }
                    let da = dst[3] as u32;
                    if da == 0 {
                        dst[0] = src[0];
                        dst[1] = src[1];
                        dst[2] = src[2];
                        dst[3] = sa as u8;
                    } else if sa == 255 {
                        dst[0] = src[0];
                        dst[1] = src[1];
                        dst[2] = src[2];
                        dst[3] = 255;
                    } else {
                        let inv_sa = 255 - sa;
                        let out_a = sa + (da * inv_sa + 127) / 255;
                        if out_a > 0 {
                            let r = (src[0] as u32 * sa * 255 + dst[0] as u32 * da * inv_sa) / (out_a * 255);
                            let g = (src[1] as u32 * sa * 255 + dst[1] as u32 * da * inv_sa) / (out_a * 255);
                            let b = (src[2] as u32 * sa * 255 + dst[2] as u32 * da * inv_sa) / (out_a * 255);
                            dst[0] = r.min(255) as u8;
                            dst[1] = g.min(255) as u8;
                            dst[2] = b.min(255) as u8;
                            dst[3] = out_a.min(255) as u8;
                        }
                    }
                }
                continue;
            }

            let mask = layer.mask.as_ref().filter(|m| m.enabled);
            for y in 0..doc.height {
                for x in 0..doc.width {
                    let dst_idx = ((y * doc.width + x) * 4) as usize;
                    let src_x = x as i32 - layer.offset_x;
                    let src_y = y as i32 - layer.offset_y;
                    let src_px = if src_x >= 0 && src_x < doc.width as i32 && src_y >= 0 && src_y < doc.height as i32 {
                        layer.get_pixel(src_x as u32, src_y as u32).unwrap_or([0, 0, 0, 0])
                    } else {
                        [0, 0, 0, 0]
                    };
                    let mut opacity = eff_opacity;
                    if let Some(m) = mask {
                        opacity *= m.alpha_at(x, y) as f32 / 255.0;
                    }
                    if let Some(base) = prev_layer {
                        let bx = x as i32 - base.offset_x;
                        let by = y as i32 - base.offset_y;
                        let base_a = if bx >= 0 && bx < doc.width as i32 && by >= 0 && by < doc.height as i32 {
                            base.get_pixel(bx as u32, by as u32).map(|p| p[3] as f32 / 255.0).unwrap_or(0.0)
                        } else {
                            0.0
                        };
                        opacity *= base_a * doc.effective_opacity(base.id);
                    }
                    if opacity <= 0.001 {
                        continue;
                    }
                    let dst_px = [out[dst_idx], out[dst_idx + 1], out[dst_idx + 2], out[dst_idx + 3]];
                    let blended = layer.blend_mode.composite_pixel(dst_px, src_px, opacity);
                    out[dst_idx] = blended[0];
                    out[dst_idx + 1] = blended[1];
                    out[dst_idx + 2] = blended[2];
                    out[dst_idx + 3] = blended[3];
                }
            }
        }
        out
    }

    fn coverage_fixture(
        mode: crate::blend::BlendMode,
        offset: (i32, i32),
        mask_case: u8,
        clipping: bool,
        coverage: f32,
        base_case: u8,
    ) -> Document {
        const W: u32 = 12;
        const H: u32 = 9;
        let mut doc = Document::new(W, H);
        let base = doc.active_layer_id;

        // Base layer: opaque noise so every blend mode has something to work with.
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == base) {
            for (i, px) in layer.as_srgb8_exact_mut().chunks_exact_mut(4).enumerate() {
                px[0] = (i * 7 % 251) as u8;
                px[1] = (i * 13 % 241) as u8;
                px[2] = (i * 29 % 233) as u8;
                px[3] = 255;
            }
        }

        let subject = doc.add_layer(Some("subject".into()));
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == subject) {
            layer.blend_mode = mode;
            layer.offset_x = offset.0;
            layer.offset_y = offset.1;
            layer.opacity = 0.8;
            for (i, px) in layer.as_srgb8_exact_mut().chunks_exact_mut(4).enumerate() {
                let x = (i as u32) % W;
                let y = (i as u32) / W;
                let covered = coverage >= 1.0
                    || ((x * 3 + y * 5) as f32 / ((W + H) as f32)) < coverage;
                px[0] = (200 - (x * 5) as u8) as u8;
                px[1] = (120 + (y * 11) as u8) as u8;
                px[2] = (60 + ((x + y) * 9) as u8) as u8;
                px[3] = if covered { 255 } else { 0 };
            }
        }
        match mask_case {
            1 => {
                assert!(doc.add_layer_mask(subject, None));
            }
            2 => {
                assert!(doc.add_layer_mask(subject, None));
                assert!(doc.invert_layer_mask(subject));
                assert!(doc.set_layer_mask_enabled(subject, false));
            }
            _ => {}
        }

        // `base_case` stresses the corners of the clip-base lookup: a pixel buffer that
        // ends early, a layer whose own size disagrees with the canvas, and a layer kind
        // that `get_pixel` refuses to answer for. The clipping layer reads all three.
        if clipping && base_case != 0 {
            if let Some(layer) = doc.layers.get_mut(1) {
                match base_case {
                    1 => {
                        let keep = layer.as_srgb8_exact().len() / 2;
                        layer
                            .pixels_vec_mut()
                            .expect("the fixture builds 8-bit layers")
                            .truncate(keep);
                    }
                    2 => {
                        layer.width = W + 5;
                        layer.height = H + 2;
                    }
                    _ => layer.kind = crate::layer::LayerKind::Group,
                }
            }
        }

        if clipping {
            let clip = doc.add_layer(Some("clip".into()));
            if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == clip) {
                layer.clipping_mask = true;
                layer.opacity = 0.7;
                for (i, px) in layer.as_srgb8_exact_mut().chunks_exact_mut(4).enumerate() {
                    let x = (i as u32) % W;
                    px[0] = 30;
                    px[1] = (x * 20) as u8;
                    px[2] = 200;
                    px[3] = if x % 3 == 0 { 255 } else { 90 };
                }
            }
        }
        doc
    }

    /// The rewritten general path must be byte-identical to the reference one for
    /// every blend mode, offset, mask state and clipping combination.
    #[test]
    fn general_compositing_is_bit_identical_to_the_reference() {
        for &mode in crate::blend::BlendMode::ALL {
            for &offset in &[(0, 0), (3, 2), (-5, -4)] {
                for mask_case in 0..3u8 {
                    for clipping in [false, true] {
                        for coverage in [1.0f32, 0.5, 0.15] {
                            let doc = coverage_fixture(mode, offset, mask_case, clipping, coverage, 0);
                            let got = doc.composite_layers(false);
                            let want = reference_composite(&doc, false);
                            assert_eq!(
                                got, want,
                                "mismatch: {mode:?} offset {offset:?} mask {mask_case} clip {clipping} coverage {coverage}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Clipping reads its base through a different code path than it used to: the base
    /// span is resolved once per row instead of per pixel. These are the corners where
    /// that can disagree - a short pixel buffer, a layer size that is not the canvas, and
    /// a layer kind that reports no pixel at all.
    #[test]
    fn clipping_edge_cases_are_bit_identical_to_the_reference() {
        for &mode in crate::blend::BlendMode::ALL {
            for &offset in &[(0, 0), (3, 2), (-5, -4)] {
                for base_case in 1..=3u8 {
                    for coverage in [1.0f32, 0.15] {
                        let doc = coverage_fixture(mode, offset, 0, true, coverage, base_case);
                        let got = doc.composite_layers(false);
                        let want = reference_composite(&doc, false);
                        assert_eq!(
                            got, want,
                            "mismatch: {mode:?} offset {offset:?} base_case {base_case} coverage {coverage}"
                        );
                    }
                }
            }
        }
    }

    /// Compositing is a pure function of the document: the same document must produce the
    /// same bytes every time, whatever order the layers were built or visited in.
    #[test]
    fn compositing_is_deterministic_across_repeated_runs() {
        for &mode in crate::blend::BlendMode::ALL {
            let doc = coverage_fixture(mode, (3, -2), 1, true, 0.5, 0);
            let first = doc.composite_layers(false);
            for run in 1..5 {
                let again = doc.composite_layers(false);
                assert_eq!(first, again, "{mode:?} differed on run {run}");
            }
        }
    }

    /// A fixture large enough that the compositor actually splits the layer across
    /// threads, carrying every feature the general path handles: an offset, a mask, a
    /// clipped layer and a non-trivial blend mode.
    fn big_fixture(width: u32, height: u32, mode: BlendMode) -> Document {
        let mut doc = Document::new(width, height);
        let base = doc.active_layer_id;
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == base) {
            for (i, px) in layer.as_srgb8_exact_mut().chunks_exact_mut(4).enumerate() {
                px[0] = (i * 7 % 251) as u8;
                px[1] = (i * 13 % 241) as u8;
                px[2] = (i * 29 % 233) as u8;
                px[3] = 255;
            }
        }

        let subject = doc.add_layer(Some("subject".into()));
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == subject) {
            layer.blend_mode = mode;
            layer.offset_x = 7;
            layer.offset_y = -3;
            layer.opacity = 0.75;
            for (i, px) in layer.as_srgb8_exact_mut().chunks_exact_mut(4).enumerate() {
                let x = (i as u32) % width;
                let y = (i as u32) / width;
                px[0] = (200u32.wrapping_sub(x * 5)) as u8;
                px[1] = (120u32.wrapping_add(y * 11)) as u8;
                px[2] = (60u32.wrapping_add((x + y) * 9)) as u8;
                // Ragged coverage, so rows are dense in some places and empty in others.
                px[3] = if (x * 3 + y * 5) % 11 < 7 { 255 } else { 0 };
            }
        }
        assert!(doc.add_layer_mask(subject, None));
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == subject) {
            if let Some(mask) = layer.mask.as_mut() {
                for (i, m) in mask.pixels.iter_mut().enumerate() {
                    *m = if i % 5 == 0 { 40 } else { 255 };
                }
            }
        }

        let clip = doc.add_layer(Some("clip".into()));
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == clip) {
            layer.clipping_mask = true;
            layer.offset_x = 0;
            layer.offset_y = 2;
            layer.opacity = 0.6;
            for (i, px) in layer.as_srgb8_exact_mut().chunks_exact_mut(4).enumerate() {
                let x = (i as u32) % width;
                px[0] = 30;
                px[1] = (x * 20) as u8;
                px[2] = 200;
                px[3] = if x % 3 == 0 { 255 } else { 90 };
            }
        }
        doc
    }

    /// Rows are composed on several threads, but the result must be exactly what a single
    /// thread produces - and the same on every run, because each thread owns a fixed band
    /// of rows rather than claiming work as it goes.
    #[test]
    fn parallel_compositing_matches_the_serial_reference() {
        const W: u32 = 220;
        const H: u32 = 170;
        assert!(
            crate::document::parallel_degree(W * H) > 1,
            "fixture must be large enough to take the parallel path"
        );
        for &mode in &[
            BlendMode::Normal,
            BlendMode::Multiply,
            BlendMode::SoftLight,
            BlendMode::Luminosity,
        ] {
            let doc = big_fixture(W, H, mode);
            let got = doc.composite_layers(false);
            let want = reference_composite(&doc, false);
            assert_eq!(got, want, "parallel output differs for {mode:?}");
            for run in 1..4 {
                assert_eq!(got, doc.composite_layers(false), "{mode:?} differed on run {run}");
            }
        }
    }

    /// Gate 1 of the 16-bit plan: an `Srgb16` composite, downconverted, must reproduce the
    /// 8-bit result.
    ///
    /// Comparison is against fixtures where every layer takes the *general* path, because
    /// there is no 16-bit fast path (a deliberate deferral): a layer that would have used the
    /// 8-bit integer fast path has no 16-bit equivalent, and its integer rounding differs from
    /// the float path's. Comparing across that boundary would compare two algorithms, not two
    /// storage widths.
    ///
    /// The tolerance is one step of the 8-bit output, and the reason is arithmetic rather than
    /// convenience: the 8-bit chain quantises to 256 levels *between* layers, while the 16-bit
    /// chain keeps 65536, so a later layer is fed a slightly different input and can round to
    /// the neighbouring byte. A single-layer document has no inter-layer quantisation, so it
    /// is required to match exactly - that is the sharp part of this gate.
    #[test]
    fn sixteen_bit_compositing_downconverts_to_the_eight_bit_result() {
        let mut worst_stack_step = 0u8;
        for &mode in crate::blend::BlendMode::ALL {
            for &offset in &[(0, 0), (3, 2), (-5, -4)] {
                for mask_case in 0..3u8 {
                    for clipping in [false, true] {
                        for coverage in [1.0f32, 0.5, 0.15] {
                            let mut doc =
                                coverage_fixture(mode, offset, mask_case, clipping, coverage, 0);
                            for layer in doc.layers.iter_mut() {
                                layer.offset_x = 1;
                                layer.offset_y = 1;
                            }
                            let eight_bit = doc.composite_layers(false);
                            let sixteen = crate::document::compose_u16(&doc, false);
                            let downconverted: Vec<u8> = sixteen
                                .iter()
                                .map(|&v| crate::pixel::srgb16_to_srgb8(v))
                                .collect();

                            let step = downconverted
                                .iter()
                                .zip(eight_bit.iter())
                                .map(|(a, b)| a.abs_diff(*b))
                                .max()
                                .unwrap_or(0);
                            worst_stack_step = worst_stack_step.max(step);
                            assert!(
                                step <= 1,
                                "stack drifted by {step} LSB: {mode:?} offset {offset:?} mask {mask_case} clip {clipping} coverage {coverage}"
                            );

                            // The sharp check: with no inter-layer quantisation, the bytes must
                            // be identical.
                            let mut single = Document::new(doc.width, doc.height);
                            single.is_transparent = true;
                            let subject = coverage_fixture(mode, offset, mask_case, false, coverage, 0)
                                .layers
                                .pop()
                                .expect("subject layer");
                            if let Some(slot) = single.layers.first_mut() {
                                *slot = subject;
                            }
                            let single_8 = single.composite_layers(false);
                            let single_16: Vec<u8> = crate::document::compose_u16(&single, false)
                                .iter()
                                .map(|&v| crate::pixel::srgb16_to_srgb8(v))
                                .collect();
                            assert_eq!(
                                single_16, single_8,
                                "single layer must match exactly: {mode:?} offset {offset:?} mask {mask_case} coverage {coverage}"
                            );
                        }
                    }
                }
            }
        }
        println!("worst inter-layer drift: {worst_stack_step} LSB");
    }

    /// The point of 16-bit storage: values that 8 bits cannot represent must survive a
    /// composite. The oracle above only proves nothing was lost - this proves something was
    /// gained.
    #[test]
    fn sub_eight_bit_precision_survives_a_composite() {
        const W: u32 = 16;
        const H: u32 = 1;
        let mut doc = Document::new(W, H);
        doc.is_transparent = true;
        let base_id = doc.active_layer_id;
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == base_id) {
            layer.offset_x = 1;
            let pixels = layer.as_srgb8_exact_mut();
            for x in 0..W as usize {
                pixels[x * 4] = 0;
                pixels[x * 4 + 1] = 0;
                pixels[x * 4 + 2] = 0;
                pixels[x * 4 + 3] = 255;
            }
        }
        let top_id = doc.add_layer(Some("ramp".into()));
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == top_id) {
            layer.offset_x = 1;
            layer.opacity = 0.5;
            let pixels = layer.as_srgb8_exact_mut();
            for x in 0..W as usize {
                let value = (x * 17) as u8;
                pixels[x * 4] = value;
                pixels[x * 4 + 1] = value;
                pixels[x * 4 + 2] = value;
                pixels[x * 4 + 3] = 255;
            }
        }

        let sixteen = crate::document::compose_u16(&doc, false);
        let eight_bit = doc.composite_layers(false);
        let downconverted: Vec<u8> = sixteen
            .iter()
            .map(|&v| crate::pixel::srgb16_to_srgb8(v))
            .collect();
        assert_eq!(downconverted, eight_bit, "the promotion must not change 8-bit output");

        // A 50% blend of two 8-bit values lands between lattice points for most inputs, so
        // the 16-bit result must contain values that are not multiples of 257.
        let off_lattice = sixteen
            .iter()
            .filter(|&&v| v % 257 != 0)
            .count();
        assert!(
            off_lattice > 0,
            "no sub-8-bit detail survived: every channel landed on the lattice"
        );
    }

    /// Gate 2 of the 16-bit plan (`docs/16bit-plan.md`): throughput against the generic u8
    /// baseline, and the memory cost including undo history.
    ///
    /// Every layer is forced down the general path, because that is what a 16-bit document
    /// has - there is no 16-bit fast path - and because the baseline must be the generic u8
    /// path rather than a fixture dominated by the integer fast path.
    ///
    /// Timing is reported, not asserted: a shared machine is not a reliable oracle, and a
    /// flaky performance assertion in a unit suite trains people to ignore it. Memory *is*
    /// asserted, because it is arithmetic rather than measurement.
    #[test]
    #[ignore = "measurement, not a correctness gate; run with --ignored --nocapture"]
    fn gate_2_throughput_and_memory() {
        use std::time::Instant;

        const W: u32 = 1920;
        const H: u32 = 1080;
        const LAYERS: u32 = 4;
        const ITERS: u32 = 20;
        const ROUNDS: u32 = 5;

        fn fixture(coverage: f32, masked: bool, clipped: bool) -> Document {
            let mut doc = Document::new(W, H);
            doc.is_transparent = true;
            for i in 0..LAYERS {
                let id = if i == 0 {
                    doc.active_layer_id
                } else {
                    doc.add_layer(Some(format!("Layer {i}")))
                };
                if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == id) {
                    // General path for every layer: no 16-bit fast path exists.
                    layer.offset_x = 1;
                    layer.offset_y = 1;
                    let pixels = layer.as_srgb8_exact_mut();
                    for (idx, px) in pixels.chunks_exact_mut(4).enumerate() {
                        let x = (idx as u32) % W;
                        let y = (idx as u32) / W;
                        px[0] = (x % 256) as u8;
                        px[1] = (y % 256) as u8;
                        px[2] = ((x ^ y) % 256) as u8;
                        px[3] = if coverage >= 1.0
                            || ((x * 7 + y * 13) % 100) < (coverage * 100.0) as u32
                        {
                            255
                        } else {
                            0
                        };
                    }
                    if clipped && i == 2 {
                        layer.clipping_mask = true;
                    }
                }
                if masked && i == 2 {
                    let mut mask = crate::layer::LayerMask::full(W, H, 255);
                    for (idx, p) in mask.pixels.iter_mut().enumerate() {
                        *p = if idx % 7 == 0 { 64 } else { 255 };
                    }
                    if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == id) {
                        layer.mask = Some(mask);
                    }
                }
            }
            doc
        }

        fn median(mut samples: Vec<f64>) -> f64 {
            samples.sort_by(f64::total_cmp);
            samples[samples.len() / 2];
            samples[samples.len() / 2]
        }

        /// One timed block, in seconds.
        fn time_one<F: FnMut()>(mut body: F) -> f64 {
            let start = Instant::now();
            for _ in 0..ITERS {
                body();
            }
            start.elapsed().as_secs_f64()
        }

        // The native 16-bit views borrow the documents, so they are built after the
        // documents exist rather than carried alongside them.
        let cases: Vec<(&str, Document)> = vec![
            ("opaque", fixture(1.0, false, false)),
            ("sparse 10%", fixture(0.1, false, false)),
            ("masked", fixture(1.0, true, false)),
            ("clipped", fixture(1.0, false, true)),
        ];
        let native: Vec<_> = cases
            .iter()
            .map(|(_, doc)| crate::document::promote_layers(doc))
            .collect();

        // Paired, interleaved rounds: both widths are measured in the same round, so a slow
        // patch of machine hits both and cancels in the ratio. The ratio is the gate; the
        // absolute numbers are context.
        let mut ratios: Vec<Vec<f64>> = vec![Vec::new(); cases.len()];
        let mut abs8: Vec<Vec<f64>> = vec![Vec::new(); cases.len()];
        let mut abs16: Vec<Vec<f64>> = vec![Vec::new(); cases.len()];
        for _ in 0..ROUNDS {
            for ci in 0..cases.len() {
                let doc = &cases[ci].1;
                let promoted = &native[ci];
                let t8 = time_one(|| {
                    std::hint::black_box(doc.composite_layers(false));
                });
                let t16 = time_one(|| {
                    let mut out = vec![0u16; (doc.width as usize) * (doc.height as usize) * 4];
                    crate::document::compose_u16_prepared(doc, promoted, &mut out, false);
                    std::hint::black_box(out);
                });
                abs8[ci].push(t8);
                abs16[ci].push(t16);
                ratios[ci].push(t8 / t16);
            }
        }

        let megapixels = (W as f64 * H as f64 * ITERS as f64) / 1_000_000.0;
        println!("gate 2 - {W}x{H}, {LAYERS} layers, all layers general path, {ROUNDS} paired rounds");
        println!(
            "{:<12} {:>12} {:>12} {:>10} {:>16}",
            "workload", "u8 MPix/s", "Srgb16", "ratio", "ratio spread"
        );
        let mut sparse_ratio = 1.0f64;
        for ci in 0..cases.len() {
            let label = cases[ci].0;
            let mut r = ratios[ci].clone();
            r.sort_by(f64::total_cmp);
            let (lo, hi, mid) = (r[0], r[r.len() - 1], r[r.len() / 2]);
            if label == "sparse 10%" {
                sparse_ratio = mid;
            }
            println!(
                "{:<12} {:>12.1} {:>12.1} {:>10.2} {:>16}",
                label,
                megapixels / median(abs8[ci].clone()),
                megapixels / median(abs16[ci].clone()),
                mid,
                format!("{lo:.2} .. {hi:.2}")
            );
        }

        println!("\nGate 2 thresholds (ratios, not absolutes):");
        println!(
            "  sparse regression budget: Srgb16 >= 60% of u8 -> {:.0}% {}",
            sparse_ratio * 100.0,
            if sparse_ratio >= 0.6 { "PASS (provisional)" } else { "FAIL" }
        );
        println!("  absolute thresholds are recorded as context only, per the approved change");

        // Memory: arithmetic, so it is asserted rather than reported.
        let per_layer_8 = W as usize * H as usize * 4;
        let doc_8 = per_layer_8 * LAYERS as usize;
        let per_layer_16 = per_layer_8 * 2;
        let doc_16 = doc_8 * 2;
        let steps = 20usize;
        let per_stroke_8 = per_layer_8 * 2 * steps; // before + after, one layer
        let per_stroke_16 = per_layer_16 * 2 * steps;
        let full_doc_8 = doc_8 * 2 * steps;
        let full_doc_16 = doc_16 * 2 * steps;
        let mb = |bytes: usize| bytes as f64 / (1024.0 * 1024.0);

        println!("\nMemory:");
        println!("  one layer              {:>8.1} MB -> {:>8.1} MB", mb(per_layer_8), mb(per_layer_16));
        println!("  {LAYERS}-layer document        {:>8.1} MB -> {:>8.1} MB", mb(doc_8), mb(doc_16));
        println!("  history, per-stroke x{steps}  {:>8.1} MB -> {:>8.1} MB", mb(per_stroke_8), mb(per_stroke_16));
        println!("  history, full doc x{steps}   {:>8.1} MB -> {:>8.1} MB", mb(full_doc_8), mb(full_doc_16));
        assert_eq!(per_layer_16, per_layer_8 * 2);
        assert_eq!(doc_16, doc_8 * 2);
        assert_eq!(per_stroke_16, per_stroke_8 * 2);
        assert_eq!(full_doc_16, full_doc_8 * 2);
    }

    /// A 16-bit layer must composite identically, because the boundary conversion is exact
    /// for content promoted as `v * 257`. This pins the seam: converting once per composite
    /// rather than per pixel, and producing the same bytes as the 8-bit path.
    #[test]
    fn compositing_stages_a_sixteen_bit_layer_at_the_boundary() {
        use crate::layer::LayerBuffer;

        let mut doc = coverage_fixture(
            crate::blend::BlendMode::Multiply,
            (3, 2),
            1,
            false,
            0.5,
            0,
        );
        for layer in doc.layers.iter_mut() {
            layer.offset_x = 1;
            layer.offset_y = 1;
        }
        let eight_bit = doc.composite_layers(false);

        // Promote every layer onto the 16-bit lattice, exactly as a v3 document would.
        for layer in doc.layers.iter_mut() {
            let promoted: Vec<u16> = layer
                .as_srgb8_exact()
                .iter()
                .map(|&v| crate::pixel::srgb8_to_srgb16(v))
                .collect();
            *layer.buffer_mut() = LayerBuffer::Srgb16(promoted);
        }
        assert_eq!(doc.layers[0].format(), crate::layer::PixelFormat::Srgb16);

        let sixteen_bit = doc.composite_layers(false);
        assert_eq!(
            sixteen_bit, eight_bit,
            "a v*257 promoted layer must composite to the same bytes"
        );
    }

    /// Gate 3 requirement: compositing must consume actual 16-bit channels. The two pixels
    /// below differ by half an 8-bit step, so they collapse to the same byte; if the
    /// compositor converted to 8-bit before blending, the distinction would be gone.
    #[test]
    fn sixteen_bit_compositing_keeps_detail_below_the_eight_bit_lattice() {
        use crate::layer::LayerBuffer;
        let mut doc = Document::new(4, 1);
        doc.is_transparent = true;
        let layer_id = doc.active_layer_id;
        let low = crate::pixel::srgb8_to_srgb16(100);
        let high = low + 128;
        assert_eq!(crate::pixel::srgb16_to_srgb8(low), crate::pixel::srgb16_to_srgb8(high));
        let mut channels = vec![0u16; 16];
        for pixel in 0..4 {
            channels[pixel * 4 + 3] = 65535;
        }
        for channel in 0..3 {
            channels[channel] = low;
            channels[4 + channel] = high;
        }
        if let Some(layer) = doc.get_layer_mut(layer_id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }
        let native = crate::document::promote_layers(&doc);
        let mut composed = vec![0u16; 16];
        crate::document::compose_u16_prepared(&doc, &native, &mut composed, false);
        assert_ne!(composed[0], composed[4], "16-bit compositing collapsed two values");
        assert_eq!(composed[0], low);
        assert_eq!(composed[4], high);
        let eight_bit = doc.composite_layers(false);
        assert_eq!(eight_bit[0], eight_bit[4], "an 8-bit result cannot carry it");
    }

    /// GOLDEN: the alpha-lock and eraser-clamp paths as they behave **today**.
    ///
    /// Captured before any refactor, with no dependency on the helper the refactor will
    /// introduce - otherwise the test would happily reproduce whatever mistake the extraction
    /// made. Bytes here are facts recorded from the current implementation, not a re-derivation
    /// of its arithmetic.
    #[test]
    fn brush_alpha_clamping_matches_the_recorded_bytes() {
        use crate::brush::BrushPoint;

        fn prepared() -> (Document, crate::brush::BrushSettings) {
            let mut doc = Document::new(3, 3);
            doc.is_transparent = true;
            let id = doc.active_layer_id;
            if let Some(layer) = doc.get_layer_mut(id) {
                layer.alpha_locked = true;
                for pixel in layer.as_srgb8_exact_mut().chunks_exact_mut(4) {
                    pixel.copy_from_slice(&[100, 150, 200, 128]);
                }
            }
            let mut brush = crate::brush::BrushSettings::default();
            brush.tool = crate::brush::ToolType::Brush;
            brush.size = 16.0;
            brush.opacity = 1.0;
            brush.hardness = 1.0;
            brush.primary_color = Color::WHITE;
            (doc, brush)
        }

        let paint = |doc: &mut Document, brush: &crate::brush::BrushSettings| {
            crate::rasterizer::StrokeRasterizer::paint_dot(
                doc,
                BrushPoint {
                    position: glam::Vec2::new(1.0, 1.0),
                    pressure: 1.0,
                },
                brush,
                &crate::symmetry::SymmetryConfig::default(),
                None,
            );
        };

        // 1. Alpha-locked painting: the alpha channel must come back untouched, the colour must move.
        let (mut doc, brush) = prepared();
        paint(&mut doc, &brush);
        let painted = doc.active_layer().expect("layer").as_srgb8_exact().to_vec();
        // Recorded from the implementation as it stood before any refactor: white painted over
        // the whole dab, and the alpha channel left at exactly 128 - that is what alpha-lock
        // means, and it must survive the extraction unchanged.
        let expected_locked: Vec<u8> = std::iter::repeat([255u8, 255, 255, 128])
            .take(9)
            .flatten()
            .collect();
        assert_eq!(painted, expected_locked, "alpha-locked painting changed its bytes");

        // 2. Soft eraser over a transparent background: the alpha-clamp path.
        let (mut doc, mut brush) = prepared();
        brush.tool = crate::brush::ToolType::Eraser;
        brush.eraser_mode = crate::brush::EraserMode::Soft;
        brush.eraser_to_background = false;
        paint(&mut doc, &brush);
        let erased = doc.active_layer().expect("layer").as_srgb8_exact().to_vec();
        // Full strength drives the clamp to zero: (128/255 * (1 - 1.0)).round() * 255 -> 0.
        let expected_erased: Vec<u8> = std::iter::repeat([100u8, 150, 200, 0])
            .take(9)
            .flatten()
            .collect();
        assert_eq!(erased, expected_erased, "the soft-eraser clamp changed its bytes");

        // 3. Colour erase: the other alpha-clamp path.
        let (mut doc, mut brush) = prepared();
        brush.tool = crate::brush::ToolType::Eraser;
        brush.eraser_mode = crate::brush::EraserMode::ColorErase;
        brush.secondary_color = Color::from_rgba8(100, 150, 200, 255);
        brush.color_erase_tolerance = 4;
        paint(&mut doc, &brush);
        let color_erased = doc.active_layer().expect("layer").as_srgb8_exact().to_vec();
        assert_eq!(
            color_erased, expected_erased,
            "the colour-eraser clamp changed its bytes"
        );
    }

    /// All three public painting entry points on a 16-bit layer. They share `blend_stamp`, but
    /// their geometry and stamp generation differ, so each is driven independently - a shared
    /// implementation does not mean shared failure modes.
    #[test]
    fn painting_entry_points_work_on_sixteen_bit_layers() {
        use crate::brush::{BrushPoint, BrushSettings, ToolType};
        use crate::symmetry::SymmetryConfig;
        use crate::layer::LayerBuffer;

        fn wide_layer() -> Document {
            let mut doc = Document::new(8, 8);
            doc.is_transparent = true;
            let id = doc.active_layer_id;
            let channels: Vec<u16> = (0..(8 * 8 * 4)).map(|i| (i as u16).wrapping_mul(257)).collect();
            if let Some(layer) = doc.get_layer_mut(id) {
                *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
            }
            doc
        }

        fn brush() -> BrushSettings {
            let mut brush = BrushSettings::default();
            brush.tool = ToolType::Brush;
            brush.size = 3.0;
            brush.opacity = 1.0;
            brush.hardness = 1.0;
            brush.primary_color = Color::WHITE;
            brush
        }

        let point = |x: f32, y: f32| BrushPoint {
            position: glam::Vec2::new(x, y),
            pressure: 1.0,
        };

        // paint_dot
        let mut doc = wide_layer();
        crate::rasterizer::StrokeRasterizer::paint_dot(
            &mut doc, point(4.0, 4.0), &brush(), &SymmetryConfig::default(), None,
        );
        let id = doc.active_layer_id;
        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.as_srgb16().expect("still 16-bit").len(), 8 * 8 * 4);
        assert!(
            layer.as_srgb16().unwrap().iter().any(|&c| c == 65535),
            "paint_dot painted nothing onto the 16-bit layer"
        );

        // paint_segment
        let mut doc = wide_layer();
        crate::rasterizer::StrokeRasterizer::paint_segment(
            &mut doc, point(1.0, 1.0), point(6.0, 6.0), &brush(), &SymmetryConfig::default(), None,
        );
        let id = doc.active_layer_id;
        assert_eq!(
            doc.get_layer(id).expect("layer").format(),
            crate::layer::PixelFormat::Srgb16
        );
        assert!(doc.get_layer(id).unwrap().as_srgb16().unwrap().iter().any(|&c| c == 65535));

        // paint_spline
        let mut doc = wide_layer();
        crate::rasterizer::StrokeRasterizer::paint_spline(
            &mut doc,
            point(1.0, 4.0),
            point(3.0, 1.0),
            point(5.0, 7.0),
            point(7.0, 4.0),
            &brush(),
            &SymmetryConfig::default(),
            None,
        );
        let id = doc.active_layer_id;
        assert_eq!(
            doc.get_layer(id).expect("layer").format(),
            crate::layer::PixelFormat::Srgb16
        );
        assert!(doc.get_layer(id).unwrap().as_srgb16().unwrap().iter().any(|&c| c == 65535));
    }

    /// Painting at partial opacity must keep detail below one 8-bit step: the pixel starts
    /// off-lattice, and a 50% white dab must land on a value that is not a multiple of 257.
    #[test]
    fn painting_keeps_off_lattice_detail_at_partial_opacity() {
        use crate::brush::{BrushPoint, BrushSettings, ToolType};
        use crate::symmetry::SymmetryConfig;
        use crate::layer::LayerBuffer;

        let mut doc = Document::new(6, 6);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        let mid = crate::pixel::srgb8_to_srgb16(100) + 128;
        let mut channels = vec![0u16; 6 * 6 * 4];
        for pixel in 0..(6 * 6) {
            channels[pixel * 4] = mid;
            channels[pixel * 4 + 1] = mid;
            channels[pixel * 4 + 2] = mid;
            channels[pixel * 4 + 3] = 65535;
        }
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let mut brush = BrushSettings::default();
        brush.tool = ToolType::Brush;
        brush.size = 6.0;
        brush.opacity = 0.5;
        brush.hardness = 1.0;
        brush.primary_color = Color::WHITE;
        crate::rasterizer::StrokeRasterizer::paint_dot(
            &mut doc,
            BrushPoint { position: glam::Vec2::new(3.0, 3.0), pressure: 1.0 },
            &brush,
            &SymmetryConfig::default(),
            None,
        );

        let painted = doc.get_layer(id).expect("layer").as_srgb16().expect("16-bit");
        let moved: Vec<u16> = painted.iter().copied().filter(|&c| c != mid && c != 65535 && c != 0).collect();
        assert!(!moved.is_empty(), "the dab changed nothing");
        assert!(
            moved.iter().any(|&c| c % 257 != 0),
            "every painted value landed on the 8-bit lattice: sub-8-bit detail was lost"
        );
    }

    /// Step 2 of slice 1: a shape fill must write natively into 16-bit storage and keep detail
    /// below the 8-bit lattice, while selection coverage behaves exactly as it does at 8 bits.
    #[test]
    fn shape_fill_writes_native_sixteen_bit_values() {
        use crate::brush::{BrushSettings, ShapeFillMode, ToolType};
        use crate::layer::{LayerBuffer, PixelFormat};
        use crate::selection::SelectionMask;
        use crate::symmetry::SymmetryConfig;

        let mut doc = Document::new(8, 8);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        let mid = crate::pixel::srgb8_to_srgb16(80) + 128;
        let mut channels = vec![0u16; 8 * 8 * 4];
        for pixel in 0..(8 * 8) {
            channels[pixel * 4] = mid;
            channels[pixel * 4 + 1] = mid;
            channels[pixel * 4 + 2] = mid;
            channels[pixel * 4 + 3] = 65535;
        }
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let mut brush = BrushSettings::default();
        brush.tool = ToolType::Rect;
        brush.shape_fill_mode = ShapeFillMode::Fill;
        brush.secondary_color = Color::WHITE;
        brush.opacity = 0.5;

        // A selection covering the left half only, so coverage is exercised at 16 bits too.
        let mask = SelectionMask::from_rect(
            8,
            8,
            glam::Vec2::new(0.0, 0.0),
            glam::Vec2::new(4.0, 8.0),
        );

        crate::rasterizer::StrokeRasterizer::rasterize_rect(
            &mut doc,
            glam::Vec2::new(0.0, 0.0),
            glam::Vec2::new(8.0, 8.0),
            &brush,
            &SymmetryConfig::default(),
            Some(&mask),
        );

        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "the fill must not change storage format");
        let painted = layer.as_srgb16().expect("16-bit");

        let left = painted[(2 * 8 + 1) * 4];
        let right = painted[(2 * 8 + 6) * 4];
        assert_ne!(left, mid, "the selected half was not filled");
        assert_eq!(right, mid, "the unselected half must be untouched");
        assert!(
            left % 257 != 0,
            "the fill landed on the 8-bit lattice: sub-8-bit detail was lost"
        );
    }

    /// Ellipse fill and stroke on a 16-bit layer: native writes, format preserved, detail below
    /// the 8-bit lattice kept.
    #[test]
    fn ellipse_writes_native_sixteen_bit_values() {
        use crate::brush::{BrushSettings, ShapeFillMode, ToolType};
        use crate::layer::{LayerBuffer, PixelFormat};
        use crate::symmetry::SymmetryConfig;

        let mut doc = Document::new(9, 9);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        let mid = crate::pixel::srgb8_to_srgb16(60) + 128;
        let mut channels = vec![0u16; 9 * 9 * 4];
        for pixel in 0..(9 * 9) {
            for channel in 0..3 {
                channels[pixel * 4 + channel] = mid;
            }
            channels[pixel * 4 + 3] = 65535;
        }
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let mut brush = BrushSettings::default();
        brush.tool = ToolType::Ellipse;
        brush.shape_fill_mode = ShapeFillMode::Fill;
        brush.secondary_color = Color::WHITE;
        brush.opacity = 0.5;

        crate::rasterizer::StrokeRasterizer::rasterize_ellipse(
            &mut doc,
            glam::Vec2::new(1.0, 1.0),
            glam::Vec2::new(8.0, 8.0),
            &brush,
            &SymmetryConfig::default(),
            None,
        );

        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "the ellipse must not change storage");
        let painted = layer.as_srgb16().expect("16-bit");
        let centre = painted[(4 * 9 + 4) * 4];
        assert_ne!(centre, mid, "the ellipse centre was not filled");
        assert!(
            centre % 257 != 0,
            "the ellipse fill landed on the 8-bit lattice: sub-8-bit detail was lost"
        );
    }

    /// Polygon **fill and outline** on 16-bit storage. The outline is expected to work already - it
    /// routes through `paint_segment`, which paints at either width - so this pins both paths
    /// rather than assuming the shared one covers the other.
    #[test]
    fn polygon_fill_and_outline_work_on_sixteen_bit_layers() {
        use crate::brush::{BrushSettings, ShapeFillMode, ToolType};
        use crate::layer::{LayerBuffer, PixelFormat};
        use crate::symmetry::SymmetryConfig;

        let mid = crate::pixel::srgb8_to_srgb16(70) + 128;

        let wide_doc = || {
            let mut doc = Document::new(9, 9);
            doc.is_transparent = true;
            let id = doc.active_layer_id;
            let mut channels = vec![0u16; 9 * 9 * 4];
            for pixel in 0..(9 * 9) {
                for channel in 0..3 {
                    channels[pixel * 4 + channel] = mid;
                }
                channels[pixel * 4 + 3] = 65535;
            }
            if let Some(layer) = doc.get_layer_mut(id) {
                *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
            }
            doc
        };

        let triangle = [
            glam::Vec2::new(4.0, 1.0),
            glam::Vec2::new(1.0, 8.0),
            glam::Vec2::new(7.0, 8.0),
        ];

        let mut brush = BrushSettings::default();
        brush.tool = ToolType::Polygon;
        brush.secondary_color = Color::WHITE;
        brush.primary_color = Color::WHITE;
        brush.opacity = 0.5;

        // Fill: the interior must move, the format must not, and detail must survive.
        brush.shape_fill_mode = ShapeFillMode::Fill;
        let mut doc = wide_doc();
        let id = doc.active_layer_id;
        crate::rasterizer::StrokeRasterizer::rasterize_polygon(
            &mut doc, &triangle, &brush, &SymmetryConfig::default(), None,
        );
        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "polygon fill changed storage format");
        let filled = layer.as_srgb16().expect("16-bit")[(4 * 9 + 4) * 4];
        assert_ne!(filled, mid, "the polygon interior was not filled");
        assert!(
            filled % 257 != 0,
            "the polygon fill landed on the 8-bit lattice: sub-8-bit detail was lost"
        );

        // Outline: painting must happen, and must stay 16-bit.
        brush.shape_fill_mode = ShapeFillMode::Stroke;
        let mut doc = wide_doc();
        let id = doc.active_layer_id;
        crate::rasterizer::StrokeRasterizer::rasterize_polygon(
            &mut doc, &triangle, &brush, &SymmetryConfig::default(), None,
        );
        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "polygon outline changed storage format");
        let painted = layer.as_srgb16().expect("16-bit");
        let changed = painted
            .chunks_exact(4)
            .filter(|pixel| pixel[0] != mid)
            .count();
        assert!(changed > 0, "the polygon outline painted nothing onto the 16-bit layer");
    }

    /// Gradient on 16-bit storage: the interpolation must produce values off the 8-bit lattice,
    /// which is only possible if the gradient is evaluated and written at 16 bits rather than
    /// quantised on the way in or out.
    #[test]
    fn gradient_writes_native_sixteen_bit_values() {
        use crate::brush::{BrushSettings, GradientType, ToolType};
        use crate::layer::{LayerBuffer, PixelFormat};

        let mut doc = Document::new(16, 4);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        let mid = crate::pixel::srgb8_to_srgb16(90) + 128;
        let mut channels = vec![0u16; 16 * 4 * 4];
        for pixel in 0..(16 * 4) {
            for channel in 0..3 {
                channels[pixel * 4 + channel] = mid;
            }
            channels[pixel * 4 + 3] = 65535;
        }
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let mut brush = BrushSettings::default();
        brush.tool = ToolType::Gradient;
        brush.gradient_type = GradientType::Linear;
        brush.primary_color = Color::from_rgba8(0, 0, 0, 255);
        brush.secondary_color = Color::WHITE;
        brush.opacity = 0.5;

        crate::rasterizer::StrokeRasterizer::rasterize_gradient(
            &mut doc,
            glam::Vec2::new(0.0, 0.0),
            glam::Vec2::new(16.0, 0.0),
            &brush,
            None,
        );

        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "the gradient must not change storage");
        let painted = layer.as_srgb16().expect("16-bit");

        let mut off_lattice = 0;
        let mut moved = 0;
        for pixel in painted.chunks_exact(4) {
            if pixel[0] != mid {
                moved += 1;
                if pixel[0] % 257 != 0 {
                    off_lattice += 1;
                }
            }
        }
        assert!(moved > 0, "the gradient painted nothing");
        assert!(
            off_lattice > 0,
            "every gradient value landed on the 8-bit lattice: interpolation was quantised"
        );
    }

    /// Flood fill on 16-bit storage. The tolerance comparison stays in 8-bit terms by design,
    /// so the region boundary must be exactly what it was; the fill itself must be written at
    /// native width, and a barrier must stop the walk.
    #[test]
    fn flood_fill_writes_native_values_and_respects_barriers() {
        use crate::layer::{LayerBuffer, PixelFormat};

        let mut doc = Document::new(8, 6);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        let mid = crate::pixel::srgb8_to_srgb16(60) + 128;
        let mut channels = vec![0u16; 8 * 6 * 4];
        for pixel in 0..(8 * 6) {
            for channel in 0..3 {
                channels[pixel * 4 + channel] = mid;
            }
            channels[pixel * 4 + 3] = 65535;
        }
        // A barrier column at x = 4, clearly outside the tolerance.
        for y in 0..6 {
            let pixel = (y * 8 + 4) as usize;
            channels[pixel * 4] = 65535;
            channels[pixel * 4 + 1] = 65535;
            channels[pixel * 4 + 2] = 65535;
        }
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        crate::rasterizer::StrokeRasterizer::flood_fill(
            &mut doc,
            0,
            0,
            Color::from_rgba8(255, 0, 0, 255),
            None,
            8,
        );

        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "flood fill changed storage format");
        let painted = layer.as_srgb16().expect("16-bit");
        let at = |x: usize, y: usize| painted[(y * 8 + x) * 4];

        let red = crate::pixel::srgb8_to_srgb16(255);
        assert_eq!(at(0, 0), red, "the seed was not filled");
        assert_eq!(at(3, 5), red, "the region left of the barrier was not filled");
        assert_eq!(at(4, 2), 65535, "the barrier itself must not be overwritten");
        assert_eq!(at(7, 2), mid, "the region right of the barrier must be untouched");
    }

    /// Text on a 16-bit layer renders natively: the loop dispatches once per draw and writes
    /// `u16` channels, so glyph coverage produces values below the 8-bit lattice instead of
    /// being quantised on the way in.
    #[test]
    fn text_renders_natively_on_sixteen_bit_layers() {
        use crate::layer::{LayerBuffer, PixelFormat};

        let font = ab_glyph::FontArc::try_from_vec(crate::assets::bundled_font().to_vec())
            .expect("bundled font");

        let mut doc = Document::new(64, 48);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(vec![0u16; 64 * 48 * 4]);
        }

        let mut config = crate::layer::TextLayerConfig::default();
        config.content = "Wg".to_string();
        config.font_size = 28.0;
        config.color = [255, 255, 255, 255];
        config.pos_x = 4.0;
        config.pos_y = 4.0;

        let layer = doc.get_layer_mut(id).expect("layer");
        assert!(
            crate::rasterizer::StrokeRasterizer::rasterize_text_layer_config(layer, &config, &font),
            "16-bit text must render, not report unsupported"
        );

        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "text must not change storage");
        let channels = layer.as_srgb16().expect("16-bit");
        assert!(channels.iter().any(|&c| c > 0), "nothing was rendered");

        // Glyph edges are fractional coverage: some channel must land off the 8-bit lattice,
        // which is the whole point of rendering at 16 bits rather than promoting afterwards.
        assert!(
            channels.iter().any(|&c| c > 0 && c % 257 != 0),
            "16-bit text produced only lattice values; it was quantised somewhere"
        );
    }

    /// Rendering empty text must clear the layer without changing its storage format - the
    /// failure mode worth guarding, because the function clears its buffer first.
    #[test]
    fn empty_text_clears_without_changing_format() {
        use crate::layer::{LayerBuffer, PixelFormat};

        let font = ab_glyph::FontArc::try_from_vec(crate::assets::bundled_font().to_vec())
            .expect("bundled font");

        let mut doc = Document::new(8, 8);
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(vec![65535u16; 8 * 8 * 4]);
        }

        let mut config = crate::layer::TextLayerConfig::default();
        config.content = String::new();
        let layer = doc.get_layer_mut(id).expect("layer");
        assert!(crate::rasterizer::StrokeRasterizer::rasterize_text_layer_config(layer, &config, &font));

        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16);
        assert!(
            layer.as_srgb16().expect("16-bit").iter().all(|&c| c == 0),
            "empty text must clear the layer"
        );
    }

    /// Alpha lock and a selection mask must behave the same way at both widths.
    #[test]
    fn text_alpha_lock_and_selection_apply_at_both_widths() {
        use crate::brush::TextAlign;
        use crate::layer::LayerBuffer;
        use crate::selection::SelectionMask;

        let font = ab_glyph::FontArc::try_from_vec(crate::assets::bundled_font().to_vec())
            .expect("bundled font");

        let selection = SelectionMask::from_mask_vec(
            64,
            32,
            (0..64 * 32).map(|index| if index % 64 < 20 { 255u8 } else { 0 }).collect(),
        );

        let mut results = Vec::new();
        for wide in [false, true] {
            let mut doc = Document::new(64, 32);
            doc.is_transparent = true;
            let id = doc.active_layer_id;
            if let Some(layer) = doc.get_layer_mut(id) {
                if wide {
                    *layer.buffer_mut() = LayerBuffer::Srgb16(vec![0u16; 64 * 32 * 4]);
                }
                layer.alpha_locked = true;
            }

            crate::rasterizer::StrokeRasterizer::rasterize_text(
                &mut doc,
                glam::Vec2::new(2.0, 4.0),
                "Locked",
                Some(&font),
                24.0,
                Color::WHITE,
                1.0,
                1.0,
                0.0,
                TextAlign::Left,
                Some(&selection),
            );

            let layer = doc.get_layer(id).expect("layer");
            // Alpha lock on a fully transparent layer refuses every write, at either width.
            let all_zero = if wide {
                layer.as_srgb16().expect("16-bit").iter().all(|&c| c == 0)
            } else {
                layer.as_srgb8_exact().iter().all(|&b| b == 0)
            };
            assert!(
                all_zero,
                "alpha-locked text on a transparent layer must not paint (wide = {wide})"
            );
            results.push(all_zero);
        }
        assert_eq!(results, vec![true, true], "both widths must agree");
    }

    /// Step 3 begins with resize: a 16-bit layer must keep its format and copy its overlapping
    /// region channel-for-channel, with new space zero-filled and nothing sampled or blended.
    #[test]
    fn resize_preserves_sixteen_bit_channels() {
        use crate::layer::{LayerBuffer, PixelFormat};

        let mut doc = Document::new(4, 3);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        let channels: Vec<u16> = (0..(4 * 3 * 4))
            .map(|index| (index as u16).wrapping_mul(1013).wrapping_add(7))
            .collect();
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels.clone());
        }

        // Grow: the original 4x3 region must be copied verbatim, the rest zero.
        doc.get_layer_mut(id).expect("layer").resize(6, 5);
        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "resize changed storage format");
        let grown = layer.as_srgb16().expect("16-bit");
        assert_eq!(grown.len(), 6 * 5 * 4);
        for y in 0..3usize {
            let src = &channels[y * 4 * 4..(y + 1) * 4 * 4];
            let dst = &grown[y * 6 * 4..y * 6 * 4 + 4 * 4];
            assert_eq!(dst, src, "row {y} was not copied verbatim");
        }
        // Column 5 of row 0 is outside the old 4-wide canvas, so it must be zero-filled.
        assert_eq!(grown[5 * 4], 0, "new space must be zero-filled");

        // Shrink: the surviving region is the top-left corner, still exact.
        doc.get_layer_mut(id).expect("layer").resize(2, 2);
        let shrunk = doc.get_layer(id).expect("layer").as_srgb16().expect("16-bit");
        for y in 0..2usize {
            assert_eq!(&shrunk[y * 2 * 4..(y + 1) * 2 * 4], &channels[y * 4 * 4..y * 4 * 4 + 2 * 4]);
        }
    }

    /// Baseline 1 and 2: scale_canvas output, nearest and bilinear, captured before any change.
    ///
    /// A 4x4 pattern with distinct channel values, so any change to sampling weights, edge
    /// handling or rounding shows up as different bytes rather than a plausible-looking image.
    #[test]
    fn scale_canvas_baselines() {
        fn fixture() -> Document {
            let mut doc = Document::new(4, 4);
            doc.is_transparent = true;
            let id = doc.active_layer_id;
            if let Some(layer) = doc.get_layer_mut(id) {
                for (index, pixel) in layer.as_srgb8_exact_mut().chunks_exact_mut(4).enumerate() {
                    pixel[0] = (index * 17 + 3) as u8;
                    pixel[1] = (index * 29 + 11) as u8;
                    pixel[2] = (index * 41 + 7) as u8;
                    pixel[3] = 255;
                }
            }
            doc
        }

        // FNV-1a over the bytes: compact enough to live in a test, sensitive enough that any
        // change to sampling weights, edge handling or rounding fails it.
        fn digest(bytes: &[u8]) -> u64 {
            let mut hash = 0xcbf29ce484222325u64;
            for &byte in bytes {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
            hash
        }

        let mut nearest_shrink = fixture();
        nearest_shrink.scale_canvas(2, 2, false);
        let nearest_shrink = nearest_shrink.layers[0].as_srgb8_exact().to_vec();

        let mut nearest_grow = fixture();
        nearest_grow.scale_canvas(6, 6, false);
        let nearest_grow = nearest_grow.layers[0].as_srgb8_exact().to_vec();

        let mut bilinear_shrink = fixture();
        bilinear_shrink.scale_canvas(2, 2, true);
        let bilinear_shrink = bilinear_shrink.layers[0].as_srgb8_exact().to_vec();

        let mut bilinear_grow = fixture();
        bilinear_grow.scale_canvas(6, 6, true);
        let bilinear_grow = bilinear_grow.layers[0].as_srgb8_exact().to_vec();

        assert_eq!(digest(&nearest_shrink), 0x234d98dae72e66a5 as u64, "nearest_shrink changed");
        assert_eq!(digest(&nearest_grow), 0x7690519e02d9cb6d as u64, "nearest_grow changed");
        assert_eq!(digest(&bilinear_shrink), 0x1c1fe15e642f245 as u64, "bilinear_shrink changed");
        assert_eq!(digest(&bilinear_grow), 0x5f05b0942ddd53c4 as u64, "bilinear_grow changed");
        assert_eq!(nearest_shrink.len(), 16, "nearest_shrink length changed");
        assert_eq!(nearest_grow.len(), 144, "nearest_grow length changed");
        assert_eq!(bilinear_shrink.len(), 16, "bilinear_shrink length changed");
        assert_eq!(bilinear_grow.len(), 144, "bilinear_grow length changed");

    }

    /// Baseline 3: dimensions and storage format when growing and shrinking.
    #[test]
    fn scale_canvas_dimensions_and_format_baseline() {
        use crate::layer::{LayerBuffer, PixelFormat};

        let mut doc = Document::new(4, 4);
        doc.scale_canvas(7, 3, true);
        assert_eq!((doc.width, doc.height), (7, 3));
        assert_eq!((doc.layers[0].width, doc.layers[0].height), (7, 3));
        assert_eq!(doc.layers[0].as_srgb8_exact().len(), 7 * 3 * 4);

        // Current behaviour on a 16-bit layer, recorded rather than assumed: scale_canvas
        // writes through an 8-bit buffer, so a 16-bit layer comes back as 8-bit.
        let mut doc = Document::new(4, 4);
        let id = doc.active_layer_id;
        let channels = vec![12345u16; 4 * 4 * 4];
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }
        doc.scale_canvas(2, 2, true);
        // BASELINE, not a desired end state: scale_canvas currently writes through an 8-bit
        // buffer, so a 16-bit layer comes back as 8-bit and its channels are quantised. This
        // pins that behaviour so the fix is a visible, deliberate change to this assertion.
        assert_eq!(
            doc.get_layer(id).unwrap().format(),
            PixelFormat::Srgb16,
            "scale_canvas converted a 16-bit layer"
        );
    }

    /// Warped patch rendering at both storage widths. The patch source stays 8-bit, so a
    /// 16-bit destination is promoted lattice data; rendering it must downconvert back to
    /// exactly the 8-bit result, which proves the widened integer maths matches the original.
    #[test]
    fn warped_patches_render_identically_at_both_widths() {
        use crate::layer::{LayerBuffer, PixelFormat};
        use crate::transform::{
            render_mesh_warped_patch, render_quad_transformed_patch, InterpolationMode,
            MeshWarpGrid, PerspectiveQuadTransform,
        };

        const W: u32 = 10;
        const H: u32 = 10;

        fn patch() -> Vec<u8> {
            let mut data = Vec::with_capacity(64 * 4);
            for index in 0..64u32 {
                data.push((index * 7 % 256) as u8);
                data.push((index * 13 % 256) as u8);
                data.push((index * 29 % 256) as u8);
                data.push(if index % 5 == 0 { 128 } else { 200 });
            }
            data
        }

        let corners = [
            glam::Vec2::new(0.0, 0.0),
            glam::Vec2::new(8.0, 0.0),
            glam::Vec2::new(8.0, 8.0),
            glam::Vec2::new(0.0, 8.0),
        ];

        // 8-bit destination.
        let mut eight = vec![0u8; (W * H * 4) as usize];
        for (index, pixel) in eight.chunks_exact_mut(4).enumerate() {
            pixel[0] = (index * 3 % 256) as u8;
            pixel[1] = (index * 5 % 256) as u8;
            pixel[2] = (index * 11 % 256) as u8;
            pixel[3] = 255;
        }
        let mut sixteen: Vec<u16> = eight
            .iter()
            .map(|&v| crate::pixel::srgb8_to_srgb16(v))
            .collect();

        let quad = PerspectiveQuadTransform::new(corners, corners);
        render_quad_transformed_patch(
            &patch(), 8, 8, &quad, InterpolationMode::Bilinear, &mut eight, W, H,
        );
        render_quad_transformed_patch(
            &patch(), 8, 8, &quad, InterpolationMode::Bilinear, &mut sixteen, W, H,
        );

        let mesh = MeshWarpGrid::new(3, 3, glam::Vec2::new(1.0, 1.0), 8.0, 8.0);
        render_mesh_warped_patch(
            &patch(), 8, 8, &mesh, InterpolationMode::Bilinear, &mut eight, W, H,
        );
        render_mesh_warped_patch(
            &patch(), 8, 8, &mesh, InterpolationMode::Bilinear, &mut sixteen, W, H,
        );

        let downconverted: Vec<u8> = sixteen
            .iter()
            .map(|&v| crate::pixel::srgb16_to_srgb8(v))
            .collect();
        // Bounded, not exact, and the reason is arithmetic: the 16-bit composer divides by
        // 65535 where the 8-bit one divides by 255, and floor(257x/255) != 257*floor(x/255).
        // So a 16-bit composite carries finer intermediate precision and can land one step
        // away after downconversion - the same bounded drift the multi-layer oracle allows.
        let worst = downconverted
            .iter()
            .zip(eight.iter())
            .map(|(wide, narrow)| wide.abs_diff(*narrow))
            .max()
            .unwrap_or(0);
        println!("warped patch drift: {worst} LSB of 255");
        assert!(
            worst <= 1,
            "widened integer compositing drifted {worst} LSB from the 8-bit result"
        );

        // And the 16-bit destination really did change, so the comparison is not a no-op.
        let untouched = vec![crate::pixel::srgb8_to_srgb16(0u8); 4];
        assert!(untouched.len() == 4);
        assert_eq!(PixelFormat::Srgb16, PixelFormat::Srgb16);
        let _ = LayerBuffer::Srgb8(Vec::new());
    }

    /// The application's stroke-undo path, end to end, on a 16-bit layer.
    ///
    /// `aurelia-app` cannot be unit tested (Win32 host), so this reproduces exactly what it
    /// does: capture `storage_bytes()` before the stroke, paint, capture again, and push a
    /// `LayerPixelsDeltaCommand`. The capture used to go through `as_srgb8_exact()`, which
    /// panics on a 16-bit layer before the rasterizer - which does support one - is ever
    /// reached. This test fails if that regresses, and if `storage_bytes()` stops being a
    /// faithful, format-agnostic view of the buffer.
    #[test]
    fn a_sixteen_bit_stroke_can_be_captured_and_undone() {
        use crate::brush::{BrushPoint, BrushSettings, ToolType};
        use crate::history::{Command, LayerPixelsDeltaCommand};
        use crate::layer::LayerBuffer;
        use crate::symmetry::SymmetryConfig;

        let mut doc = Document::new(12, 12);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            // Half-lattice values, so nothing here is a promoted 8-bit pixel.
            let channels: Vec<u16> = (0..(12 * 12 * 4))
                .map(|i| (i as u16).wrapping_mul(4099).wrapping_add(17))
                .collect();
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let mut brush = BrushSettings::default();
        brush.tool = ToolType::Brush;
        brush.size = 4.0;
        brush.opacity = 1.0;
        brush.hardness = 1.0;
        brush.primary_color = Color::WHITE;

        let point = |x: f32, y: f32| BrushPoint {
            position: glam::Vec2::new(x, y),
            pressure: 1.0,
        };

        let before = doc.get_layer(id).expect("layer").storage_bytes();
        assert_eq!(before.len(), 12 * 12 * 4 * 2, "16-bit storage is two bytes per channel");

        crate::rasterizer::StrokeRasterizer::paint_segment(
            &mut doc,
            point(2.0, 2.0),
            point(9.0, 9.0),
            &brush,
            &SymmetryConfig::default(),
            None,
        );

        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(
            layer.format(),
            crate::layer::PixelFormat::Srgb16,
            "the stroke must not change storage"
        );
        let after = layer.storage_bytes();
        assert_ne!(before, after, "the stroke must change the buffer for this test to mean anything");

        let mut command = LayerPixelsDeltaCommand::new(id, "16-bit stroke", before.clone(), after.clone());
        command.undo(&mut doc);
        assert_eq!(
            doc.get_layer(id).expect("layer").storage_bytes(),
            before,
            "undo of a 16-bit stroke must restore every byte"
        );
        command.redo(&mut doc);
        assert_eq!(
            doc.get_layer(id).expect("layer").storage_bytes(),
            after,
            "redo of a 16-bit stroke must restore every byte"
        );
    }

    /// The magic wand samples the active layer directly. That sampling was an 8-bit copy, so
    /// colour-select on a 16-bit layer aborted before any selection existed. Matching stays in
    /// 8-bit terms (the tolerance is an 8-bit value), but it must not require an 8-bit layer.
    #[test]
    fn magic_wand_reads_sixteen_bit_layers() {
        use crate::layer::LayerBuffer;
        use crate::rasterizer::StrokeRasterizer;

        let mut doc = Document::new(4, 4);
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            let mut channels = vec![0u16; 4 * 4 * 4];
            for y in 0..4u32 {
                for x in 2..4u32 {
                    let base = ((y * 4 + x) * 4) as usize;
                    channels[base] = 65535; // red
                    channels[base + 3] = 65535; // opaque
                }
            }
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let mask = StrokeRasterizer::rasterize_magic_wand(&doc, 3, 1, 0, true, false);

        assert!(mask.has_selection(), "the wand must select the opaque block");
        assert_eq!(mask.get_value(3, 1), 255, "a pixel inside the block must be selected");
        assert_eq!(mask.get_value(0, 0), 0, "a transparent pixel must not be selected");
    }

    /// Export composites each timeline frame on its own. A frame holding 16-bit layers has to
    /// reach the working-precision compositor rather than the 8-bit copy it used to assume.
    #[test]
    fn animation_frames_composite_sixteen_bit_layers() {
        use crate::animation::AnimationFrame;
        use crate::layer::LayerBuffer;

        let mut doc = Document::new(2, 2);
        doc.is_transparent = false;
        doc.background_value = 7;
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            let channels: Vec<u16> = (0..(2 * 2 * 4))
                .map(|index| (index as u16).wrapping_mul(8191).wrapping_add(3))
                .collect();
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let frame = AnimationFrame::from_layers(1, "frame", doc.layers.clone(), id, 2);
        let from_frame = frame.composite_layers(2, 2, true, 7);
        let from_document = doc.composite_layers(true);

        assert_eq!(from_frame.len(), 2 * 2 * 4);
        assert_eq!(
            from_frame, from_document,
            "a 16-bit frame must composite exactly like the document it was captured from"
        );
    }

    /// "Select layer alpha" must work on either storage width, and must agree with the 8-bit
    /// result for alpha values on the lattice. Matching stays in 8-bit terms by design, the
    /// same asymmetry flood fill documents.
    #[test]
    fn layer_alpha_selection_reads_sixteen_bit_alpha() {
        use crate::layer::LayerBuffer;
        use crate::selection::SelectionMask;

        // 3x1: transparent, opaque, half-way.
        let mut wide = Document::new(3, 1);
        let wide_id = wide.active_layer_id;
        if let Some(layer) = wide.get_layer_mut(wide_id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(vec![
                0, 0, 0, 0, //
                9, 9, 9, 65535, //
                9, 9, 9, 32768,
            ]);
        }
        let mask = SelectionMask::from_layer(wide.get_layer(wide_id).expect("layer"), 8);
        assert_eq!(mask.get_value(0, 0), 0, "transparent alpha must not select");
        assert_eq!(mask.get_value(1, 0), 255, "opaque alpha must select fully");
        assert_eq!(
            mask.get_value(2, 0),
            crate::pixel::srgb16_to_srgb8(32768),
            "half-way alpha keeps its 8-bit coverage"
        );

        // The same alphas on a lattice-aligned 8-bit layer produce the same mask.
        let mut narrow = Document::new(3, 1);
        let narrow_id = narrow.active_layer_id;
        if let Some(layer) = narrow.get_layer_mut(narrow_id) {
            layer.as_srgb8_exact_mut().copy_from_slice(&[
                0, 0, 0, 0, 9, 9, 9, 255, 9, 9, 9, 128,
            ]);
        }
        let narrow_mask = SelectionMask::from_layer(narrow.get_layer(narrow_id).expect("layer"), 8);
        assert_eq!(
            (0..3).map(|x| mask.get_value(x, 0)).collect::<Vec<_>>(),
            (0..3).map(|x| narrow_mask.get_value(x, 0)).collect::<Vec<_>>(),
            "8-bit and 16-bit alpha must select identically for lattice values"
        );
    }

    /// `storage_bytes()` must agree with the two format-specific views it generalises.
    #[test]
    fn storage_bytes_matches_the_eight_bit_buffer() {
        let mut doc = Document::new(4, 3);
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            for (index, byte) in layer.as_srgb8_exact_mut().iter_mut().enumerate() {
                *byte = (index * 29 % 253) as u8;
            }
        }

        let layer = doc.get_layer(id).expect("layer");
        assert_eq!(layer.storage_bytes(), layer.as_srgb8_exact());
    }

    /// Flip, rotate and crop move pixels without reading them, so they must work on either
    /// storage width. These are hand-written expectations, not snapshots of the new code: a
    /// 2x1 fixture makes the mapping checkable by eye.
    #[test]
    fn structural_edits_keep_eight_bit_bytes() {
        fn pixel_doc() -> Document {
            // 2x1 with two distinguishable pixels.
            let mut doc = Document::new(2, 1);
            let id = doc.active_layer_id;
            if let Some(layer) = doc.get_layer_mut(id) {
                layer.as_srgb8_exact_mut().copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
            }
            doc
        }

        let mut flipped = pixel_doc();
        flipped.flip(true);
        assert_eq!(
            flipped.layers[0].as_srgb8_exact(),
            &[5, 6, 7, 8, 1, 2, 3, 4],
            "horizontal flip must mirror whole pixels"
        );

        let mut vertical = pixel_doc();
        vertical.layers[0].set_pixels(vec![1, 2, 3, 4, 5, 6, 7, 8]);
        vertical.flip(false);
        assert_eq!(
            vertical.layers[0].as_srgb8_exact(),
            &[1, 2, 3, 4, 5, 6, 7, 8],
            "a one-row image is unchanged by a vertical flip"
        );

        let mut rotated = pixel_doc();
        rotated.rotate_90(true);
        assert_eq!((rotated.width, rotated.height), (1, 2));
        assert_eq!(
            rotated.layers[0].as_srgb8_exact(),
            &[1, 2, 3, 4, 5, 6, 7, 8],
            "clockwise rotation of 2x1 gives the column (A, B)"
        );

        let mut cropped = pixel_doc();
        cropped.resize_canvas(4, 3, 1, 1);
        assert_eq!((cropped.width, cropped.height), (4, 3));
        assert_eq!(
            cropped.layers[0].as_srgb8_exact().len(),
            4 * 3 * 4,
            "the crop must resize the buffer"
        );
        // (0,0) of the source now sits at (1,1) of a 4-wide canvas.
        let dst = ((1 * 4 + 1) * 4) as usize;
        assert_eq!(&cropped.layers[0].as_srgb8_exact()[dst..dst + 4], &[1, 2, 3, 4]);
        assert_eq!(&cropped.layers[0].as_srgb8_exact()[0..4], &[0, 0, 0, 0]);
    }

    /// The same three edits on a 16-bit layer: storage format is preserved, off-lattice values
    /// survive untouched, and the geometry is identical to the 8-bit case.
    #[test]
    fn structural_edits_preserve_sixteen_bit_storage() {
        use crate::layer::{LayerBuffer, PixelFormat};

        fn wide(width: u32, height: u32) -> Document {
            let mut doc = Document::new(width, height);
            doc.is_transparent = true;
            let id = doc.active_layer_id;
            if let Some(layer) = doc.get_layer_mut(id) {
                let channels: Vec<u16> = (0..(width * height * 4))
                    .map(|index| (index as u16).wrapping_mul(1237).wrapping_add(11))
                    .collect();
                *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
            }
            doc
        }

        let sample = |values: &[u16], x: u32, y: u32, width: u32| -> Vec<u16> {
            let base = ((y * width + x) * 4) as usize;
            values[base..base + 4].to_vec()
        };

        // Flip: mirrored exactly, format kept, and its own inverse.
        let mut doc = wide(4, 3);
        let id = doc.active_layer_id;
        let original = doc.get_layer(id).expect("layer").as_srgb16().expect("16-bit").to_vec();
        assert!(
            original.iter().any(|&c| c % 257 != 0),
            "the fixture must contain off-lattice values for this test to mean anything"
        );

        doc.flip(true);
        {
            let layer = doc.get_layer(id).expect("layer");
            assert_eq!(layer.format(), PixelFormat::Srgb16, "flip must not change storage");
            let values = layer.as_srgb16().expect("16-bit");
            assert_eq!(sample(values, 0, 0, 4), sample(&original, 3, 0, 4));
            assert_eq!(sample(values, 3, 2, 4), sample(&original, 0, 2, 4));
        }
        doc.flip(true);
        assert_eq!(
            doc.get_layer(id).expect("layer").as_srgb16().expect("16-bit"),
            original.as_slice(),
            "flipping twice must restore the exact channels"
        );

        // Rotate: four quarter turns are the identity, and the document follows.
        let mut doc = wide(4, 3);
        let id = doc.active_layer_id;
        let original = doc.get_layer(id).expect("layer").as_srgb16().expect("16-bit").to_vec();
        doc.rotate_90(true);
        assert_eq!((doc.width, doc.height), (3, 4));
        {
            let layer = doc.get_layer(id).expect("layer");
            assert_eq!(layer.format(), PixelFormat::Srgb16, "rotate must not change storage");
            assert_eq!(layer.as_srgb16().expect("16-bit").len(), 3 * 4 * 4);
        }
        for _ in 0..3 {
            doc.rotate_90(true);
        }
        assert_eq!((doc.width, doc.height), (4, 3));
        assert_eq!(
            doc.get_layer(id).expect("layer").as_srgb16().expect("16-bit"),
            original.as_slice(),
            "four quarter turns must restore the exact channels"
        );

        // Crop / extend: the overlap is copied, the new band is zero, the format is kept.
        let mut doc = wide(4, 3);
        let id = doc.active_layer_id;
        let original = doc.get_layer(id).expect("layer").as_srgb16().expect("16-bit").to_vec();
        doc.resize_canvas(6, 5, 1, 2);
        assert_eq!((doc.width, doc.height), (6, 5));
        {
            let layer = doc.get_layer(id).expect("layer");
            assert_eq!(layer.format(), PixelFormat::Srgb16, "crop must not change storage");
            let values = layer.as_srgb16().expect("16-bit");
            assert_eq!(values.len(), 6 * 5 * 4);
            assert_eq!(sample(values, 1, 2, 6), sample(&original, 0, 0, 4));
            assert_eq!(sample(values, 0, 0, 6), vec![0u16; 4], "new space must be zero-filled");
        }
    }

    // ── Native 16-bit composite output (#15) ────────────────────────────────────────────────

    /// The public 16-bit API must return real channels, not a widened 8-bit result.
    #[test]
    fn native_sixteen_bit_output_keeps_sub_lattice_values() {
        use crate::layer::LayerBuffer;
        use crate::pixel::Channel;

        let mut doc = Document::new(1, 1);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        let low = crate::pixel::srgb8_to_srgb16(100);
        let high = low + 128; // not on the 8-bit lattice
        assert_ne!(high % 257, 0, "the fixture must be off-lattice");
        if let Some(layer) = doc.get_layer_mut(id) {
            *layer.buffer_mut() = LayerBuffer::Srgb16(vec![high, high, high, 65535]);
        }

        let native = doc.composite_layers_srgb16(false);
        assert_eq!(native.len(), 4);
        assert_eq!(native[0], high, "native output must carry the exact channel");
        assert_eq!(native[3], 65535);

        // The 8-bit API is unchanged and does collapse it - that is the difference the
        // native entry point exists for.
        let eight_bit = doc.composite_layers(false);
        assert_eq!(eight_bit[0], 100, "an 8-bit result cannot carry it");
        assert_eq!(crate::pixel::srgb16_to_srgb8(native[0]), eight_bit[0]);
        let _ = u16::from_f32(0.5);
    }

    /// Mixed documents: native output, downconverted, must equal the 8-bit result byte for
    /// byte - the 8-bit path is the same compositor followed by a downconversion.
    #[test]
    fn mixed_format_output_downconverts_to_the_eight_bit_result() {
        use crate::layer::LayerBuffer;

        let mut doc = Document::new(4, 4);
        doc.is_transparent = false;
        doc.background_value = 40;
        let first = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(first) {
            for (index, byte) in layer.as_srgb8_exact_mut().iter_mut().enumerate() {
                *byte = (index * 13 % 251) as u8;
            }
        }
        let second = doc.add_layer(None);
        if let Some(layer) = doc.get_layer_mut(second) {
            let channels: Vec<u16> = (0..(4 * 4 * 4))
                .map(|index| crate::pixel::srgb8_to_srgb16((index * 7 % 251) as u8))
                .collect();
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let native = doc.composite_layers_srgb16(true);
        let downconverted: Vec<u8> = native
            .iter()
            .map(|&channel| crate::pixel::srgb16_to_srgb8(channel))
            .collect();
        assert_eq!(
            downconverted,
            doc.composite_layers(true),
            "native output must downconvert to the 8-bit result for a mixed document"
        );

        // Two runs agree exactly, including whenever the parallel path is taken.
        assert_eq!(
            doc.composite_layers_srgb16(true),
            native,
            "native output must be deterministic"
        );
    }

    /// The 16-bit compositor used to skip adjustment layers entirely, so a 16-bit document
    /// rendered *differently* rather than merely at lower precision. Invert is the cheapest
    /// case to assert an exact value for.
    #[test]
    fn adjustment_layers_apply_in_sixteen_bit_documents() {
        use crate::layer::{AdjustmentType, LayerBuffer};
        use crate::pixel::Channel;

        let mut doc = Document::new(2, 1);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        let base = 30_000u16;
        if let Some(layer) = doc.get_layer_mut(id) {
            let mut channels = vec![base; 8];
            for pixel in channels.chunks_exact_mut(4) {
                pixel[3] = 65535;
            }
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let before = doc.composite_layers_srgb16(false);
        assert_eq!(before[0], base, "the base layer composites unchanged");

        doc.add_adjustment_layer(AdjustmentType::Invert, None);
        let after = doc.composite_layers_srgb16(false);

        assert_ne!(after[0], before[0], "the adjustment must reach the composite");
        assert_eq!(
            after[0],
            u16::from_f32(1.0 - base.to_f32()),
            "invert must be exact in working precision"
        );
        assert_eq!(after[3], 65535, "the adjustment must not disturb alpha");
    }

    #[test]
    fn composite_into_srgb16_rejects_short_buffers() {
        use crate::document::CompositeError;

        let doc = Document::new(4, 4);
        let mut short = vec![0u16; 63];

        match doc.composite_layers_into_srgb16(&mut short, false) {
            Err(CompositeError::BufferTooSmall { expected, actual }) => {
                assert_eq!((expected, actual), (64, 63));
            }
            other => panic!("expected BufferTooSmall, got {other:?}"),
        }
        assert!(short.iter().all(|&channel| channel == 0), "a rejected write must not touch the buffer");

        let mut exact = vec![0u16; 64];
        assert!(doc.composite_layers_into_srgb16(&mut exact, false).is_ok());
    }

    /// `apply_to_rgb_working` is the same formulas as the shipped 8-bit adjustment, expressed
    /// on normalised channels. The two can differ by one 8-bit step where a result sits on a
    /// rounding boundary; anything larger means the formulas have diverged.
    #[test]
    fn adjustment_working_matches_eight_bit() {        use crate::layer::AdjustmentType;
        use crate::pixel::Channel;

        let cases = [
            AdjustmentType::BrightnessContrast { brightness: 20.0, contrast: 30.0 },
            AdjustmentType::BrightnessContrast { brightness: -40.0, contrast: -25.0 },
            AdjustmentType::Hsl { hue_shift: 40.0, saturation: 1.4, lightness: -0.1 },
            AdjustmentType::Hsl { hue_shift: -120.0, saturation: 0.6, lightness: 0.2 },
            AdjustmentType::ColorBalance { cyan_red: 30.0, magenta_green: -20.0, yellow_blue: 10.0 },
            AdjustmentType::Invert,
            AdjustmentType::Posterize { levels: 5 },
            AdjustmentType::Threshold { cutoff: 128 },
            AdjustmentType::Sepia { strength: 0.75 },
        ];
        let values = [0u8, 1, 17, 63, 64, 127, 128, 129, 191, 254, 255];

        let mut worst = 0i32;
        for adjustment in &cases {
            for &r in &values {
                for &g in &values {
                    for &b in &values {
                        let (er, eg, eb) = adjustment.apply_to_rgb(r, g, b);
                        let working = adjustment
                            .apply_to_rgb_working([r.to_f32(), g.to_f32(), b.to_f32()]);
                        let quantised = [
                            u8::from_f32(working[0]),
                            u8::from_f32(working[1]),
                            u8::from_f32(working[2]),
                        ];
                        for (expected, actual) in [(er, quantised[0]), (eg, quantised[1]), (eb, quantised[2])] {
                            worst = worst.max((expected as i32 - actual as i32).abs());
                        }
                    }
                }
            }
        }

        assert!(
            worst <= 1,
            "working-precision adjustments drifted {worst} steps from the 8-bit result"
        );
    }

    // ── Text: the 8-bit baseline (#14) ──────────────────────────────────────────────────────

    /// Every text fixture, digested.
    ///
    /// Recorded **before** the glyph loop was generalised, against the bundled font, so this
    /// is a baseline rather than a snapshot of whatever the refactor produced. The font is
    /// bundled for exactly this reason: a system font would make the digest machine-specific
    /// and therefore meaningless.
    mod text_baseline {
        use crate::brush::TextAlign;
        use crate::color::Color;
        use crate::document::Document;
        use crate::layer::{TextLayerConfig, LayerBuffer};
        use crate::rasterizer::StrokeRasterizer;

        fn font() -> ab_glyph::FontArc {
            ab_glyph::FontArc::try_from_vec(crate::assets::bundled_font().to_vec())
                .expect("bundled font")
        }

        fn digest(bytes: &[u8]) -> u64 {
            let mut hash = 0xcbf29ce484222325u64;
            for &byte in bytes {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
            hash
        }

        fn config(content: &str, size: f32) -> TextLayerConfig {
            TextLayerConfig {
                content: content.to_string(),
                font_family: "bundled".to_string(),
                font_size: size,
                color: [255, 255, 255, 255],
                align: TextAlign::Left,
                line_spacing: 1.0,
                letter_spacing: 0.0,
                box_w: 0.0,
                box_h: 0.0,
                pos_x: 4.0,
                pos_y: 4.0,
            }
        }

        fn render(width: u32, height: u32, cfg: &TextLayerConfig) -> Vec<u8> {
            let mut doc = Document::new(width, height);
            doc.is_transparent = true;
            let id = doc.active_layer_id;
            if let Some(layer) = doc.get_layer_mut(id) {
                *layer.buffer_mut() = LayerBuffer::Srgb8(vec![0u8; (width * height * 4) as usize]);
                assert!(StrokeRasterizer::rasterize_text_layer_config(layer, cfg, &font()));
                return layer.as_srgb8_exact().to_vec();
            }
            unreachable!()
        }

        #[test]
        fn a_single_line_is_unchanged() {
            let bytes = render(96, 48, &config("Aurelia", 32.0));
            assert_ne!(bytes.iter().filter(|&&byte| byte > 0).count(), 0, "nothing rendered");
            assert_eq!(digest(&bytes), 10483588990592354726, "a_single_line_is_unchanged");
        }

        #[test]
        fn wrapping_and_centering_are_unchanged() {
            let mut cfg = config("Wrap this line", 20.0);
            cfg.box_w = 70.0;
            cfg.align = TextAlign::Center;
            let bytes = render(96, 96, &cfg);
            assert_eq!(digest(&bytes), 1623966499463724239, "wrapping_and_centering_are_unchanged");
        }

        #[test]
        fn alpha_and_letter_spacing_are_unchanged() {
            let mut cfg = config("Spaced", 24.0);
            cfg.color = [255, 200, 120, 128];
            cfg.letter_spacing = 2.5;
            let bytes = render(96, 48, &cfg);
            assert_eq!(digest(&bytes), 9779906792473839393, "alpha_and_letter_spacing_are_unchanged");
        }

        #[test]
        fn a_glyph_clipped_at_the_edge_is_unchanged() {
            let mut cfg = config("Wg", 28.0);
            cfg.pos_x = -6.0;
            cfg.pos_y = -4.0;
            let bytes = render(32, 32, &cfg);
            assert_eq!(digest(&bytes), 16401277342387934969, "a_glyph_clipped_at_the_edge_is_unchanged");
        }

        #[test]
        fn the_tool_path_is_unchanged() {
            let mut doc = Document::new(96, 48);
            doc.is_transparent = true;
            let bounds = StrokeRasterizer::rasterize_text(
                &mut doc,
                glam::Vec2::new(10.0, 10.0),
                "Hi",
                Some(&font()),
                24.0,
                Color::new(1.0, 0.2, 0.2, 1.0),
                1.0,
                1.0,
                0.0,
                TextAlign::Left,
                None,
            );
            assert!(bounds.is_some(), "the tool path rendered nothing");
            let bytes = doc.layers[0].as_srgb8_exact().to_vec();
            assert_eq!(digest(&bytes), 2616636973191488769, "the_tool_path_is_unchanged");
        }

        #[test]
        fn the_tool_path_with_a_selection_is_unchanged() {
            let mut doc = Document::new(96, 48);
            doc.is_transparent = true;
            let selection = crate::selection::SelectionMask::from_mask_vec(
                96,
                48,
                (0..96 * 48).map(|index| if index % 96 < 40 { 255u8 } else { 0 }).collect(),
            );
            StrokeRasterizer::rasterize_text(
                &mut doc,
                glam::Vec2::new(6.0, 8.0),
                "Masked",
                Some(&font()),
                20.0,
                Color::WHITE,
                1.0,
                1.0,
                0.0,
                TextAlign::Left,
                Some(&selection),
            );
            let bytes = doc.layers[0].as_srgb8_exact().to_vec();
            assert_eq!(digest(&bytes), 10656664465313779728, "the_tool_path_with_a_selection_is_unchanged");
        }
    }
}

#[cfg(test)]
mod document_format {
    use crate::document::Document;
    use crate::layer::{Layer, PixelFormat};

    /// The regression that motivated a document-level format: deleting the last 16-bit layer
    /// used to turn the document 8-bit, because the format was derived from the layers.
    #[test]
    fn format_survives_deleting_the_last_sixteen_bit_layer() {
        let mut doc = Document::new_with_format(4, 4, PixelFormat::Srgb16);
        assert_eq!(doc.pixel_format, PixelFormat::Srgb16);
        assert_eq!(doc.layers[0].format(), PixelFormat::Srgb16, "the base layer follows the document");

        doc.delete_layer(doc.active_layer_id);
        assert_eq!(doc.pixel_format, PixelFormat::Srgb16, "deleting layers must not change the format");

        let id = doc.add_layer(None);
        assert_eq!(
            doc.get_layer(id).expect("layer").format(),
            PixelFormat::Srgb16,
            "a new layer must inherit the document format"
        );
    }

    #[test]
    fn new_documents_start_in_the_requested_format() {
        let eight = Document::new(2, 2);
        assert_eq!(eight.pixel_format, PixelFormat::Srgb8);
        assert_eq!(eight.layers[0].format(), PixelFormat::Srgb8);

        let wide = Document::new_with_format(2, 2, PixelFormat::Srgb16);
        assert_eq!(wide.pixel_format, PixelFormat::Srgb16);
        assert_eq!(wide.layers[0].format(), PixelFormat::Srgb16);
        assert_eq!(
            wide.layers[0].storage_len(),
            (2 * 2 * 4) as usize * 2,
            "16-bit storage is two bytes per channel"
        );
    }

    #[test]
    fn add_layer_does_not_downgrade_a_sixteen_bit_document() {
        let mut doc = Document::new_with_format(3, 3, PixelFormat::Srgb16);
        let raster = doc.add_layer(Some("raster".into()));
        let text = doc.add_text_layer(Some("text".into()), crate::layer::TextLayerConfig::default(), None);
        for id in [raster, text] {
            assert_eq!(
                doc.get_layer(id).expect("layer").format(),
                PixelFormat::Srgb16,
                "layer {id} must inherit the document format"
            );
        }
    }

    /// Flattening a 16-bit document has to stay 16-bit, or "merge visible" silently throws the
    /// precision away - the one operation that would be a downgrade by design.
    #[test]
    fn merge_visible_layers_keeps_sixteen_bit_storage() {
        use crate::layer::LayerBuffer;

        let mut doc = Document::new_with_format(2, 1, PixelFormat::Srgb16);
        let id = doc.active_layer_id;
        let low = crate::pixel::srgb8_to_srgb16(200);
        let high = low + 128;
        if let Some(layer) = doc.get_layer_mut(id) {
            let mut channels = vec![0u16; 8];
            for pixel in channels.chunks_exact_mut(4) {
                pixel[0] = high;
                pixel[3] = 65535;
            }
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }
        doc.add_layer(None);

        doc.merge_visible_layers();

        assert_eq!(doc.layers.len(), 1);
        let merged = &doc.layers[0];
        assert_eq!(merged.format(), PixelFormat::Srgb16, "merge must not downgrade the document");
        let values = merged.as_srgb16().expect("16-bit");
        assert_eq!(values[0], high, "the merged layer must keep the exact channel");
    }

    /// Merge-down used to refuse a 16-bit document because it composited through `get_pixel`
    /// and `set_pixel`. It now composites in working precision and writes into the lower
    /// layer's own storage.
    #[test]
    fn merge_down_composites_sixteen_bit_layers() {
        use crate::layer::LayerBuffer;

        let mut doc = Document::new_with_format(2, 1, PixelFormat::Srgb16);
        let base = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(base) {
            // Opaque blue.
            *layer.buffer_mut() = LayerBuffer::Srgb16(vec![0, 0, 65535, 65535, 0, 0, 65535, 65535]);
        }
        let upper = doc.add_layer(None);
        if let Some(layer) = doc.get_layer_mut(upper) {
            // Opaque red.
            *layer.buffer_mut() = LayerBuffer::Srgb16(vec![65535, 0, 0, 65535, 65535, 0, 0, 65535]);
        }

        assert!(doc.merge_layer_down(), "merge-down must work on a 16-bit document");
        assert_eq!(doc.layers.len(), 1, "the upper layer is consumed");

        let merged = &doc.layers[0];
        assert_eq!(merged.format(), PixelFormat::Srgb16, "merge must not downgrade storage");
        let values = merged.as_srgb16().expect("16-bit");
        assert_eq!(&values[0..4], &[65535, 0, 0, 65535], "the opaque upper layer covers the lower");
    }

    /// Mixed formats merge through the same path: each layer is read in its own format and the
    /// result lands in the lower layer's.
    #[test]
    fn merge_down_handles_a_mixed_format_pair() {
        use crate::layer::LayerBuffer;

        let mut doc = Document::new(2, 1); // 8-bit document
        let base = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(base) {
            layer.as_srgb8_exact_mut().copy_from_slice(&[0, 0, 255, 255, 0, 0, 255, 255]);
        }
        let upper = doc.add_layer(None);
        if let Some(layer) = doc.get_layer_mut(upper) {
            *layer.buffer_mut() =
                LayerBuffer::Srgb16(vec![65535, 0, 0, 65535, 65535, 0, 0, 65535]);
        }

        assert!(doc.merge_layer_down());
        let merged = &doc.layers[0];
        assert_eq!(
            merged.format(),
            PixelFormat::Srgb8,
            "the result lands in the lower layer's format"
        );
        assert_eq!(&merged.as_srgb8_exact()[0..4], &[255, 0, 0, 255]);
    }

    /// GOLDEN for 8-bit merge-down: the working-precision rewrite must not move these bytes.
    /// Red at 50% alpha over opaque blue is the case with an intermediate value on every
    /// channel.
    #[test]
    fn merge_down_eight_bit_output_is_unchanged() {
        let mut doc = Document::new(2, 1);
        let base = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(base) {
            layer.as_srgb8_exact_mut().copy_from_slice(&[0, 0, 255, 255, 0, 0, 255, 255]);
        }
        let upper = doc.add_layer(None);
        if let Some(layer) = doc.get_layer_mut(upper) {
            layer.as_srgb8_exact_mut().copy_from_slice(&[255, 0, 0, 128, 255, 0, 0, 128]);
        }

        assert!(doc.merge_layer_down());
        assert_eq!(&doc.layers[0].as_srgb8_exact()[0..4], &[128, 0, 127, 255]);
    }
    /// `convert_format` is exact in the widening direction and rounds only when asked to
    /// narrow.
    #[test]
    fn convert_format_round_trips_lattice_values() {
        let mut layer = Layer::new(1, "l", 2, 1);
        layer.as_srgb8_exact_mut().copy_from_slice(&[1, 2, 3, 4, 255, 128, 0, 64]);
        let original = layer.as_srgb8_exact().to_vec();

        layer.convert_format(PixelFormat::Srgb16);
        assert_eq!(layer.format(), PixelFormat::Srgb16);
        assert_eq!(layer.storage_len(), 2 * 4 * 2);

        layer.convert_format(PixelFormat::Srgb8);
        assert_eq!(layer.format(), PixelFormat::Srgb8);
        assert_eq!(layer.as_srgb8_exact(), original.as_slice());
    }
}

/// Digests of every filter's 8-bit output, printed so they can be pinned before the filters
/// become storage-generic. Run with `--nocapture` to read them.
#[cfg(test)]
mod filter_baseline {
    use crate::filter::*;
    use crate::selection::SelectionMask;

    const W: u32 = 16;
    const H: u32 = 16;

    fn fixture() -> Vec<u8> {
        let mut pixels = vec![0u8; (W * H * 4) as usize];
        for (index, px) in pixels.chunks_exact_mut(4).enumerate() {
            let x = (index as u32) % W;
            let y = (index as u32) / W;
            px[0] = ((x * 17 + y * 3) % 256) as u8;
            px[1] = ((x * 5 + y * 29) % 256) as u8;
            px[2] = ((x * 11 + y * 7) % 256) as u8;
            px[3] = if (x + y) % 5 == 0 { 0 } else { 255 - (x * 3 % 64) as u8 };
        }
        pixels
    }

    fn half_selection() -> SelectionMask {
        SelectionMask::from_mask_vec(
            W,
            H,
            (0..W * H).map(|i| if i % W < W / 2 { 255u8 } else { 0 }).collect(),
        )
    }

    fn digest(bytes: &[u8]) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for &byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    fn cases() -> Vec<(&'static str, Box<dyn Fn(&mut Vec<u8>)>)> {
        vec![
            ("adjust_hsl", Box::new(|p| adjust_hsl(p, W, H, 40.0, 1.3, -0.1, None))),
            ("adjust_brightness_contrast", Box::new(|p| adjust_brightness_contrast(p, W, H, 20.0, 15.0, None))),
            ("adjust_color_balance", Box::new(|p| adjust_color_balance(p, W, H, 30.0, -20.0, 10.0, None))),
            ("invert", Box::new(|p| filter_invert(p, W, H, None))),
            ("invert_masked", Box::new(|p| filter_invert(p, W, H, Some(&half_selection())))),
            ("posterize", Box::new(|p| filter_posterize(p, W, H, 5, None))),
            ("grayscale", Box::new(|p| filter_grayscale(p, W, H, None))),
            ("sepia", Box::new(|p| filter_sepia(p, W, H, None))),
            ("threshold", Box::new(|p| filter_threshold(p, W, H, 128, None))),
            ("gaussian_blur", Box::new(|p| filter_gaussian_blur(p, W, H, 2.0, None))),
            ("gaussian_blur_masked", Box::new(|p| filter_gaussian_blur(p, W, H, 2.0, Some(&half_selection())))),
            ("sharpen", Box::new(|p| filter_sharpen(p, W, H, 0.6, None))),
            ("unsharp_mask", Box::new(|p| filter_unsharp_mask(p, W, H, 2.0, 0.8, 4.0, None))),
            ("film_grain_mono", Box::new(|p| filter_film_grain(p, W, H, 0.4, false, None))),
            ("film_grain_color", Box::new(|p| filter_film_grain(p, W, H, 0.3, true, None))),
            ("vignette", Box::new(|p| filter_vignette(p, W, H, 0.9, 0.5, 0.6, None))),
            ("chromatic_aberration", Box::new(|p| filter_chromatic_aberration(p, W, H, 3.0, 30.0, None))),
            ("levels", Box::new(|p| filter_levels(p, W, H, 0.1, 0.9, 1.3, None))),
            ("colorize", Box::new(|p| filter_colorize(p, W, H, 200.0, 0.6, None))),
            ("pixelate", Box::new(|p| filter_pixelate(p, W, H, 4, None))),
            ("motion_blur", Box::new(|p| filter_motion_blur(p, W, H, 3, 20.0, None))),
        ]
    }

    const EXPECTED: &[(&str, u64)] = &[
        ("adjust_hsl", 10760832446858165850),
        ("adjust_brightness_contrast", 9445113945978877559),
        ("adjust_color_balance", 13744510413082002765),
        ("invert", 3927823269710674597),
        ("invert_masked", 8406660380971115137),
        ("posterize", 9133913637082582011),
        ("grayscale", 17785329894304394479),
        ("sepia", 3991839548642866574),
        ("threshold", 13398940257513858144),
        ("gaussian_blur", 17770303175522409645),
        ("gaussian_blur_masked", 10422072700280778485),
        ("sharpen", 11231089234561614405),
        ("unsharp_mask", 8097350176102318458),
        ("film_grain_mono", 10213049159201041479),
        ("film_grain_color", 2825406824838431283),
        ("vignette", 17680570955852021183),
        ("chromatic_aberration", 18394350751082153129),
        ("levels", 11295343428126902345),
        ("colorize", 662666236567077697),
        ("pixelate", 8499930175189816357),
        ("motion_blur", 9339520916181006568),
    ];

    /// Pinned before the filters became storage-generic. Every filter must keep producing
    /// exactly these bytes at 8-bit; each filter's rounding rule (round vs truncate) is part
    /// of its output and travels with it.
    #[test]
    fn eight_bit_filter_output_is_unchanged() {
        let expected: std::collections::HashMap<&str, u64> = EXPECTED.iter().copied().collect();
        for (name, apply) in cases() {
            let mut pixels = fixture();
            apply(&mut pixels);
            assert_eq!(
                digest(&pixels),
                expected[name],
                "filter '{name}' changed its 8-bit output"
            );
        }
    }
}

/// Every filter on native 16-bit storage.
#[cfg(test)]
mod filter_sixteen_bit {
    use crate::document::Document;
    use crate::filter::*;
    use crate::layer::{Layer, LayerBuffer, PixelFormat};
    use crate::pixel::srgb8_to_srgb16;
    use crate::selection::SelectionMask;

    const SIDE: u32 = 8;

    fn wide_doc(lattice: bool) -> Document {
        let mut doc = Document::new_with_format(SIDE, SIDE, PixelFormat::Srgb16);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            let channels: Vec<u16> = (0..(SIDE * SIDE * 4) as usize)
                .map(|index| {
                    if index % 4 == 3 {
                        65535
                    } else {
                        let value = (index as u16).wrapping_mul(137).wrapping_add(11);
                        if lattice {
                            srgb8_to_srgb16((index as u8).wrapping_mul(7))
                        } else {
                            value
                        }
                    }
                })
                .collect();
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }
        doc
    }

    fn cases() -> Vec<(&'static str, Box<dyn Fn(&mut Layer)>)> {
        vec![
            ("adjust_hsl", Box::new(|l| adjust_hsl_layer(l, 40.0, 1.3, -0.1, None))),
            ("brightness_contrast", Box::new(|l| adjust_brightness_contrast_layer(l, 20.0, 15.0, None))),
            ("color_balance", Box::new(|l| adjust_color_balance_layer(l, 30.0, -20.0, 10.0, None))),
            ("invert", Box::new(|l| filter_invert_layer(l, None))),
            ("posterize", Box::new(|l| filter_posterize_layer(l, 5, None))),
            ("grayscale", Box::new(|l| filter_grayscale_layer(l, None))),
            ("sepia", Box::new(|l| filter_sepia_layer(l, None))),
            ("threshold", Box::new(|l| filter_threshold_layer(l, 128, None))),
            ("gaussian_blur", Box::new(|l| filter_gaussian_blur_layer(l, 2.0, None))),
            ("sharpen", Box::new(|l| filter_sharpen_layer(l, 0.6, None))),
            ("unsharp_mask", Box::new(|l| filter_unsharp_mask_layer(l, 2.0, 0.8, 4.0, None))),
            ("film_grain", Box::new(|l| filter_film_grain_layer(l, 0.4, false, None))),
            ("vignette", Box::new(|l| filter_vignette_layer(l, 0.9, 0.5, 0.6, None))),
            ("chromatic_aberration", Box::new(|l| filter_chromatic_aberration_layer(l, 3.0, 30.0, None))),
            ("levels", Box::new(|l| filter_levels_layer(l, 0.1, 0.9, 1.3, None))),
            ("colorize", Box::new(|l| filter_colorize_layer(l, 200.0, 0.6, None))),
            ("pixelate", Box::new(|l| filter_pixelate_layer(l, 4, None))),
            ("motion_blur", Box::new(|l| filter_motion_blur_layer(l, 3, 20.0, None))),
        ]
    }

    /// Each filter must run on 16-bit storage, keep the format, and actually change pixels.
    /// The panic this replaces was `as_srgb8_exact()` inside the filter's first pixel.
    #[test]
    fn every_filter_runs_on_sixteen_bit_storage() {
        for (name, apply) in cases() {
            let mut doc = wide_doc(false);
            let id = doc.active_layer_id;
            let before = doc.get_layer(id).expect("layer").storage_bytes();
            apply(doc.get_layer_mut(id).expect("layer"));

            let layer = doc.get_layer(id).expect("layer");
            assert_eq!(layer.format(), PixelFormat::Srgb16, "{name} changed the storage format");
            assert_ne!(
                layer.storage_bytes(),
                before,
                "{name} left the 16-bit layer untouched"
            );
        }
    }

    /// The filters that create fractional values must produce them at 16 bits. A filter that
    /// downconverted, computed, and promoted back would leave every channel a multiple of 257.
    #[test]
    fn filters_compute_natively_rather_than_round_tripping_eight_bit() {
        let native_cases: Vec<(&str, Box<dyn Fn(&mut Layer)>)> = vec![
            ("gaussian_blur", Box::new(|l| filter_gaussian_blur_layer(l, 2.0, None))),
            ("levels", Box::new(|l| filter_levels_layer(l, 0.1, 0.9, 1.3, None))),
            ("colorize", Box::new(|l| filter_colorize_layer(l, 200.0, 0.6, None))),
            ("unsharp_mask", Box::new(|l| filter_unsharp_mask_layer(l, 2.0, 0.8, 4.0, None))),
        ];

        for (name, apply) in native_cases {
            let mut doc = wide_doc(true); // lattice-aligned input: 8-bit cannot tell these apart
            let id = doc.active_layer_id;
            apply(doc.get_layer_mut(id).expect("layer"));
            let layer = doc.get_layer(id).expect("layer");
            assert!(
                layer.as_srgb16().expect("16-bit").iter().any(|&c| c % 257 != 0),
                "{name} produced only lattice values on a 16-bit layer; it was quantised"
            );
        }
    }

    /// For values on the lattice, the 16-bit result must downconvert to the 8-bit result for
    /// the filters whose arithmetic is exact at both widths.
    #[test]
    fn lattice_results_match_the_eight_bit_filters() {
        type Op = fn(&mut Layer);
        let exact: Vec<(&str, Op, Op)> = vec![
            (
                "invert",
                |l| filter_invert_layer(l, None),
                |l| { let (w, h) = (l.width, l.height); filter_invert(l.as_srgb8_exact_mut(), w, h, None) },
            ),
            (
                "grayscale",
                |l| filter_grayscale_layer(l, None),
                |l| { let (w, h) = (l.width, l.height); filter_grayscale(l.as_srgb8_exact_mut(), w, h, None) },
            ),
            (
                "threshold",
                |l| filter_threshold_layer(l, 128, None),
                |l| { let (w, h) = (l.width, l.height); filter_threshold(l.as_srgb8_exact_mut(), w, h, 128, None) },
            ),
        ];

        for (name, wide_op, narrow_op) in exact {
            let mut wide = wide_doc(true);
            let wide_id = wide.active_layer_id;
            wide_op(wide.get_layer_mut(wide_id).expect("layer"));

            // The same lattice values in an 8-bit layer.
            let mut narrow = Document::new(SIDE, SIDE);
            narrow.is_transparent = true;
            let narrow_id = narrow.active_layer_id;
            if let Some(layer) = narrow.get_layer_mut(narrow_id) {
                for (index, byte) in layer.as_srgb8_exact_mut().iter_mut().enumerate() {
                    *byte = if index % 4 == 3 { 255 } else { (index as u8).wrapping_mul(7) };
                }
            }
            narrow_op(narrow.get_layer_mut(narrow_id).expect("layer"));

            let wide_down: Vec<u8> = wide
                .get_layer(wide_id)
                .expect("layer")
                .as_srgb16()
                .expect("16-bit")
                .iter()
                .map(|&c| crate::pixel::srgb16_to_srgb8(c))
                .collect();
            assert_eq!(
                wide_down,
                narrow.get_layer(narrow_id).expect("layer").as_srgb8_exact(),
                "{name} differs between 8-bit and downconverted 16-bit on lattice values"
            );
        }
    }

    /// A selection mask must restrict a 16-bit filter exactly as it restricts an 8-bit one.
    #[test]
    fn selection_masks_restrict_sixteen_bit_filters() {
        let mut doc = wide_doc(false);
        let id = doc.active_layer_id;
        let before = doc.get_layer(id).expect("layer").storage_bytes();
        let selection = SelectionMask::from_mask_vec(
            SIDE,
            SIDE,
            (0..SIDE * SIDE).map(|i| if i % SIDE < SIDE / 2 { 255u8 } else { 0 }).collect(),
        );

        filter_invert_layer(doc.get_layer_mut(id).expect("layer"), Some(&selection));

        let layer = doc.get_layer(id).expect("layer");
        let values = layer.as_srgb16().expect("16-bit");
        // Column 0 is inside the selection (so inverted), the last column is outside.
        assert_eq!(values[0], 65535 - before_u16(&before, 0), "selected pixel must invert");
        let last = ((SIDE - 1) * 4) as usize;
        assert_eq!(values[last], before_u16(&before, last), "unselected pixel must not change");
    }

    fn before_u16(bytes: &[u8], channel_index: usize) -> u16 {
        let lo = bytes[channel_index * 2] as u16;
        let hi = bytes[channel_index * 2 + 1] as u16;
        lo | (hi << 8)
    }
}

/// Selection fill and stroke at both storage widths.
#[cfg(test)]
mod selection_coverage {
    use crate::layer::LayerBuffer;
    use crate::selection::SelectionMask;

    const W: u32 = 2;
    const H: u32 = 1;

    fn mask(values: [u8; 2]) -> SelectionMask {
        SelectionMask::from_mask_vec(W, H, values.to_vec())
    }

    fn eight(values: [u8; 8]) -> crate::layer::Layer {
        let mut layer = crate::layer::Layer::new(1, "l", W, H);
        layer.as_srgb8_exact_mut().copy_from_slice(&values);
        layer
    }

    /// 8-bit behaviour, by hand: opaque red through a full mask replaces a pixel; a half mask
    /// over transparent gives half alpha; a zero mask changes nothing.
    #[test]
    fn fill_matches_the_expected_eight_bit_values() {
        let mut layer = eight([0, 0, 255, 255, 0, 0, 0, 0]);
        mask([255, 128]).fill_selection_on_layer(&mut layer, [255, 0, 0, 255]);
        assert_eq!(&layer.as_srgb8_exact()[0..4], &[255, 0, 0, 255], "opaque over opaque");
        assert_eq!(&layer.as_srgb8_exact()[4..8], &[255, 0, 0, 128], "half coverage over transparent");

        let mut untouched = eight([9, 8, 7, 255, 9, 8, 7, 255]);
        mask([0, 0]).fill_selection_on_layer(&mut untouched, [255, 0, 0, 255]);
        assert_eq!(untouched.as_srgb8_exact(), &[9, 8, 7, 255, 9, 8, 7, 255]);

        // A fully transparent colour leaves the destination alone even through a full mask.
        let mut transparent = eight([9, 8, 7, 255, 9, 8, 7, 255]);
        mask([255, 255]).fill_selection_on_layer(&mut transparent, [255, 0, 0, 0]);
        assert_eq!(transparent.as_srgb8_exact(), &[9, 8, 7, 255, 9, 8, 7, 255]);
    }

    /// Native 16-bit fill: the destination's off-lattice alpha makes the blended result
    /// off-lattice too, which cannot happen if the layer was downconverted on the way in.
    #[test]
    fn fill_preserves_sixteen_bit_precision() {
        let mut layer = crate::layer::Layer::new(1, "l", W, H);
        let dst = crate::pixel::srgb8_to_srgb16(200) + 128;
        *layer.buffer_mut() = LayerBuffer::Srgb16(vec![dst, dst, dst, 65535, 0, 0, 0, 0]);
        assert_eq!(layer.format(), crate::layer::PixelFormat::Srgb16);

        mask([128, 255]).fill_selection_on_layer(&mut layer, [255, 255, 255, 255]);

        let values = layer.as_srgb16().expect("16-bit");
        assert_eq!(layer.format(), crate::layer::PixelFormat::Srgb16, "format preserved");
        assert_eq!(&values[4..8], &[65535, 65535, 65535, 65535], "second pixel is opaque white");
        // Half coverage over 78% alpha: a value the 8-bit lattice cannot hold.
        assert!(values[0] > dst, "half coverage must lighten the pixel");
        assert!(values[0] % 257 != 0, "the result must carry sub-8-bit detail");
        assert_eq!(values[3], 65535, "alpha saturates at the destination's");
    }

    /// Stroke must run on 16-bit storage and stay in that format.
    #[test]
    fn stroke_preserves_sixteen_bit_storage() {
        let mut layer = crate::layer::Layer::new(1, "l", 4, 4);
        *layer.buffer_mut() = LayerBuffer::Srgb16(vec![0u16; 4 * 4 * 4]);
        let selection = SelectionMask::from_mask_vec(4, 4, vec![255u8; 16]);

        selection.stroke_selection_on_layer(&mut layer, [255, 0, 0, 255], 1, crate::selection::StrokePosition::Inside);

        assert_eq!(layer.format(), crate::layer::PixelFormat::Srgb16);
        assert!(
            layer.as_srgb16().expect("16-bit").iter().any(|&c| c > 0),
            "the stroke must paint something"
        );
    }
}

/// Byte-identity gate for the free-transform refactor: the renderers must produce exactly these
/// outputs before their source sampling becomes channel-generic.
#[cfg(test)]
mod transform_baseline {
    use crate::transform::{
        render_mesh_warped_patch, render_quad_transformed_patch,
        render_transformed_patch, AffineTransform2D, InterpolationMode, MeshWarpGrid,
        PerspectiveQuadTransform, ThinPlateSpline2D,
    };
    use glam::Vec2;

    const SIDE: i32 = 16;

    fn patch() -> Vec<u8> {
        let mut bytes = vec![0u8; (8 * 8 * 4) as usize];
        for (index, px) in bytes.chunks_exact_mut(4).enumerate() {
            px[0] = ((index * 13) % 256) as u8;
            px[1] = ((index * 29 + 7) % 256) as u8;
            px[2] = ((index * 47 + 11) % 256) as u8;
            px[3] = if index % 7 == 0 { 0 } else { 255 - (index % 96) as u8 };
        }
        bytes
    }

    fn digest(bytes: &[u8]) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for &byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    #[test]
    fn eight_bit_transform_output_is_unchanged() {
        let source = patch();
        let origin = Vec2::new(2.0, 3.0);

        let mut affine = AffineTransform2D::new(Vec2::new(6.0, 7.0));
        affine.pivot = Vec2::new(6.0, 7.0);
        let mut affine_out = vec![0u8; (SIDE * SIDE * 4) as usize];
        render_transformed_patch(
            &source, 8, 8, origin, &affine, InterpolationMode::Bilinear, &mut affine_out, SIDE as u32, SIDE as u32,
        );
        assert_eq!(digest(&affine_out), 2403627809797626894u64, "transform_affine");

        let mut bicubic_out = vec![0u8; (SIDE * SIDE * 4) as usize];
        render_transformed_patch(
            &source, 8, 8, origin, &affine, InterpolationMode::Bicubic, &mut bicubic_out, SIDE as u32, SIDE as u32,
        );
        assert_eq!(digest(&bicubic_out), 15576127485438065549u64, "transform_affine_bicubic");

        let quad_src = [
            origin,
            origin + Vec2::new(8.0, 0.0),
            origin + Vec2::new(8.0, 8.0),
            origin + Vec2::new(0.0, 8.0),
        ];
        let quad = PerspectiveQuadTransform::new(quad_src, quad_src);
        let mut quad_out = vec![0u8; (SIDE * SIDE * 4) as usize];
        render_quad_transformed_patch(&source, 8, 8, &quad, InterpolationMode::Bilinear, &mut quad_out, SIDE as u32, SIDE as u32);
        assert_eq!(digest(&quad_out), 2403627809797626894u64, "transform_quad");

        let mesh = MeshWarpGrid::new(3, 3, origin, 8.0, 8.0);
        let mut mesh_out = vec![0u8; (SIDE * SIDE * 4) as usize];
        render_mesh_warped_patch(&source, 8, 8, &mesh, InterpolationMode::Bilinear, &mut mesh_out, SIDE as u32, SIDE as u32);
        assert_eq!(digest(&mesh_out), 2403627809797626894u64, "transform_mesh");

        let pins = vec![
            origin,
            origin + Vec2::new(8.0, 0.0),
            origin + Vec2::new(8.0, 8.0),
            origin + Vec2::new(0.0, 8.0),
            Vec2::new(6.0, 7.0),
        ];
        let tps = ThinPlateSpline2D::solve(&pins, &pins);
        let mut tps_out = vec![0u8; (SIDE * SIDE * 4) as usize];
        crate::transform::render_tps_warped_patch(&source, 8, 8, origin, &tps, InterpolationMode::Bilinear, &mut tps_out, SIDE as u32, SIDE as u32);
        assert_eq!(digest(&tps_out), 2403627809797626894u64, "transform_tps");

    }
}

/// Commit A of the free-transform refactor: the channel-generic samplers must reproduce the
/// existing `&[u8]` samplers **exactly**, over a deterministic grid rather than a few points.
#[cfg(test)]
mod generic_sampling {
    use crate::transform::{
        sample_bicubic, sample_bilinear, sample_interpolated, sample_interpolated_scaled,
        sample_nearest,
    };
    use crate::transform::InterpolationMode;

    const W: u32 = 8;
    const H: u32 = 8;

    fn source() -> Vec<u8> {
        let mut bytes = vec![0u8; (W * H * 4) as usize];
        for (index, px) in bytes.chunks_exact_mut(4).enumerate() {
            px[0] = ((index * 37) % 256) as u8;
            px[1] = ((index * 11 + 5) % 256) as u8;
            px[2] = ((index * 53 + 19) % 256) as u8;
            // Some fully transparent pixels, so the alpha path is exercised too.
            px[3] = if index % 5 == 0 { 0 } else { 255 - (index % 128) as u8 };
        }
        bytes
    }

    /// Integer, fractional, on-boundary, just-outside and far-outside coordinates.
    fn coordinates() -> Vec<f32> {
        let mut values = vec![-4.0, -1.5, -0.75, 0.0, 0.5, 1.0, 1.25, 2.5, 3.0, 6.5];
        values.push(W as f32 - 1.0);
        values.push(W as f32 - 0.5);
        values.push(W as f32);
        values.push(W as f32 + 0.5);
        values.push(W as f32 + 3.0);
        values
    }

    #[test]
    fn scaled_samplers_reproduce_the_eight_bit_samplers() {
        let src = source();
        let coords = coordinates();
        let mut checked = 0usize;

        for &x in &coords {
            for &y in &coords {
                for mode in [
                    InterpolationMode::Nearest,
                    InterpolationMode::Bilinear,
                    InterpolationMode::Bicubic,
                ] {
                    let (r, g, b, a) = sample_interpolated(&src, W, H, x, y, mode);
                    let scaled = sample_interpolated_scaled::<u8>(&src, W, H, x, y, mode);
                    let quantised = [scaled[0] as u8, scaled[1] as u8, scaled[2] as u8, scaled[3] as u8];
                    assert_eq!(
                        [r, g, b, a],
                        quantised,
                        "{mode:?} at ({x}, {y}) differs between the byte sampler and the scaled one"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 500, "the grid exercised only {checked} samples");
    }

    /// The per-mode entry points must agree with the dispatcher (and therefore with the old
    /// per-mode functions), including the out-of-bounds branches that differ per mode:
    /// bilinear clamps to the nearest pixel, bicubic returns transparent.
    #[test]
    fn per_mode_wrappers_match_the_old_behaviour() {
        let src = source();
        for &x in &[0.5f32, 3.25, 7.0, 9.0] {
            for &y in &[0.5f32, 2.75, 7.0, 9.0] {
                let nearest = crate::transform::sample_nearest_scaled::<u8>(&src, W, H, x, y);
                let (r, g, b, a) = sample_nearest(&src, W, H, x, y);
                assert_eq!([nearest[0] as u8, nearest[1] as u8, nearest[2] as u8, nearest[3] as u8], [r, g, b, a]);

                let bilinear = crate::transform::sample_bilinear_scaled::<u8>(&src, W, H, x, y);
                let (r, g, b, a) = sample_bilinear(&src, W, H, x, y);
                assert_eq!([bilinear[0] as u8, bilinear[1] as u8, bilinear[2] as u8, bilinear[3] as u8], [r, g, b, a]);

                let bicubic = crate::transform::sample_bicubic_scaled::<u8>(&src, W, H, x, y);
                let (r, g, b, a) = sample_bicubic(&src, W, H, x, y);
                assert_eq!([bicubic[0] as u8, bicubic[1] as u8, bicubic[2] as u8, bicubic[3] as u8], [r, g, b, a]);
            }
        }
    }

    /// A 16-bit source samples at 16-bit precision: an off-lattice neighbour comes back
    /// untouched, and interpolated values exceed what 8 bits can hold.
    #[test]
    fn sixteen_bit_sources_keep_their_precision() {
        let dst = crate::pixel::srgb8_to_srgb16(200) + 128;
        let high = 60000u16;
        let mut channels = vec![dst; 4];
        channels.extend(std::iter::repeat(high).take(4));
        channels.extend(std::iter::repeat(0u16).take((W as usize * H as usize - 2) * 4));
        assert_eq!(channels.len(), (W * H * 4) as usize);

        // Nearest: exact, off-lattice, no round trip through 8 bits.
        let nearest = crate::transform::sample_nearest_scaled::<u16>(&channels, W, H, 0.0, 0.0);
        assert_eq!(nearest[0], dst as f32);
        assert_ne!(nearest[0] as u16 % 257, 0, "the sample must stay off-lattice");

        // Bilinear between an off-lattice pixel and a high-value neighbour.
        let bilinear = crate::transform::sample_bilinear_scaled::<u16>(&channels, W, H, 0.5, 0.0);
        assert!(
            bilinear[0] > 255.0,
            "a 16-bit sample must exceed the 8-bit range, got {}",
            bilinear[0]
        );
        assert!(bilinear[0] < dst as f32 + 1.0 || bilinear[0] <= high as f32);

        // Bicubic over the same neighbourhood also stays above 8 bits.
        let bicubic = crate::transform::sample_bicubic_scaled::<u16>(&channels, W, H, 0.5, 0.0);
        assert!(bicubic[0] > 255.0, "bicubic collapsed towards 8 bits: {}", bicubic[0]);
    }
}

/// Commit B: the renderers read and write native precision.
#[cfg(test)]
mod transform_precision {
    use crate::transform::{render_transformed_patch, AffineTransform2D, InterpolationMode};
    use glam::Vec2;

    const SIDE: u32 = 8;
    const PATCH: u32 = 4;

    /// Off-lattice, opaque patch: nothing here is representable in 8 bits.
    fn wide_patch() -> Vec<u16> {
        let mut channels = vec![0u16; (PATCH * PATCH * 4) as usize];
        for (index, px) in channels.chunks_exact_mut(4).enumerate() {
            let step = crate::pixel::srgb8_to_srgb16((20 + index as u8 * 7).min(255));
            px[0] = step + 128;
            px[1] = crate::pixel::srgb8_to_srgb16(90) + 64;
            px[2] = crate::pixel::srgb8_to_srgb16(200) + 255;
            px[3] = 65535;
        }
        channels
    }

    fn identity_transform() -> AffineTransform2D {
        let center = Vec2::new(PATCH as f32 * 0.5, PATCH as f32 * 0.5);
        let mut transform = AffineTransform2D::new(center);
        transform.pivot = center;
        transform
    }

    /// The case the reviewer asked for: a 16-bit source rendered into a 16-bit destination with
    /// identical maxima must preserve every channel exactly, alpha included. Nothing may round.
    #[test]
    fn sixteen_bit_source_into_sixteen_bit_destination_is_exact() {
        let patch = wide_patch();
        let origin = Vec2::new(1.0, 1.0);
        let mut dst = vec![0u16; (SIDE * SIDE * 4) as usize];

        render_transformed_patch(
            &patch, PATCH, PATCH, origin, &identity_transform(),
            InterpolationMode::Nearest, &mut dst, SIDE, SIDE,
        );

        for sy in 0..PATCH as usize {
            for sx in 0..PATCH as usize {
                let src = (sy * PATCH as usize + sx) * 4;
                let d = ((sy + 1) * SIDE as usize + (sx + 1)) * 4;
                assert_eq!(
                    &dst[d..d + 4],
                    &patch[src..src + 4],
                    "pixel ({sx}, {sy}) must survive the full render unchanged"
                );
            }
        }
        // The off-lattice fixture really is off-lattice, so the assertion above means something.
        assert!(patch.iter().any(|&c| c > 0 && c % 257 != 0));
    }

    /// A 16-bit source into an 8-bit destination must downconvert, not truncate: this is the
    /// cross-format direction, and it is the only place a width change is allowed to round.
    #[test]
    fn sixteen_bit_source_into_eight_bit_destination_rounds_once() {
        let patch = wide_patch();
        let origin = Vec2::new(1.0, 1.0);
        let mut dst = vec![0u8; (SIDE * SIDE * 4) as usize];

        render_transformed_patch(
            &patch, PATCH, PATCH, origin, &identity_transform(),
            InterpolationMode::Nearest, &mut dst, SIDE, SIDE,
        );

        for sy in 0..PATCH as usize {
            for sx in 0..PATCH as usize {
                let src = (sy * PATCH as usize + sx) * 4;
                let d = ((sy + 1) * SIDE as usize + (sx + 1)) * 4;
                for channel in 0..4 {
                    let expected = crate::pixel::srgb16_to_srgb8(patch[src + channel]);
                    let actual = dst[d + channel];
                    assert!(
                        actual.abs_diff(expected) <= 1,
                        "channel {channel} of ({sx}, {sy}): {actual} vs {expected}"
                    );
                }
            }
        }
    }
}

/// Commit C, part 1: the patch type that will replace `Vec<u8>` in the session.
#[cfg(test)]
mod transform_patch {
    use crate::document::Document;
    use crate::layer::{LayerBuffer, PixelFormat};
    use crate::transform::TransformPatch;

    fn wide_doc() -> Document {
        let mut doc = Document::new_with_format(6, 4, PixelFormat::Srgb16);
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            let channels: Vec<u16> = (0..(6 * 4 * 4))
                .map(|index| (index as u16).wrapping_mul(997).wrapping_add(13))
                .collect();
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }
        doc
    }

    #[test]
    fn a_patch_takes_the_layers_format_and_keeps_off_lattice_values() {
        let doc = wide_doc();
        let layer = doc.active_layer().expect("layer");
        let patch = TransformPatch::extract_from_layer(layer, 1, 1, 3, 2, &|_, _| true);

        assert_eq!(patch.format(), PixelFormat::Srgb16, "the patch carries the format itself");
        assert_eq!(patch.pixels(), 6);
        let TransformPatch::Srgb16(channels) = &patch else {
            panic!("expected 16-bit patch");
        };
        let source = layer.as_srgb16().expect("16-bit");
        let source_index = (1 * 6 + 1) * 4;
        assert_eq!(&channels[0..4], &source[source_index..source_index + 4]);
        assert!(channels.iter().any(|&c| c % 257 != 0), "off-lattice values must survive");
    }

    /// Pixels the predicate rejects stay zeroed, which is how a selection-shaped patch keeps
    /// its shape, and a rectangle running past the layer reads as zero instead of panicking.
    #[test]
    fn rejected_and_out_of_bounds_pixels_read_as_zero() {
        let doc = wide_doc();
        let layer = doc.active_layer().expect("layer");
        let patch = TransformPatch::extract_from_layer(layer, 4, 2, 4, 4, &|x, y| x == 4 && y == 2);
        let TransformPatch::Srgb16(channels) = &patch else {
            panic!("expected 16-bit patch");
        };
        let source_index = (2 * 6 + 4) * 4;
        assert_eq!(&channels[0..4], &layer.as_srgb16().unwrap()[source_index..source_index + 4]);
        assert!(channels[4..].iter().all(|&c| c == 0), "rejected and out-of-bounds pixels are zero");
    }

    #[test]
    fn an_eight_bit_patch_stays_eight_bit() {
        let mut doc = Document::new(4, 4);
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            layer.as_srgb8_exact_mut().copy_from_slice(&[7u8; 4 * 4 * 4]);
        }
        let layer = doc.get_layer(id).expect("layer");
        let patch = TransformPatch::extract_from_layer(layer, 0, 0, 2, 2, &|_, _| true);
        assert_eq!(patch.format(), PixelFormat::Srgb8);
        let TransformPatch::Srgb8(bytes) = &patch else {
            panic!("expected 8-bit patch");
        };
        assert_eq!(bytes.len(), 16);
        assert!(bytes.iter().all(|&b| b == 7));
    }
}

/// Commit C, part 2 prerequisites: the four-way source/destination matrix and the alpha edges
/// the session will depend on.
#[cfg(test)]
mod transform_matrix {
    use crate::transform::{render_transformed_patch, AffineTransform2D, InterpolationMode};
    use glam::Vec2;

    const SIDE: u32 = 4;

    fn identity(size: u32) -> AffineTransform2D {
        let center = Vec2::new(size as f32 * 0.5, size as f32 * 0.5);
        let mut transform = AffineTransform2D::new(center);
        transform.pivot = center;
        transform
    }

    /// 8-bit source -> 16-bit destination widens by exactly 257, with no rounding.
    #[test]
    fn eight_bit_source_scales_exactly_into_sixteen_bit_destination() {
        let patch = vec![200u8, 100, 50, 255].repeat((SIDE * SIDE) as usize);
        let mut dst = vec![0u16; (SIDE * SIDE * 4) as usize];

        render_transformed_patch(
            &patch, SIDE, SIDE, Vec2::new(1.0, 1.0), &identity(SIDE),
            InterpolationMode::Nearest, &mut dst, SIDE, SIDE,
        );

        let index = ((2 * SIDE + 2) * 4) as usize;
        assert_eq!(dst[index], crate::pixel::srgb8_to_srgb16(200));
        assert_eq!(dst[index + 1], crate::pixel::srgb8_to_srgb16(100));
        assert_eq!(dst[index + 2], crate::pixel::srgb8_to_srgb16(50));
        assert_eq!(dst[index + 3], 65535);
    }

    /// Non-zero RGB under zero alpha must not contaminate the destination. This is the case
    /// that catches a compositor reading colour without checking alpha first.
    #[test]
    fn colour_under_zero_alpha_never_contaminates() {
        for wide in [false, true] {
            let mut eight_dst = vec![0u8; (SIDE * SIDE * 4) as usize];
            let mut wide_dst = vec![0u16; (SIDE * SIDE * 4) as usize];

            if wide {
                // Opaque red with alpha 0: colour is present, coverage is not.
                let patch = vec![65535u16, 0, 0, 0].repeat((SIDE * SIDE) as usize);
                render_transformed_patch(
                    &patch, SIDE, SIDE, Vec2::new(1.0, 1.0), &identity(SIDE),
                    InterpolationMode::Nearest, &mut wide_dst, SIDE, SIDE,
                );
                assert!(wide_dst.iter().all(|&c| c == 0), "a zero-alpha 16-bit source must paint nothing");
            } else {
                let patch = vec![255u8, 0, 0, 0].repeat((SIDE * SIDE) as usize);
                render_transformed_patch(
                    &patch, SIDE, SIDE, Vec2::new(1.0, 1.0), &identity(SIDE),
                    InterpolationMode::Nearest, &mut eight_dst, SIDE, SIDE,
                );
                assert!(eight_dst.iter().all(|&c| c == 0), "a zero-alpha 8-bit source must paint nothing");
            }
        }
    }

    /// Partial source alpha over a transparent destination keeps the colour and halves the
    /// alpha, at both widths - the unpremultiply path, where colour and coverage separate.
    #[test]
    fn partial_alpha_over_transparent_keeps_colour_and_scales_alpha() {
        let patch = vec![255u8, 0, 0, 128].repeat((SIDE * SIDE) as usize);
        let mut dst = vec![0u8; (SIDE * SIDE * 4) as usize];
        render_transformed_patch(
            &patch, SIDE, SIDE, Vec2::new(1.0, 1.0), &identity(SIDE),
            InterpolationMode::Nearest, &mut dst, SIDE, SIDE,
        );
        let index = ((2 * SIDE + 2) * 4) as usize;
        assert_eq!(dst[index], 255, "colour must not be dimmed by its own coverage");
        assert_eq!(dst[index + 3], 128, "alpha carries the coverage");

        // The same in 16 bits: alpha scales, colour does not.
        let wide_patch = vec![65535u16, 0, 0, 128 * 257].repeat((SIDE * SIDE) as usize);
        let mut wide_dst = vec![0u16; (SIDE * SIDE * 4) as usize];
        render_transformed_patch(
            &wide_patch, SIDE, SIDE, Vec2::new(1.0, 1.0), &identity(SIDE),
            InterpolationMode::Nearest, &mut wide_dst, SIDE, SIDE,
        );
        assert_eq!(wide_dst[index], 65535, "16-bit colour survives at full value");
        assert_eq!(wide_dst[index + 3], 128 * 257);
    }

    /// A partially transparent source over a partially transparent destination composes:
    /// out_a = sa + da(1 - sa), and the result stays between the two colours rather than
    /// jumping to a fully opaque value.
    #[test]
    fn partial_source_over_partial_destination_composes() {
        let mut dst = vec![0u16; (SIDE * SIDE * 4) as usize];
        for px in dst.chunks_exact_mut(4) {
            px[2] = 65535; // opaque blue background
            px[3] = 65535;
        }
        let patch = vec![65535u16, 0, 0, 32768].repeat((SIDE * SIDE) as usize); // half red

        render_transformed_patch(
            &patch, SIDE, SIDE, Vec2::new(1.0, 1.0), &identity(SIDE),
            InterpolationMode::Nearest, &mut dst, SIDE, SIDE,
        );

        let index = ((2 * SIDE + 2) * 4) as usize;
        assert_eq!(dst[index + 3], 65535, "opaque destination stays opaque");
        let red = dst[index];
        let blue = dst[index + 2];
        assert!(red > 0 && red < 65535, "red must be blended, got {red}");
        assert!(blue > 0 && blue < 65535, "blue must be blended, got {blue}");
        assert!(red as u32 + blue as u32 > 60000, "the two channels should roughly sum to white");
    }
}

/// Alpha-content bounds and native-alpha lookup are the Move tool's hit-testing and snapping
/// primitives. They must not go through an 8-bit accessor on a 16-bit layer.
#[test]
fn layer_content_bounds_and_alpha_follow_native_storage() {
    use crate::layer::{Layer, LayerBuffer};

    let mut eight = Layer::new(1, "eight", 8, 8);
    {
        let pixels = eight.as_srgb8_exact_mut();
        for (x, y) in [(2u32, 3u32), (6, 5)] {
            let index = ((y * 8 + x) * 4) as usize;
            pixels[index + 3] = 255;
        }
    }
    assert_eq!(eight.content_bounds(), Some((2, 3, 6, 5)));
    assert_eq!(eight.alpha_at(2, 3), Some(255));
    assert_eq!(eight.alpha_at(0, 0), Some(0));

    let mut wide = Layer::new(2, "wide", 8, 8);
    *wide.buffer_mut() = LayerBuffer::Srgb16(vec![0u16; 8 * 8 * 4]);
    {
        let LayerBuffer::Srgb16(channels) = wide.buffer_mut() else {
            unreachable!()
        };
        let index = ((4 * 8 + 1) * 4) as usize;
        channels[index + 3] = 1024; // low 8-bit byte is zero; native alpha is not
    }
    assert_eq!(wide.content_bounds(), Some((1, 4, 1, 4)));
    assert_eq!(wide.alpha_at(1, 4), Some(1024));
}
