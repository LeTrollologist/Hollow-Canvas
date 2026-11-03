#[cfg(test)]
mod tests {
    use crate::project::{load_project_from_reader, save_project_to_writer};
    use aurelia_core::document::Document;
    use std::io::Cursor;

    #[test]
    fn test_binary_hcv_project_archive_roundtrip() {
        let mut doc = Document::new(64, 64);
        let id2 = doc.add_layer(Some("Overlay Layer".to_string()));
        if let Some(l2) = doc.get_layer_mut(id2) {
            l2.set_pixel(10, 10, [168, 159, 216, 255]);
            l2.opacity = 0.85;
            l2.blend_mode = aurelia_core::blend::BlendMode::Overlay;
        }

        let mut buffer = Cursor::new(Vec::new());
        save_project_to_writer(&doc, &mut buffer).expect("Failed to save project");

        buffer.set_position(0);
        let loaded_doc = load_project_from_reader(&mut buffer).expect("Failed to load project");

        assert_eq!(loaded_doc.width, 64);
        assert_eq!(loaded_doc.height, 64);
        assert_eq!(loaded_doc.layers.len(), 2);
        assert_eq!(loaded_doc.layers[1].name, "Overlay Layer");
        assert_eq!(loaded_doc.layers[1].opacity, 0.85);
        assert_eq!(loaded_doc.layers[1].blend_mode, aurelia_core::blend::BlendMode::Overlay);
        assert_eq!(loaded_doc.layers[1].get_pixel(10, 10).unwrap(), [168, 159, 216, 255]);
    }

    #[test]
    fn test_binary_hcv_folder_group_roundtrip() {
        let mut doc = Document::new(32, 32);
        let group_id = doc.add_group(Some("Art Folder".to_string()));
        let child_id = doc.add_layer(Some("Inked Layer".to_string()));
        doc.set_layer_parent(child_id, Some(group_id));

        if let Some(child) = doc.get_layer_mut(child_id) {
            child.alpha_locked = true;
            child.clipping_mask = true;
            child.is_reference = true;
        }

        let mut buffer = Cursor::new(Vec::new());
        save_project_to_writer(&doc, &mut buffer).expect("Failed to save project");

        buffer.set_position(0);
        let loaded_doc = load_project_from_reader(&mut buffer).expect("Failed to load project");

        assert_eq!(loaded_doc.layers.len(), 3);
        let loaded_group = loaded_doc.get_layer(group_id).expect("Group not found");
        assert!(loaded_group.is_group());
        assert_eq!(loaded_group.name, "Art Folder");

        let loaded_child = loaded_doc.get_layer(child_id).expect("Child not found");
        assert!(loaded_child.is_raster());
        assert_eq!(loaded_child.parent_id, Some(group_id));
        assert!(loaded_child.alpha_locked);
        assert!(loaded_child.clipping_mask);
        assert!(loaded_child.is_reference);
    }

    #[test]
    fn test_binary_hcv_adjustment_layer_roundtrip() {
        use aurelia_core::layer::AdjustmentType;

        let mut doc = Document::new(32, 32);
        let adj_id = doc.add_adjustment_layer(
            AdjustmentType::Hsl {
                hue_shift: 45.0,
                saturation: 1.5,
                lightness: -0.1,
            },
            Some("Grading Layer".to_string()),
        );

        if let Some(adj_layer) = doc.get_layer_mut(adj_id) {
            adj_layer.opacity = 0.75;
            adj_layer.clipping_mask = true;
        }

        let mut buffer = Cursor::new(Vec::new());
        save_project_to_writer(&doc, &mut buffer).expect("Failed to save project");

        buffer.set_position(0);
        let loaded_doc = load_project_from_reader(&mut buffer).expect("Failed to load project");

        assert_eq!(loaded_doc.layers.len(), 2);
        let loaded_adj = loaded_doc.get_layer(adj_id).expect("Adjustment layer not found");
        assert!(loaded_adj.is_adjustment());
        assert_eq!(loaded_adj.name, "Grading Layer");
        assert_eq!(loaded_adj.opacity, 0.75);
        assert!(loaded_adj.clipping_mask);

        if let Some(cfg) = &loaded_adj.adjustment {
            match &cfg.adjustment_type {
                AdjustmentType::Hsl { hue_shift, saturation, lightness } => {
                    assert!((hue_shift - 45.0).abs() < 1e-4);
                    assert!((saturation - 1.5).abs() < 1e-4);
                    assert!((lightness - (-0.1)).abs() < 1e-4);
                }
                _ => panic!("Expected Hsl adjustment"),
            }
        } else {
            panic!("Adjustment config missing");
        }
    }

