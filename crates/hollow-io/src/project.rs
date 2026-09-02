use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use hollow_core::blend::BlendMode;
use hollow_core::color::ThemeMode;
use hollow_core::document::Document;
use hollow_core::layer::Layer;
use image::{ImageBuffer, ImageFormat, Rgba};
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use thiserror::Error;

const HCV_MAGIC: &[u8; 4] = b"HCV\x02";

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Image encode error: {0}")]
    Image(#[from] image::ImageError),
    #[error("Invalid .hcv binary container signature")]
    InvalidMagic,
    #[error("Corrupt or unsupported .hcv project data")]
    CorruptData,
}

pub fn save_project_to_writer<W: Write>(doc: &Document, mut writer: W) -> Result<(), ProjectError> {
    writer.write_all(HCV_MAGIC)?;
    writer.write_all(&doc.width.to_le_bytes())?;
    writer.write_all(&doc.height.to_le_bytes())?;
    writer.write_all(&doc.active_layer_id.to_le_bytes())?;
    writer.write_all(&[doc.background_value])?;
    writer.write_all(&[if doc.is_transparent { 1 } else { 0 }])?;

    let theme_byte = match doc.theme {
        ThemeMode::DeepMist => 0u8,
        ThemeMode::Moonlit => 1u8,
        ThemeMode::EmberGlow => 2u8,
    };
    writer.write_all(&[theme_byte])?;

    let layer_count = doc.layers.len() as u32;
    writer.write_all(&layer_count.to_le_bytes())?;

    for layer in &doc.layers {
        writer.write_all(&layer.id.to_le_bytes())?;
        
        let name_bytes = layer.name.as_bytes();
        let name_len = name_bytes.len() as u16;
        writer.write_all(&name_len.to_le_bytes())?;
        writer.write_all(name_bytes)?;

        writer.write_all(&[if layer.visible { 1 } else { 0 }])?;
        writer.write_all(&[if layer.locked { 1 } else { 0 }])?;
        writer.write_all(&layer.opacity.to_le_bytes())?;

        let blend_byte = match layer.blend_mode {
            BlendMode::Normal => 0u8,
            BlendMode::Multiply => 1u8,
            BlendMode::Screen => 2u8,
            BlendMode::Overlay => 3u8,
            BlendMode::Darken => 4u8,
            BlendMode::Lighten => 5u8,
            BlendMode::ColorDodge => 6u8,
            BlendMode::ColorBurn => 7u8,
            BlendMode::HardLight => 8u8,
            BlendMode::SoftLight => 9u8,
            BlendMode::Difference => 10u8,
            BlendMode::Exclusion => 11u8,
            BlendMode::Luminosity => 12u8,
        };
        writer.write_all(&[blend_byte])?;
        writer.write_all(&layer.offset_x.to_le_bytes())?;
        writer.write_all(&layer.offset_y.to_le_bytes())?;

        // Compress RGBA pixel stream using Deflate
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&layer.pixels)?;
        let compressed = encoder.finish()?;

        let comp_len = compressed.len() as u32;
        writer.write_all(&comp_len.to_le_bytes())?;
        writer.write_all(&compressed)?;
    }

    // Save flat preview thumbnail
    let thumb_flat = doc.composite_layers(true);
    let thumb_img: ImageBuffer<Rgba<u8>, &[u8]> = ImageBuffer::from_raw(doc.width, doc.height, &thumb_flat[..])
        .ok_or(ProjectError::CorruptData)?;
    let mut thumb_bytes = Cursor::new(Vec::new());
    thumb_img.write_to(&mut thumb_bytes, ImageFormat::Png)?;
    let thumb_ref = thumb_bytes.get_ref();
    let thumb_len = thumb_ref.len() as u32;
    writer.write_all(&thumb_len.to_le_bytes())?;
    writer.write_all(thumb_ref)?;

    Ok(())
}

pub fn save_project_file(doc: &Document, path: impl AsRef<Path>) -> Result<(), ProjectError> {
    let file = File::create(path)?;
    save_project_to_writer(doc, file)
}

