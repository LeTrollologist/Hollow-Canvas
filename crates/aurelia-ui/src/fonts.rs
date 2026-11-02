//! Runtime typography.
//!
//! egui ships with Ubuntu + Noto by default, which reads a little generic next to
//! native Windows chrome. The system already carries Segoe UI (Regular, Semibold,
//! Light) and Cascadia Mono, licensed for applications running on the machine, so
//! the studio prefers those and keeps egui's bundled faces as fallbacks for
//! glyphs they do not cover (emoji, symbols) and for non-Windows builds.

use std::sync::atomic::{AtomicBool, Ordering};

use egui::{FontData, FontDefinitions, FontFamily};

/// Semibold face, used for headings and section captions.
pub const FAMILY_DISPLAY: &str = "display";
/// Light face, used for the wordmark and large numerals.
pub const FAMILY_BRAND: &str = "brand";

static INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn display() -> FontFamily {
    FontFamily::Name(FAMILY_DISPLAY.into())
}

pub fn brand() -> FontFamily {
    FontFamily::Name(FAMILY_BRAND.into())
}

/// Reads the first available font out of `%WINDIR%\Fonts`.
fn system_font(candidates: &[&str]) -> Option<Vec<u8>> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    for name in candidates {
        let path = std::path::Path::new(&windir).join("Fonts").join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            if !bytes.is_empty() {
                log::debug!("loaded typeface {name}");
                return Some(bytes);
            }
        }
    }
    None
}

/// Installs the font families. Safe to call more than once; only the first call
/// touches the context (and therefore the atlas).
pub fn install(ctx: &egui::Context) {
    if INSTALLED.swap(true, Ordering::Relaxed) {
        return;
    }

    let mut fonts = FontDefinitions::default();
    let mut proportional_fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mono_fallbacks = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    let regular = system_font(&["segoeui.ttf"]);
    let semibold = system_font(&["seguisb.ttf", "segoeuib.ttf"]);
    let light = system_font(&["seguisl.ttf", "segoeuil.ttf"]);
    let mono = system_font(&["CascadiaMono.ttf", "consola.ttf"]);

    if let Some(bytes) = regular {
        fonts
            .font_data
            .insert("ui".to_owned(), FontData::from_owned(bytes));
        proportional_fallbacks.insert(0, "ui".to_owned());
    }
    if let Some(bytes) = semibold {
        fonts
            .font_data
            .insert("ui-semibold".to_owned(), FontData::from_owned(bytes));
    }
    if let Some(bytes) = light {
        fonts
            .font_data
            .insert("ui-light".to_owned(), FontData::from_owned(bytes));
    }
    if let Some(bytes) = mono {
        fonts
            .font_data
            .insert("ui-mono".to_owned(), FontData::from_owned(bytes));
        let mut list = mono_fallbacks.clone();
        list.insert(0, "ui-mono".to_owned());
        fonts.families.insert(FontFamily::Monospace, list);
    }

    // Heading face: semibold, then regular, then whatever egui bundles.
    let mut display_list: Vec<String> = Vec::new();
    for face in ["ui-semibold", "ui"] {
        if fonts.font_data.contains_key(face) {
            display_list.push(face.to_owned());
        }
    }
    display_list.extend(proportional_fallbacks.iter().cloned());
    fonts
        .families
        .insert(display(), display_list);

    // Wordmark face: light, then regular.
    let mut brand_list: Vec<String> = Vec::new();
    for face in ["ui-light", "ui"] {
        if fonts.font_data.contains_key(face) {
            brand_list.push(face.to_owned());
        }
    }
    brand_list.extend(proportional_fallbacks.iter().cloned());
    fonts.families.insert(brand(), brand_list);

    fonts
        .families
        .insert(FontFamily::Proportional, proportional_fallbacks);
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_names_are_stable() {
        // The names are referenced from the stylesheet and from RichText calls.
        assert_eq!(FAMILY_DISPLAY, "display");
        assert_eq!(FAMILY_BRAND, "brand");
        assert_ne!(display(), FontFamily::Proportional);
    }
}
