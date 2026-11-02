pub mod design;
pub mod dialogs;
pub mod fonts;
pub mod icons;
pub mod ruler_text;
pub mod shell;
pub mod state;
pub mod style;
pub mod ui;

/// Single source of truth for the version the UI displays.
///
/// Comes from this crate's `Cargo.toml`, which the release pipeline rewrites in
/// lockstep with `aurelia-app`, so the About dialog and title bar can never drift
/// behind the shipped build.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use dialogs::{
    export_image_dialog, export_png_dialog, open_image_dialog, open_project_dialog,
    save_project_dialog,
};
pub use shell::{is_shell_registered, register_shell_associations, unregister_shell_associations};
pub use state::{AppState, PendingFileAction};
pub use style::configure_aurelia_style;
pub use ui::render_ui;

#[cfg(test)]
mod tests {
    use super::APP_VERSION;

    #[test]
    fn app_version_is_semver_and_manifest_driven() {
        assert_eq!(APP_VERSION, env!("CARGO_PKG_VERSION"));
        let core = APP_VERSION.split(['-', '+']).next().unwrap_or_default();
        let parts: Vec<&str> = core.split('.').collect();
        assert_eq!(parts.len(), 3, "expected MAJOR.MINOR.PATCH, got {APP_VERSION}");
        for part in parts {
            assert!(
                part.parse::<u32>().is_ok(),
                "non-numeric version component in {APP_VERSION}"
            );
        }
    }
}
