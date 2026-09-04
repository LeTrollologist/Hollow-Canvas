use std::path::PathBuf;

pub fn open_project_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open Hollow Canvas Project")
        .add_filter("Hollow Canvas Project (*.hcv)", &["hcv"])
        .pick_file()
}

pub fn save_project_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Save Hollow Canvas Project")
        .add_filter("Hollow Canvas Project (*.hcv)", &["hcv"])
        .set_file_name("artwork.hcv")
        .save_file()
}

pub fn export_png_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export Flat PNG Image")
        .add_filter("PNG Image (*.png)", &["png"])
        .set_file_name("artwork.png")
        .save_file()
}

pub fn open_image_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open Reference Image")
        .add_filter("Image Files (*.png, *.jpg, *.jpeg, *.webp)", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
}
