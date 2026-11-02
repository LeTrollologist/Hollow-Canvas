use aurelia_core::document::Document;
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
    export_flat_image_with_quality(doc, path, format, include_background, None)
}

/// Same as [`export_flat_image`], but lets the caller pick JPEG quality (1..=100).
///
/// JPEG has no alpha channel, so the composite is flattened onto white before
/// encoding (writing RGBA straight into the JPEG encoder is unsupported).
pub fn export_flat_image_with_quality(
    doc: &Document,
    path: impl AsRef<Path>,
    format: ExportFormat,
    include_background: bool,
    jpeg_quality: Option<u8>,
) -> Result<(), ExportError> {
    let pixels = doc.composite_layers(include_background);
    write_rgba_to_path(&pixels, doc.width, doc.height, path, format, jpeg_quality)
}

fn write_rgba_to_path(
    pixels: &[u8],
    width: u32,
    height: u32,
    path: impl AsRef<Path>,
    format: ExportFormat,
    jpeg_quality: Option<u8>,
) -> Result<(), ExportError> {
    if width == 0 || height == 0 {
        return Err(ExportError::UnsupportedFormat);
    }

    match format {
        ExportFormat::Jpeg => {
            let mut rgb = Vec::with_capacity((width as usize) * (height as usize) * 3);
            for chunk in pixels.chunks_exact(4) {
                let alpha = chunk[3] as u32;
                for c in 0..3 {
                    // Alpha-composite over opaque white.
                    let blended = (chunk[c] as u32 * alpha + 255 * (255 - alpha)) / 255;
                    rgb.push(blended as u8);
                }
            }
            let quality = jpeg_quality.unwrap_or(90).clamp(1, 100);
            let file = File::create(path)?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality);
            encoder.encode(&rgb, width, height, image::ExtendedColorType::Rgb8)?;
        }
        _ => {
            let img: ImageBuffer<Rgba<u8>, &[u8]> = ImageBuffer::from_raw(width, height, pixels)
                .ok_or(ExportError::UnsupportedFormat)?;
            let mut file = File::create(path)?;
            img.write_to(&mut file, format.image_format())?;
        }
    }

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
    let dir = dir_path.as_ref();
    std::fs::create_dir_all(dir)?;
    let mut saved_paths = Vec::new();

    for (i, frame_data) in frames_rgba.iter().enumerate() {
        let filename = format!("{}_{:04}.{}", prefix, i + 1, format.extension());
        let full_path = dir.join(filename);
        write_rgba_to_path(frame_data, width, height, &full_path, format, None)?;
        saved_paths.push(full_path);
    }

    Ok(saved_paths)
}

/// Packs every frame into a single grid (sprite sheet) PNG and returns its dimensions.
///
/// Frames are laid out left-to-right, top-to-bottom with `columns` cells per row;
/// unused trailing cells stay fully transparent.
pub fn export_sprite_sheet(
    frames_rgba: &[Vec<u8>],
    width: u32,
    height: u32,
    columns: u32,
    path: impl AsRef<Path>,
) -> Result<(u32, u32), ExportError> {
    if frames_rgba.is_empty() || width == 0 || height == 0 {
        return Err(ExportError::UnsupportedFormat);
    }

    let cols = columns.max(1).min(frames_rgba.len() as u32);
    let rows = (frames_rgba.len() as u32).div_ceil(cols);
    let sheet_w = width * cols;
    let sheet_h = height * rows;
    let mut sheet = vec![0u8; (sheet_w as usize) * (sheet_h as usize) * 4];
    let row_bytes = (width as usize) * 4;

    for (i, frame) in frames_rgba.iter().enumerate() {
        let cx = (i as u32 % cols) * width;
        let cy = (i as u32 / cols) * height;
        if frame.len() < row_bytes * height as usize {
            return Err(ExportError::UnsupportedFormat);
        }
        for y in 0..height as usize {
            let src_start = y * row_bytes;
            let dst_start = ((cy as usize + y) * sheet_w as usize + cx as usize) * 4;
            sheet[dst_start..dst_start + row_bytes]
                .copy_from_slice(&frame[src_start..src_start + row_bytes]);
        }
    }

    let file = File::create(path)?;
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(sheet_w, sheet_h, sheet).ok_or(ExportError::UnsupportedFormat)?;
    img.write_to(&mut std::io::BufWriter::new(file), ImageFormat::Png)?;

    Ok((sheet_w, sheet_h))
}
