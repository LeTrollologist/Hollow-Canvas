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

const HCV_MAGIC_V2: &[u8; 4] = b"HCV\x02";
const HCV_MAGIC_V3: &[u8; 4] = b"HCV\x03";
const HCV_MAGIC_V4: &[u8; 4] = b"HCV\x04";

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
    writer.write_all(HCV_MAGIC_V4)?;
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

        // Kind (0 = Raster, 1 = Group, 2 = Adjustment)
        let kind_byte = match layer.kind {
            hollow_core::layer::LayerKind::Raster => 0u8,
            hollow_core::layer::LayerKind::Group => 1u8,
            hollow_core::layer::LayerKind::Adjustment => 2u8,
        };
        writer.write_all(&[kind_byte])?;

        // If adjustment, serialize adjustment parameters
        if let Some(adj) = &layer.adjustment {
            match &adj.adjustment_type {
                hollow_core::layer::AdjustmentType::BrightnessContrast { brightness, contrast } => {
                    writer.write_all(&[0u8])?;
                    writer.write_all(&brightness.to_le_bytes())?;
                    writer.write_all(&contrast.to_le_bytes())?;
                }
                hollow_core::layer::AdjustmentType::Hsl { hue_shift, saturation, lightness } => {
                    writer.write_all(&[1u8])?;
                    writer.write_all(&hue_shift.to_le_bytes())?;
                    writer.write_all(&saturation.to_le_bytes())?;
                    writer.write_all(&lightness.to_le_bytes())?;
                }
                hollow_core::layer::AdjustmentType::ColorBalance { cyan_red, magenta_green, yellow_blue } => {
                    writer.write_all(&[2u8])?;
                    writer.write_all(&cyan_red.to_le_bytes())?;
                    writer.write_all(&magenta_green.to_le_bytes())?;
                    writer.write_all(&yellow_blue.to_le_bytes())?;
                }
                hollow_core::layer::AdjustmentType::Invert => {
                    writer.write_all(&[3u8])?;
                }
                hollow_core::layer::AdjustmentType::Posterize { levels } => {
                    writer.write_all(&[4u8])?;
                    writer.write_all(&levels.to_le_bytes())?;
                }
                hollow_core::layer::AdjustmentType::Threshold { cutoff } => {
                    writer.write_all(&[5u8])?;
                    writer.write_all(&[*cutoff])?;
                }
                hollow_core::layer::AdjustmentType::Sepia { strength } => {
                    writer.write_all(&[6u8])?;
                    writer.write_all(&strength.to_le_bytes())?;
                }
            }
        }

        // Parent ID (0 if None)
        let pid = layer.parent_id.unwrap_or(0);
        writer.write_all(&pid.to_le_bytes())?;

        // Flags bitmask
        let mut flags = 0u8;
        if layer.visible { flags |= 1 << 0; }
        if layer.locked { flags |= 1 << 1; }
        if layer.alpha_locked { flags |= 1 << 2; }
        if layer.clipping_mask { flags |= 1 << 3; }
        if layer.is_reference { flags |= 1 << 4; }
        if layer.is_expanded { flags |= 1 << 5; }
        if layer.pass_through { flags |= 1 << 6; }
        writer.write_all(&[flags])?;

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

        if layer.is_group() || layer.is_adjustment() {
            // Groups and Adjustments have no pixel buffer
            let comp_len = 0u32;
            writer.write_all(&comp_len.to_le_bytes())?;
        } else {
            // Compress RGBA pixel stream using Deflate
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(&layer.pixels)?;
            let compressed = encoder.finish()?;

            let comp_len = compressed.len() as u32;
            writer.write_all(&comp_len.to_le_bytes())?;
            writer.write_all(&compressed)?;
        }
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
    let is_v4 = &magic == HCV_MAGIC_V4;
    let is_v3 = &magic == HCV_MAGIC_V3;
    let is_v2 = &magic == HCV_MAGIC_V2;
    if !is_v4 && !is_v3 && !is_v2 {
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

        let mut adjustment = None;

        let (kind, parent_id, visible, locked, alpha_locked, clipping_mask, is_reference, is_expanded, pass_through) = if is_v4 {
            reader.read_exact(&mut buf1)?;
            let kind = match buf1[0] {
                1 => hollow_core::layer::LayerKind::Group,
                2 => {
                    reader.read_exact(&mut buf1)?;
                    let adj_type = match buf1[0] {
                        0 => {
                            reader.read_exact(&mut buf4)?;
                            let brightness = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let contrast = f32::from_le_bytes(buf4);
                            hollow_core::layer::AdjustmentType::BrightnessContrast { brightness, contrast }
                        }
                        1 => {
                            reader.read_exact(&mut buf4)?;
                            let hue_shift = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let saturation = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let lightness = f32::from_le_bytes(buf4);
                            hollow_core::layer::AdjustmentType::Hsl { hue_shift, saturation, lightness }
                        }
                        2 => {
                            reader.read_exact(&mut buf4)?;
                            let cyan_red = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let magenta_green = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let yellow_blue = f32::from_le_bytes(buf4);
                            hollow_core::layer::AdjustmentType::ColorBalance { cyan_red, magenta_green, yellow_blue }
                        }
                        3 => hollow_core::layer::AdjustmentType::Invert,
                        4 => {
                            reader.read_exact(&mut buf4)?;
                            let levels = u32::from_le_bytes(buf4);
                            hollow_core::layer::AdjustmentType::Posterize { levels }
                        }
                        5 => {
                            reader.read_exact(&mut buf1)?;
                            let cutoff = buf1[0];
                            hollow_core::layer::AdjustmentType::Threshold { cutoff }
                        }
                        6 => {
                            reader.read_exact(&mut buf4)?;
                            let strength = f32::from_le_bytes(buf4);
                            hollow_core::layer::AdjustmentType::Sepia { strength }
                        }
                        _ => hollow_core::layer::AdjustmentType::Invert,
                    };
                    adjustment = Some(hollow_core::layer::AdjustmentConfig { adjustment_type: adj_type });
                    hollow_core::layer::LayerKind::Adjustment
                }
                _ => hollow_core::layer::LayerKind::Raster,
            };

            reader.read_exact(&mut buf8)?;
            let raw_pid = u64::from_le_bytes(buf8);
            let parent_id = if raw_pid > 0 { Some(raw_pid) } else { None };

            reader.read_exact(&mut buf1)?;
            let flags = buf1[0];
            let visible = (flags & (1 << 0)) != 0;
            let locked = (flags & (1 << 1)) != 0;
            let alpha_locked = (flags & (1 << 2)) != 0;
            let clipping_mask = (flags & (1 << 3)) != 0;
            let is_reference = (flags & (1 << 4)) != 0;
            let is_expanded = (flags & (1 << 5)) != 0;
            let pass_through = (flags & (1 << 6)) != 0;

            (kind, parent_id, visible, locked, alpha_locked, clipping_mask, is_reference, is_expanded, pass_through)
        } else if is_v3 {
            reader.read_exact(&mut buf1)?;
            let kind = if buf1[0] == 1 {
                hollow_core::layer::LayerKind::Group
            } else {
                hollow_core::layer::LayerKind::Raster
            };

            reader.read_exact(&mut buf8)?;
            let raw_pid = u64::from_le_bytes(buf8);
            let parent_id = if raw_pid > 0 { Some(raw_pid) } else { None };

            reader.read_exact(&mut buf1)?;
            let flags = buf1[0];
            let visible = (flags & (1 << 0)) != 0;
            let locked = (flags & (1 << 1)) != 0;
            let alpha_locked = (flags & (1 << 2)) != 0;
            let clipping_mask = (flags & (1 << 3)) != 0;
            let is_reference = (flags & (1 << 4)) != 0;
            let is_expanded = (flags & (1 << 5)) != 0;
            let pass_through = (flags & (1 << 6)) != 0;

            (kind, parent_id, visible, locked, alpha_locked, clipping_mask, is_reference, is_expanded, pass_through)
        } else {
            reader.read_exact(&mut buf1)?;
            let visible = buf1[0] != 0;

            reader.read_exact(&mut buf1)?;
            let locked = buf1[0] != 0;

            (hollow_core::layer::LayerKind::Raster, None, visible, locked, false, false, false, true, false)
        };

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

        let mut layer = if kind == hollow_core::layer::LayerKind::Adjustment {
            let adj_type = adjustment
                .as_ref()
                .map(|a| a.adjustment_type.clone())
                .unwrap_or(hollow_core::layer::AdjustmentType::Invert);
            let mut adj_layer = Layer::new_adjustment(id, name, adj_type);
            adj_layer.is_expanded = is_expanded;
            adj_layer
        } else if kind == hollow_core::layer::LayerKind::Group {
            let mut group = Layer::new_group(id, name);
            group.is_expanded = is_expanded;
            group.pass_through = pass_through;
            group
        } else {
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

            Layer::from_pixels(id, name, width, height, pixels)
        };

        layer.kind = kind;
        layer.adjustment = adjustment;
        layer.parent_id = parent_id;
        layer.visible = visible;
        layer.locked = locked;
        layer.alpha_locked = alpha_locked;
        layer.clipping_mask = clipping_mask;
        layer.is_reference = is_reference;
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