pub fn load_project_from_reader<R: Read>(mut reader: R) -> Result<Document, ProjectError> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != HCV_MAGIC {
        return Err(ProjectError::InvalidMagic);
    }

    let mut buf4 = [0u8; 4];
    let mut buf8 = [0u8; 8];
    let mut buf1 = [0u8; 1];

    reader.read_exact(&mut buf4)?;
    let width = u32::from_le_bytes(buf4);

    reader.read_exact(&mut buf4)?;
    let height = u32::from_le_bytes(buf4);

    reader.read_exact(&mut buf8)?;
    let active_layer_id = u64::from_le_bytes(buf8);

    reader.read_exact(&mut buf1)?;
    let background_value = buf1[0];

    reader.read_exact(&mut buf1)?;
    let is_transparent = buf1[0] != 0;

    reader.read_exact(&mut buf1)?;
    let theme = match buf1[0] {
        1 => ThemeMode::Moonlit,
        2 => ThemeMode::EmberGlow,
        _ => ThemeMode::DeepMist,
    };

    reader.read_exact(&mut buf4)?;
    let layer_count = u32::from_le_bytes(buf4);

    let mut layers = Vec::with_capacity(layer_count as usize);
    let mut max_id = active_layer_id;

    for _ in 0..layer_count {
        reader.read_exact(&mut buf8)?;
        let id = u64::from_le_bytes(buf8);
        if id > max_id {
            max_id = id;
        }

        let mut buf2 = [0u8; 2];
        reader.read_exact(&mut buf2)?;
        let name_len = u16::from_le_bytes(buf2) as usize;

        let mut name_vec = vec![0u8; name_len];
        reader.read_exact(&mut name_vec)?;
        let name = String::from_utf8(name_vec).unwrap_or_else(|_| format!("Layer {}", id));

        reader.read_exact(&mut buf1)?;
        let visible = buf1[0] != 0;

        reader.read_exact(&mut buf1)?;
        let locked = buf1[0] != 0;

        reader.read_exact(&mut buf4)?;
        let opacity = f32::from_le_bytes(buf4);

        reader.read_exact(&mut buf1)?;
        let blend_mode = match buf1[0] {
            1 => BlendMode::Multiply,
            2 => BlendMode::Screen,
            3 => BlendMode::Overlay,
            4 => BlendMode::Darken,
            5 => BlendMode::Lighten,
            6 => BlendMode::ColorDodge,
            7 => BlendMode::ColorBurn,
            8 => BlendMode::HardLight,
            9 => BlendMode::SoftLight,
            10 => BlendMode::Difference,
            11 => BlendMode::Exclusion,
            12 => BlendMode::Luminosity,
            _ => BlendMode::Normal,
        };

        reader.read_exact(&mut buf4)?;
        let offset_x = i32::from_le_bytes(buf4);

        reader.read_exact(&mut buf4)?;
        let offset_y = i32::from_le_bytes(buf4);

        reader.read_exact(&mut buf4)?;
        let comp_len = u32::from_le_bytes(buf4) as usize;

        let mut comp_bytes = vec![0u8; comp_len];
        reader.read_exact(&mut comp_bytes)?;

        // Decompress pixels
        let mut decoder = DeflateDecoder::new(&comp_bytes[..]);
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        decoder.read_to_end(&mut pixels)?;

        let expected_len = (width * height * 4) as usize;
        if pixels.len() != expected_len {
            return Err(ProjectError::CorruptData);
        }

        let mut layer = Layer::from_pixels(id, name, width, height, pixels);
        layer.visible = visible;
        layer.locked = locked;
        layer.opacity = opacity;
        layer.blend_mode = blend_mode;
        layer.offset_x = offset_x;
        layer.offset_y = offset_y;

        layers.push(layer);
    }

    Ok(Document {
        width,
        height,
        layers,
        active_layer_id,
        background_value,
        is_transparent,
        theme,
        next_layer_id: max_id + 1,
    })
}

pub fn load_project_file(path: impl AsRef<Path>) -> Result<Document, ProjectError> {
    let file = File::open(path)?;
    load_project_from_reader(file)
}