    #[test]
    fn test_binary_hcv_text_layer_roundtrip() {
        use aurelia_core::layer::TextLayerConfig;

        let mut doc = Document::new(48, 48);
        let config = TextLayerConfig {
            content: "Typography Test".to_string(),
            font_family: "Roboto".to_string(),
            font_size: 16.0,
            color: [100, 200, 255, 255],
            align: aurelia_core::brush::TextAlign::Right,
            line_spacing: 1.25,
            letter_spacing: 1.5,
            box_w: 40.0,
            box_h: 20.0,
            pos_x: 4.0,
            pos_y: 6.0,
        };

        let txt_id = doc.add_text_layer(Some("Title Text".to_string()), config.clone(), None);

        let mut buffer = Cursor::new(Vec::new());
        save_project_to_writer(&doc, &mut buffer).expect("Failed to save project");

        buffer.set_position(0);
        let loaded_doc = load_project_from_reader(&mut buffer).expect("Failed to load project");

        assert_eq!(loaded_doc.layers.len(), 2);
        let loaded_txt = loaded_doc.get_layer(txt_id).expect("Text layer not found");
        assert!(loaded_txt.is_text());
        assert_eq!(loaded_txt.name, "Title Text");

        let loaded_cfg = loaded_txt.text.as_ref().expect("Text config missing");
        assert_eq!(loaded_cfg.content, "Typography Test");
        assert_eq!(loaded_cfg.font_family, "Roboto");
        assert_eq!(loaded_cfg.font_size, 16.0);
        assert_eq!(loaded_cfg.color, [100, 200, 255, 255]);
        assert_eq!(loaded_cfg.align, aurelia_core::brush::TextAlign::Right);
        assert_eq!(loaded_cfg.line_spacing, 1.25);
        assert_eq!(loaded_cfg.letter_spacing, 1.5);
        assert_eq!(loaded_cfg.box_w, 40.0);
        assert_eq!(loaded_cfg.box_h, 20.0);
        assert_eq!(loaded_cfg.pos_x, 4.0);
        assert_eq!(loaded_cfg.pos_y, 6.0);
    }

