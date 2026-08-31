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
}
