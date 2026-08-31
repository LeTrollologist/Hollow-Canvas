use hollow_core::document::Document;
use image::{ImageBuffer, ImageFormat, Rgba};
use std::fs::File;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("Unsupported image format")]
    UnsupportedFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Jpeg,
    WebP,
}

impl ExportFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
        }
    }

    pub fn image_format(&self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::WebP => ImageFormat::WebP,
        }
    }
}

pub fn export_flat_image(
    doc: &Document,
    path: impl AsRef<Path>,
    format: ExportFormat,
    include_background: bool,
) -> Result<(), ExportError> {
    let pixels = doc.composite_layers(include_background);
    let img: ImageBuffer<Rgba<u8>, &[u8]> = ImageBuffer::from_raw(doc.width, doc.height, &pixels[..])
        .ok_or(ExportError::UnsupportedFormat)?;

    let mut file = File::create(path)?;
    img.write_to(&mut file, format.image_format())?;
    Ok(())
}
