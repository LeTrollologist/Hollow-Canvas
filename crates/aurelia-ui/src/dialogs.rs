use std::path::PathBuf;

pub fn open_project_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open Aurelia Project")
        .add_filter("Aurelia Project (*.aur)", &["aur"])
        .add_filter("Legacy Hollow Canvas Project (*.hcv)", &["hcv"])
        .pick_file()
}

pub fn save_project_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Save Aurelia Project")
        .add_filter("Aurelia Project (*.aur)", &["aur"])
        .set_file_name("artwork.aur")
        .save_file()
}

pub fn export_png_dialog() -> Option<PathBuf> {
    export_image_dialog("png", "PNG Image (*.png)")
}

/// Save dialog for a flat image export in the given extension (png / jpg / webp).
pub fn export_image_dialog(extension: &str, filter_label: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export Flat Image")
        .add_filter(filter_label, &[extension])
        .set_file_name(format!("artwork.{}", extension))
        .save_file()
}

pub fn open_image_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open Reference Image")
        .add_filter("Image Files (*.png, *.jpg, *.jpeg, *.webp)", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
}
