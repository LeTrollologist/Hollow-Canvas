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
}
