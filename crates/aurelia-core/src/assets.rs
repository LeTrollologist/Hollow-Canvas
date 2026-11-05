//! Bundled binary assets.
//!
//! Two shapes, deliberately kept separate:
//!
//! * **Bundled** — compiled into the binary with `include_bytes!`, so a test or a default can
//!   never depend on what happens to be installed on the machine. The bundled DejaVu Sans is
//!   the reference font for text rendering tests, and the last-resort fallback at runtime.
//! * **Imported** — read from a path the user chose, which is what brush imports will use.
//!
//! Both arrive as bytes through [`AssetSource`] -> [`read`]. A container decoder (`.abr`
//! brush stamps, and whatever else) is the *next* step and belongs to the importer that
//! understands the format; nothing here guesses at pixels, so there is no second file loader
//! to add later.
//!
//! License: DejaVu Sans is redistributed under the Bitstream Vera license, reproduced in
//! `assets/fonts/LICENSE_DEJAVU.txt` alongside the file.

use std::path::Path;

/// The bundled reference font, used by text tests and as a final runtime fallback.
///
/// A test that renders text must not depend on the fonts installed on the machine, or its
/// expected output is only valid on one machine. This is the font that makes a text digest
/// meaningful.
pub static DEJAVU_SANS: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");

/// Where asset bytes come from.
#[derive(Debug, Clone, Copy)]
pub enum AssetSource<'a> {
    /// Compiled into the binary.
    Bundled(&'static [u8]),
    /// A file the user pointed at.
    File(&'a Path),
}

/// The bundled reference font.
pub fn bundled_font() -> &'static [u8] {
    DEJAVU_SANS
}

/// Reads an asset's bytes.
///
/// The one place file access happens for imported assets, so a future decoder receives bytes
/// and nothing else - it cannot invent its own error handling, path rules or encoding.
pub fn read(source: &AssetSource<'_>) -> Option<Vec<u8>> {
    match source {
        AssetSource::Bundled(bytes) => Some(bytes.to_vec()),
        AssetSource::File(path) => std::fs::read(path).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::{bundled_font, read, AssetSource};

    #[test]
    fn the_bundled_font_is_a_truetype_file() {
        let bytes = bundled_font();
        assert!(bytes.len() > 1024, "the bundled font is suspiciously small");
        // sfnt version 1.0 or 'true' - either is a valid TrueType container.
        let tag = &bytes[..4];
        assert!(
            tag == [0x00, 0x01, 0x00, 0x00] || tag == b"true",
            "unexpected font container tag: {tag:?}"
        );
    }

    #[test]
    fn read_returns_the_bundled_bytes_and_missing_files_do_not_panic() {
        assert_eq!(read(&AssetSource::Bundled(bundled_font())), Some(bundled_font().to_vec()));

        let missing = std::path::Path::new("definitely/not/a/file.ttf");
        assert_eq!(read(&AssetSource::File(missing)), None);
    }
}
