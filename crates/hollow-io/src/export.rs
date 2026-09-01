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

pub fn export_animated_gif(
    frames_rgba: &[Vec<u8>],
    width: u32,
    height: u32,
    fps: u32,
    repeat: bool,
    path: impl AsRef<Path>,
) -> Result<(), ExportError> {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, RgbaImage};

    if frames_rgba.is_empty() || width == 0 || height == 0 {
        return Err(ExportError::UnsupportedFormat);
    }
    let file = File::create(path)?;
    let mut encoder = GifEncoder::new(file);
    let rep = if repeat {
        Repeat::Infinite
    } else {
        Repeat::Finite(0)
    };
    encoder.set_repeat(rep)?;

    let delay_ms = (1000.0 / fps.max(1) as f32).round() as u32;
    let delay = Delay::from_numer_denom_ms(delay_ms, 1);

    for frame_data in frames_rgba {
        let img = RgbaImage::from_raw(width, height, frame_data.clone())
            .ok_or(ExportError::UnsupportedFormat)?;
        let frame = Frame::from_parts(img, 0, 0, delay);
        encoder.encode_frame(frame)?;
    }

    Ok(())
}

pub fn export_frame_sequence(
    frames_rgba: &[Vec<u8>],
    width: u32,
    height: u32,
    dir_path: impl AsRef<Path>,
    prefix: &str,
    format: ExportFormat,
) -> Result<Vec<std::path::PathBuf>, ExportError> {
    use image::RgbaImage;

    let dir = dir_path.as_ref();
    std::fs::create_dir_all(dir)?;
    let mut saved_paths = Vec::new();

    for (i, frame_data) in frames_rgba.iter().enumerate() {
        let img = RgbaImage::from_raw(width, height, frame_data.clone())
            .ok_or(ExportError::UnsupportedFormat)?;
        let filename = format!("{}_{:04}.{}", prefix, i + 1, format.extension());
        let full_path = dir.join(filename);
        let mut file = File::create(&full_path)?;
        img.write_to(&mut file, format.image_format())?;
        saved_paths.push(full_path);
    }

    Ok(saved_paths)
}
