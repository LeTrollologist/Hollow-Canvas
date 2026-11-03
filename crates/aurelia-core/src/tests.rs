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
        let mut pixels = vec![255, 0, 0, 255]; // Pure Red
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
        let mut pixels = vec![100, 150, 200, 255];
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
                    std::hint::black_box(crate::document::compose_u16_prepared(doc, promoted, false));
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
        let composed = crate::document::compose_u16_prepared(&doc, &native, false);
        assert_ne!(composed[0], composed[4], "16-bit compositing collapsed two values");
        assert_eq!(composed[0], low);
        assert_eq!(composed[4], high);
        let eight_bit = doc.composite_layers(false);
        assert_eq!(eight_bit[0], eight_bit[4], "an 8-bit result cannot carry it");
    }
}
