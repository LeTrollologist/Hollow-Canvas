#[cfg(test)]
mod tests {
    use crate::blend::BlendMode;
    use crate::brush::{BrushPoint, BrushSettings, ToolType};
    use crate::color::Color;
    use crate::document::Document;
    use crate::rasterizer::StrokeRasterizer;
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
            l2.pixels.chunks_exact_mut(4).for_each(|px| px.copy_from_slice(&[0, 0, 255, 255]));
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
            true,
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
                    layer.pixels[idx..idx + 4].copy_from_slice(&red);
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
            assert!(layer.pixels[idx] > 0, "Smudge should push red pixels to (33, 20)");
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
            let center_alpha = layer.pixels[center_idx];

            let edge_idx = (25 * 50 + 33) * 4 + 3;
            let edge_alpha = layer.pixels[edge_idx];

            assert!(edge_alpha > 0, "Edge pixel should have paint");
            assert!(center_alpha > 0, "Center pixel should have paint");
        }
    }
}
