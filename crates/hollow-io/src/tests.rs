#[cfg(test)]
mod tests {
    use crate::project::{load_project_from_reader, save_project_to_writer};
    use hollow_core::document::Document;
    use std::io::Cursor;

    #[test]
    fn test_binary_hcv_project_archive_roundtrip() {
        let mut doc = Document::new(64, 64);
        let id2 = doc.add_layer(Some("Overlay Layer".to_string()));
        if let Some(l2) = doc.get_layer_mut(id2) {
            l2.set_pixel(10, 10, [168, 159, 216, 255]);
            l2.opacity = 0.85;
            l2.blend_mode = hollow_core::blend::BlendMode::Overlay;
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
        assert_eq!(loaded_doc.layers[1].blend_mode, hollow_core::blend::BlendMode::Overlay);
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
        use hollow_core::layer::AdjustmentType;

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
        use hollow_core::layer::TextLayerConfig;

        let mut doc = Document::new(48, 48);
        let config = TextLayerConfig {
            content: "Typography Test".to_string(),
            font_family: "Roboto".to_string(),
            font_size: 16.0,
            color: [100, 200, 255, 255],
            align: hollow_core::brush::TextAlign::Right,
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
        assert_eq!(loaded_cfg.align, hollow_core::brush::TextAlign::Right);
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
        let path = dir.join("art.hcv");

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
                    layer.pixels[idx] = (x * 4) as u8;
                    layer.pixels[idx + 1] = (y * 4) as u8;
                    layer.pixels[idx + 2] = ((x + y) * 2).min(255) as u8;
                    layer.pixels[idx + 3] = 255;
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

}