    #[test]
    fn test_atomic_save_replaces_file_and_leaves_no_temp() {
        let dir = std::env::temp_dir().join(format!("hc-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("art.aur");

        let doc = Document::new(16, 16);
        crate::project::save_project_file(&doc, &path).unwrap();

        let mut bigger = Document::new(48, 32);
        bigger.add_layer(Some("Second".to_string()));
        crate::project::save_project_file(&bigger, &path).unwrap();

        let loaded = crate::project::load_project_file(&path).unwrap();
        assert_eq!((loaded.width, loaded.height), (48, 32));
        assert_eq!(loaded.layers.len(), 2);

        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "atomic save left a temp file behind");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_jpeg_quality_changes_output_size() {
        let mut doc = Document::new(64, 64);
        {
            let layer = doc.active_layer_mut().unwrap();
            for y in 0..64 {
                for x in 0..64 {
                    let idx = ((y * 64 + x) * 4) as usize;
                    layer.as_srgb8_mut()[idx] = (x * 4) as u8;
                    layer.as_srgb8_mut()[idx + 1] = (y * 4) as u8;
                    layer.as_srgb8_mut()[idx + 2] = ((x + y) * 2).min(255) as u8;
                    layer.as_srgb8_mut()[idx + 3] = 255;
                }
            }
        }

        let dir = std::env::temp_dir().join(format!("hc-jpeg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let low = dir.join("q20.jpg");
        let high = dir.join("q95.jpg");

        crate::export::export_flat_image_with_quality(
            &doc,
            &low,
            crate::export::ExportFormat::Jpeg,
            true,
            Some(20),
        )
        .unwrap();
        crate::export::export_flat_image_with_quality(
            &doc,
            &high,
            crate::export::ExportFormat::Jpeg,
            true,
            Some(95),
        )
        .unwrap();

        let low_len = std::fs::metadata(&low).unwrap().len();
        let high_len = std::fs::metadata(&high).unwrap().len();
        assert!(low_len > 0 && high_len > 0);
        assert!(
            high_len > low_len,
            "quality 95 ({high_len} bytes) should exceed quality 20 ({low_len} bytes)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sprite_sheet_packs_frames_in_grid() {
        let (w, h) = (8u32, 8u32);
        let red = [255u8, 0, 0, 255];
        let blue = [0u8, 0, 255, 255];
        let frame = |color: [u8; 4]| -> Vec<u8> {
            color.iter().copied().cycle().take((w * h * 4) as usize).collect()
        };

        let dir = std::env::temp_dir().join(format!("hc-sheet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sheet_path = dir.join("sheet.png");

        let (sw, sh) =
            crate::export::export_sprite_sheet(&[frame(red), frame(blue)], w, h, 2, &sheet_path)
                .unwrap();
        assert_eq!((sw, sh), (16, 8));

        let sheet = image::open(&sheet_path).unwrap().to_rgba8();
        assert_eq!(sheet.dimensions(), (16, 8));
        assert_eq!(sheet.get_pixel(0, 0).0, red);
        assert_eq!(sheet.get_pixel(8, 0).0, blue);

        let _ = std::fs::remove_dir_all(&dir);
    }


    #[test]
    fn layer_mask_survives_a_save_and_load_roundtrip() {
        let mut doc = Document::new(6, 5);
        let id = doc.active_layer_id;
        assert!(doc.add_layer_mask(id, None));
        assert!(doc.invert_layer_mask(id));
        if let Some(layer) = doc.active_layer_mut() {
            layer.set_pixel(1, 1, [10, 20, 30, 255]);
        }

        let mut buf = Cursor::new(Vec::new());
        save_project_to_writer(&doc, &mut buf).expect("save");
        buf.set_position(0);
        let loaded = load_project_from_reader(buf).expect("load");

        let mask = loaded.get_layer(id).expect("layer").mask.as_ref().expect("mask");
        assert_eq!((mask.width, mask.height), (6, 5));
        assert!(mask.enabled);
        assert!(mask.pixels.iter().all(|&p| p == 0), "the inverted mask is all hidden");
        // Pixels themselves are untouched by the mask.
        assert_eq!(loaded.get_layer(id).unwrap().get_pixel(1, 1), Some([10, 20, 30, 255]));
    }

    #[test]
    fn layers_without_masks_still_roundtrip() {
        let doc = Document::new(4, 4);
        let mut buf = Cursor::new(Vec::new());
        save_project_to_writer(&doc, &mut buf).expect("save");
        buf.set_position(0);
        let loaded = load_project_from_reader(buf).expect("load");
        assert!(loaded.layers.iter().all(|l| l.mask.is_none()));
    }

    /// The byte stream v2.4.x wrote: the current body with the sRGB8 magic and without the
    /// per-layer mask section that AUR v2 introduced.
    ///
    /// It is derived from the current writer rather than transcribed by hand, so the test
    /// cannot pass against a layout that never existed: the thumbnail is a PNG written
    /// straight after the layer loop behind its u32 length, and the single byte between
    /// the last layer and that length is its mask flag.
    fn downgrade_to_v1(v2: &[u8]) -> Vec<u8> {
        let png = v2
            .windows(4)
            .rposition(|w| w == b"\x89PNG")
            .expect("thumbnail is a PNG");
        let mask_flag_at = png - 5;
        assert_eq!(v2[mask_flag_at], 0, "the v1 fixture must not carry a mask");
        let mut v1 = Vec::with_capacity(v2.len() - 1);
        v1.extend_from_slice(b"AUR\x01");
        v1.extend_from_slice(&v2[4..mask_flag_at]);
        v1.extend_from_slice(&v2[png - 4..]);
        v1
    }

    #[test]
    fn v1_project_files_still_load() {
        let mut doc = Document::new(6, 5);
        doc.is_transparent = true;
        let id = doc.active_layer_id;
        if let Some(layer) = doc.active_layer_mut() {
            layer.name = "Legacy".to_string();
            layer.opacity = 0.5;
            layer.offset_x = 2;
            layer.offset_y = -1;
        }
        for (x, y, px) in [
            (0u32, 0u32, [10u8, 20, 30, 255]),
            (3, 4, [200, 100, 50, 128]),
        ] {
            if let Some(layer) = doc.active_layer_mut() {
                layer.set_pixel(x, y, px);
            }
        }

        let mut v2 = Vec::new();
        save_project_to_writer(&doc, &mut v2).expect("save");
        let v1 = downgrade_to_v1(&v2);
        let loaded = load_project_from_reader(&v1[..]).expect("a v1 file must still load");

        assert_eq!((loaded.width, loaded.height), (6, 5));
        assert_eq!(loaded.active_layer_id, id);
        assert_eq!(loaded.layers.len(), 1);
        let layer = loaded.get_layer(id).expect("layer");
        assert_eq!(layer.name, "Legacy");
        assert_eq!(layer.opacity, 0.5);
        assert_eq!((layer.offset_x, layer.offset_y), (2, -1));
        assert_eq!(layer.get_pixel(0, 0), Some([10, 20, 30, 255]));
        assert_eq!(layer.get_pixel(3, 4), Some([200, 100, 50, 128]));
        assert!(layer.mask.is_none(), "v1 files never carry a mask");
    }

    /// A damaged file must be reported, not panic - a crash here would look like data loss
    /// to the person opening it.
    #[test]
    fn truncated_project_files_are_rejected_without_panicking() {
        let mut doc = Document::new(8, 6);
        if let Some(layer) = doc.active_layer_mut() {
            layer.set_pixel(0, 0, [1, 2, 3, 255]);
        }
        let mut v2 = Vec::new();
        save_project_to_writer(&doc, &mut v2).expect("save");

        // The preview thumbnail is the last thing in the file and is never read back, so
        // it marks the boundary: everything before it is artwork data.
        let thumb_len_at = v2
            .windows(4)
            .rposition(|w| w == b"\x89PNG")
            .expect("thumbnail is a PNG")
            - 4;

        // Truncating anywhere inside the artwork must be reported.
        for cut in [0usize, 3, 4, 12, 30, thumb_len_at - 1] {
            let result = load_project_from_reader(&v2[..cut]);
            assert!(result.is_err(), "a file truncated to {cut} bytes must be rejected");
        }

        // Losing only the thumbnail is tolerated on purpose: an openable project should not
        // be rejected over preview data.
        let loaded = load_project_from_reader(&v2[..thumb_len_at]).expect("artwork is intact");
        assert_eq!(loaded.width, doc.width);

        // And a file with a bad magic must not be mistaken for a project.
        let mut bad = v2.clone();
        bad[0..4].copy_from_slice(b"NOPE");
        assert!(load_project_from_reader(&bad[..]).is_err());
    }
}
