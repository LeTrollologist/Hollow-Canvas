use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use aurelia_core::blend::BlendMode;
use aurelia_core::color::ThemeMode;
use aurelia_core::document::{Document, EditorState, GuideLine, StraightRulerState};
use aurelia_core::layer::Layer;
use image::{ImageBuffer, ImageFormat, Rgba};
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use thiserror::Error;

const AUR_MAGIC_V1: &[u8; 4] = b"AUR\x01";
/// v2 appends a per-layer mask section (flag byte, enabled byte, width*height bytes).
const AUR_MAGIC_V2: &[u8; 4] = b"AUR\x02";
const AUR_MAGIC_V3: &[u8; 4] = b"AUR\x03";

/// Document and per-layer format tags in AUR v3.
///
/// `INHERIT` is what an ordinary document writes for every layer: the layer uses the
/// document's format. The explicit values exist so a document *can* express a layer of a
/// different representation - the editor does not create such documents yet, but the format
/// round-trips them, which is what keeps mixed formats an additive change rather than another
/// format version.
const FORMAT_SRGB8: u8 = 0;
const FORMAT_INHERIT: u8 = 0;
const LAYER_FORMAT_SRGB8: u8 = 1;
const LAYER_FORMAT_SRGB16: u8 = 2;
const HCV_MAGIC_V2: &[u8; 4] = b"HCV\x02";
const HCV_MAGIC_V3: &[u8; 4] = b"HCV\x03";
const HCV_MAGIC_V4: &[u8; 4] = b"HCV\x04";
const HCV_MAGIC_V5: &[u8; 4] = b"HCV\x05";
/// Optional editor-state section appended after the preview. Older readers stop at the
/// preview, so this is an additive trailing section rather than another container version.
const EDITOR_MAGIC: &[u8; 4] = b"AED1";
const EDITOR_MAX_BYTES: u32 = 1024 * 1024;

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Image encode error: {0}")]
    Image(#[from] image::ImageError),
    #[error("Invalid .aur binary container signature")]
    InvalidMagic,
    #[error("Corrupt or unsupported .aur project data")]
    CorruptData,
    #[error("This project version cannot represent the document's storage format")]
    UnsupportedFormat,
}

pub fn save_project_to_writer<W: Write>(doc: &Document, writer: W) -> Result<(), ProjectError> {
    write_project_version(doc, writer, 3)
}

/// Writes a project in a specific format version.
///
/// Versions 1 and 2 exist so tests can produce *authentic* old files with the real writer
/// instead of transforming today's bytes into something that only looks old. The differences
/// are all here: v2 added the mask section, v3 added the format tags.
pub(crate) fn write_project_version<W: Write>(
    doc: &Document,
    mut writer: W,
    version: u8,
) -> Result<(), ProjectError> {
    match version {
        1 => writer.write_all(AUR_MAGIC_V1)?,
        2 => writer.write_all(AUR_MAGIC_V2)?,
        _ => writer.write_all(AUR_MAGIC_V3)?,
    }
    writer.write_all(&doc.width.to_le_bytes())?;
    writer.write_all(&doc.height.to_le_bytes())?;
    writer.write_all(&doc.active_layer_id.to_le_bytes())?;
    writer.write_all(&[doc.background_value])?;
    writer.write_all(&[if doc.is_transparent { 1 } else { 0 }])?;

    let theme_byte = match doc.theme {
        ThemeMode::Dawn => 0u8,
        ThemeMode::Dusk => 1u8,
        ThemeMode::Ember => 2u8,
    };
    writer.write_all(&[theme_byte])?;

    // AUR v3: the document's storage format, then one byte per layer that either inherits it
    // or names its own. A pure 8-bit document writes zeros throughout, which is exactly what
    // older readers of the reserved byte would expect.
    //
    // The tag records `doc.pixel_format` - what the editor is creating in - rather than being
    // derived from the layers. Deriving it lost the format as soon as the last 16-bit layer
    // was deleted, so a 16-bit document could silently save as 8-bit.
    let document_is_16bit = doc.pixel_format == aurelia_core::layer::PixelFormat::Srgb16;
    let any_16bit_layer = doc
        .layers
        .iter()
        .any(|layer| layer.format() == aurelia_core::layer::PixelFormat::Srgb16);
    if version >= 3 {
        writer.write_all(&[if document_is_16bit { 1u8 } else { FORMAT_SRGB8 }])?;
    } else if document_is_16bit || any_16bit_layer {
        // Refuse rather than silently flatten: an old format cannot express 16-bit channels.
        return Err(ProjectError::UnsupportedFormat);
    }

    let layer_count = doc.layers.len() as u32;
    writer.write_all(&layer_count.to_le_bytes())?;

    for layer in &doc.layers {
        writer.write_all(&layer.id.to_le_bytes())?;
        
        let name_bytes = layer.name.as_bytes();
        let name_len = name_bytes.len() as u16;
        writer.write_all(&name_len.to_le_bytes())?;
        writer.write_all(name_bytes)?;

        // Kind (0 = Raster, 1 = Group, 2 = Adjustment, 3 = Text)
        let kind_byte = match layer.kind {
            aurelia_core::layer::LayerKind::Raster => 0u8,
            aurelia_core::layer::LayerKind::Group => 1u8,
            aurelia_core::layer::LayerKind::Adjustment => 2u8,
            aurelia_core::layer::LayerKind::Text => 3u8,
        };
        writer.write_all(&[kind_byte])?;

        // If text layer, serialize text configuration
        if let Some(txt) = &layer.text {
            let content_bytes = txt.content.as_bytes();
            writer.write_all(&(content_bytes.len() as u32).to_le_bytes())?;
            writer.write_all(content_bytes)?;

            let font_bytes = txt.font_family.as_bytes();
            writer.write_all(&(font_bytes.len() as u16).to_le_bytes())?;
            writer.write_all(font_bytes)?;

            writer.write_all(&txt.font_size.to_le_bytes())?;
            writer.write_all(&txt.color)?;

            let align_byte = match txt.align {
                aurelia_core::brush::TextAlign::Left => 0u8,
                aurelia_core::brush::TextAlign::Center => 1u8,
                aurelia_core::brush::TextAlign::Right => 2u8,
            };
            writer.write_all(&[align_byte])?;
            writer.write_all(&txt.line_spacing.to_le_bytes())?;
            writer.write_all(&txt.letter_spacing.to_le_bytes())?;
            writer.write_all(&txt.box_w.to_le_bytes())?;
            writer.write_all(&txt.box_h.to_le_bytes())?;
            writer.write_all(&txt.pos_x.to_le_bytes())?;
            writer.write_all(&txt.pos_y.to_le_bytes())?;
        }

        // If adjustment, serialize adjustment parameters
        if let Some(adj) = &layer.adjustment {
            match &adj.adjustment_type {
                aurelia_core::layer::AdjustmentType::BrightnessContrast { brightness, contrast } => {
                    writer.write_all(&[0u8])?;
                    writer.write_all(&brightness.to_le_bytes())?;
                    writer.write_all(&contrast.to_le_bytes())?;
                }
                aurelia_core::layer::AdjustmentType::Hsl { hue_shift, saturation, lightness } => {
                    writer.write_all(&[1u8])?;
                    writer.write_all(&hue_shift.to_le_bytes())?;
                    writer.write_all(&saturation.to_le_bytes())?;
                    writer.write_all(&lightness.to_le_bytes())?;
                }
                aurelia_core::layer::AdjustmentType::ColorBalance { cyan_red, magenta_green, yellow_blue } => {
                    writer.write_all(&[2u8])?;
                    writer.write_all(&cyan_red.to_le_bytes())?;
                    writer.write_all(&magenta_green.to_le_bytes())?;
                    writer.write_all(&yellow_blue.to_le_bytes())?;
                }
                aurelia_core::layer::AdjustmentType::Invert => {
                    writer.write_all(&[3u8])?;
                }
                aurelia_core::layer::AdjustmentType::Posterize { levels } => {
                    writer.write_all(&[4u8])?;
                    writer.write_all(&levels.to_le_bytes())?;
                }
                aurelia_core::layer::AdjustmentType::Threshold { cutoff } => {
                    writer.write_all(&[5u8])?;
                    writer.write_all(&[*cutoff])?;
                }
                aurelia_core::layer::AdjustmentType::Sepia { strength } => {
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

        // Per-layer format: inherit the document's, or name this layer's own.
        let layer_is_16bit = layer.format() == aurelia_core::layer::PixelFormat::Srgb16;
        let layer_tag = if version < 3 {
            FORMAT_INHERIT
        } else if layer.is_group() || layer.is_adjustment() {
            FORMAT_INHERIT
        } else if layer_is_16bit == document_is_16bit {
            FORMAT_INHERIT
        } else if layer_is_16bit {
            LAYER_FORMAT_SRGB16
        } else {
            LAYER_FORMAT_SRGB8
        };
        if version >= 3 {
            writer.write_all(&[layer_tag])?;
        } else if layer_tag != FORMAT_INHERIT {
            return Err(ProjectError::UnsupportedFormat);
        }

        if layer.is_group() || layer.is_adjustment() {
            // Groups and Adjustments have no pixel buffer
            let comp_len = 0u32;
            writer.write_all(&comp_len.to_le_bytes())?;
        } else {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
            match layer.buffer() {
                aurelia_core::layer::LayerBuffer::Srgb8(bytes) => encoder.write_all(bytes)?,
                aurelia_core::layer::LayerBuffer::Srgb16(channels) => {
                    // Little-endian per channel, which is the same byte order the loader and
                    // the history bridge use.
                    for channel in channels {
                        encoder.write_all(&channel.to_le_bytes())?;
                    }
                }
            }
            let compressed = encoder.finish()?;

            let comp_len = compressed.len() as u32;
            writer.write_all(&comp_len.to_le_bytes())?;
            writer.write_all(&compressed)?;
        }

        // Layer mask (AUR v2+): 0 = none, 1 = enabled flag + raw mask bytes.
        if version >= 2 {
        match &layer.mask {
            Some(mask)
                if mask.pixels.len() == (doc.width as usize) * (doc.height as usize)
                    && mask.width == doc.width
                    && mask.height == doc.height =>
            {
                writer.write_all(&[1u8])?;
                writer.write_all(&[u8::from(mask.enabled)])?;
                writer.write_all(&mask.pixels)?;
            }
            _ => writer.write_all(&[0u8])?,
        }
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

    write_editor_state(&doc.editor, &mut writer)?;
    Ok(())
}

fn write_editor_state<W: Write>(editor: &EditorState, writer: &mut W) -> Result<(), ProjectError> {
    let mut payload = Vec::new();
    payload.push(1u8); // editor-state payload version
    payload.extend_from_slice(&(editor.guides.len() as u32).to_le_bytes());
    for guide in &editor.guides {
        payload.push(u8::from(guide.vertical));
        payload.extend_from_slice(&guide.position.to_le_bytes());
    }
    payload.extend_from_slice(&editor.grid_origin[0].to_le_bytes());
    payload.extend_from_slice(&editor.grid_origin[1].to_le_bytes());
    match &editor.straight_ruler {
        Some(ruler) => {
            payload.push(1u8);
            payload.extend_from_slice(&ruler.origin[0].to_le_bytes());
            payload.extend_from_slice(&ruler.origin[1].to_le_bytes());
            payload.extend_from_slice(&ruler.angle_rad.to_le_bytes());
            payload.extend_from_slice(&ruler.length.to_le_bytes());
        }
        None => payload.push(0u8),
    }
    writer.write_all(EDITOR_MAGIC)?;
    writer.write_all(&(payload.len() as u32).to_le_bytes())?;
    writer.write_all(&payload)?;
    Ok(())
}

/// Skips the preview PNG and reads the optional trailing editor section. Older files have
/// nothing after the preview, and unreadable/absent sections simply leave editor defaults.
fn read_editor_state<R: Read>(reader: &mut R) -> Option<EditorState> {
    let mut preview_len = [0u8; 4];
    if reader.read_exact(&mut preview_len).is_err() {
        return None;
    }
    let preview_len = u32::from_le_bytes(preview_len);
    if preview_len > 256 * 1024 * 1024 {
        return None;
    }
    if std::io::copy(
        &mut reader.by_ref().take(preview_len as u64),
        &mut std::io::sink(),
    )
    .ok()? != preview_len as u64
    {
        return None;
    }

    let mut magic = [0u8; 4];
    if reader.read_exact(&mut magic).is_err() || &magic != EDITOR_MAGIC {
        return None;
    }
    let mut length = [0u8; 4];
    reader.read_exact(&mut length).ok()?;
    let length = u32::from_le_bytes(length);
    if length > EDITOR_MAX_BYTES {
        return None;
    }
    let mut payload = vec![0u8; length as usize];
    reader.read_exact(&mut payload).ok()?;
    parse_editor_payload(&payload)
}

fn parse_editor_payload(payload: &[u8]) -> Option<EditorState> {
    let mut cursor = Cursor::new(payload);
    let mut byte = [0u8; 1];
    cursor.read_exact(&mut byte).ok()?;
    if byte[0] != 1 {
        return None;
    }
    let mut count = [0u8; 4];
    cursor.read_exact(&mut count).ok()?;
    let count = u32::from_le_bytes(count);
    if count > 4096 {
        return None;
    }
    let mut editor = EditorState::default();
    for _ in 0..count {
        cursor.read_exact(&mut byte).ok()?;
        let vertical = byte[0] != 0;
        let mut position = [0u8; 4];
        cursor.read_exact(&mut position).ok()?;
        let position = f32::from_le_bytes(position);
        if !position.is_finite() {
            return None;
        }
        editor.guides.push(GuideLine {
            vertical,
            position,
        });
    }
    let mut origin = [0u8; 8];
    cursor.read_exact(&mut origin).ok()?;
    editor.grid_origin = [
        f32::from_le_bytes(origin[0..4].try_into().ok()?),
        f32::from_le_bytes(origin[4..8].try_into().ok()?),
    ];
    if !editor.grid_origin.iter().all(|value| value.is_finite()) {
        return None;
    }
    cursor.read_exact(&mut byte).ok()?;
    if byte[0] != 0 {
        let mut numbers = [0u8; 16];
        cursor.read_exact(&mut numbers).ok()?;
        let ruler = StraightRulerState {
            origin: [
                f32::from_le_bytes(numbers[0..4].try_into().ok()?),
                f32::from_le_bytes(numbers[4..8].try_into().ok()?),
            ],
            angle_rad: f32::from_le_bytes(numbers[8..12].try_into().ok()?),
            length: f32::from_le_bytes(numbers[12..16].try_into().ok()?),
        };
        if !ruler.origin.iter().all(|value| value.is_finite())
            || !ruler.angle_rad.is_finite()
            || !ruler.length.is_finite()
        {
            return None;
        }
        editor.straight_ruler = Some(ruler);
    }
    Some(editor)
}

/// Writes a project file atomically: the stream is written to a sibling
/// `.aur.tmp` file and renamed over the target only after it is fully flushed,
/// so an interrupted save can never truncate an existing artwork.
pub fn save_project_file(doc: &Document, path: impl AsRef<Path>) -> Result<(), ProjectError> {
    let path = path.as_ref();
    let mut tmp_name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    tmp_name.push(".tmp");
    let tmp_path = path.with_file_name(tmp_name);

    let result = (|| -> Result<(), ProjectError> {
        let file = File::create(&tmp_path)?;
        save_project_to_writer(doc, file)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

pub fn load_project_from_reader<R: Read>(mut reader: R) -> Result<Document, ProjectError> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    let is_aur_v3 = &magic == AUR_MAGIC_V3;
    let is_aur_v2 = &magic == AUR_MAGIC_V2;
    let is_aur = &magic == AUR_MAGIC_V1 || is_aur_v2 || is_aur_v3;
    // AUR v1 reuses the legacy HCV v5 body layout, so legacy files stay readable.
    let is_v5 = &magic == HCV_MAGIC_V5 || is_aur;
    let is_v4 = &magic == HCV_MAGIC_V4;
    let is_v3 = &magic == HCV_MAGIC_V3;
    let is_v2 = &magic == HCV_MAGIC_V2;
    if !is_aur && !is_v5 && !is_v4 && !is_v3 && !is_v2 {
        return Err(ProjectError::InvalidMagic);
    }

    let mut buf4 = [0u8; 4];
    let mut buf8 = [0u8; 8];
    let mut buf1 = [0u8; 1];

    reader.read_exact(&mut buf4)?;
    let width = u32::from_le_bytes(buf4);

    reader.read_exact(&mut buf4)?;
    let height = u32::from_le_bytes(buf4);

    // A corrupt header must not be allowed to drive an allocation. The cap is per layer and
    // generous (8 bytes per pixel covers 16-bit RGBA) but finite.
    let layer_bytes = (width as u64)
        .checked_mul(height as u64)
        .and_then(|pixels| pixels.checked_mul(8));
    match layer_bytes {
        Some(bytes) if width > 0 && height > 0 && bytes <= (1u64 << 31) => {}
        _ => return Err(ProjectError::CorruptData),
    }

    reader.read_exact(&mut buf8)?;
    let active_layer_id = u64::from_le_bytes(buf8);

    reader.read_exact(&mut buf1)?;
    let background_value = buf1[0];

    reader.read_exact(&mut buf1)?;
    let is_transparent = buf1[0] != 0;

    reader.read_exact(&mut buf1)?;
    let theme = match buf1[0] {
        1 => ThemeMode::Dusk,
        2 => ThemeMode::Ember,
        _ => ThemeMode::Dawn,
    };

    // AUR v3 stores the document's storage format; older versions were implicitly 8-bit.
    let document_format = if is_aur_v3 {
        reader.read_exact(&mut buf1)?;
        match buf1[0] {
            0 => aurelia_core::layer::PixelFormat::Srgb8,
            1 => aurelia_core::layer::PixelFormat::Srgb16,
            _ => return Err(ProjectError::CorruptData),
        }
    } else {
        aurelia_core::layer::PixelFormat::Srgb8
    };

    reader.read_exact(&mut buf4)?;
    let layer_count = u32::from_le_bytes(buf4);

    // The count comes straight from the file. Reserving for all of it up front would let a
    // tiny header ask for terabytes before the first layer (and its EOF) is ever read.
    let mut layers = Vec::with_capacity((layer_count as usize).min(1024));
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
        let mut text_config = None;

        let (kind, parent_id, visible, locked, alpha_locked, clipping_mask, is_reference, is_expanded, pass_through) = if is_v5 {
            reader.read_exact(&mut buf1)?;
            let kind = match buf1[0] {
                1 => aurelia_core::layer::LayerKind::Group,
                2 => {
                    reader.read_exact(&mut buf1)?;
                    let adj_type = match buf1[0] {
                        0 => {
                            reader.read_exact(&mut buf4)?;
                            let brightness = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let contrast = f32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::BrightnessContrast { brightness, contrast }
                        }
                        1 => {
                            reader.read_exact(&mut buf4)?;
                            let hue_shift = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let saturation = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let lightness = f32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::Hsl { hue_shift, saturation, lightness }
                        }
                        2 => {
                            reader.read_exact(&mut buf4)?;
                            let cyan_red = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let magenta_green = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let yellow_blue = f32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::ColorBalance { cyan_red, magenta_green, yellow_blue }
                        }
                        3 => aurelia_core::layer::AdjustmentType::Invert,
                        4 => {
                            reader.read_exact(&mut buf4)?;
                            let levels = u32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::Posterize { levels }
                        }
                        5 => {
                            reader.read_exact(&mut buf1)?;
                            let cutoff = buf1[0];
                            aurelia_core::layer::AdjustmentType::Threshold { cutoff }
                        }
                        6 => {
                            reader.read_exact(&mut buf4)?;
                            let strength = f32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::Sepia { strength }
                        }
                        _ => aurelia_core::layer::AdjustmentType::Invert,
                    };
                    adjustment = Some(aurelia_core::layer::AdjustmentConfig { adjustment_type: adj_type });
                    aurelia_core::layer::LayerKind::Adjustment
                }
                3 => {
                    reader.read_exact(&mut buf4)?;
                    let content_len = u32::from_le_bytes(buf4) as usize;
                    let mut content_vec = vec![0u8; content_len];
                    reader.read_exact(&mut content_vec)?;
                    let content = String::from_utf8(content_vec).unwrap_or_default();

                    reader.read_exact(&mut buf2)?;
                    let font_len = u16::from_le_bytes(buf2) as usize;
                    let mut font_vec = vec![0u8; font_len];
                    reader.read_exact(&mut font_vec)?;
                    let font_family = String::from_utf8(font_vec).unwrap_or_default();

                    reader.read_exact(&mut buf4)?;
                    let font_size = f32::from_le_bytes(buf4);

                    let mut col_buf = [0u8; 4];
                    reader.read_exact(&mut col_buf)?;

                    reader.read_exact(&mut buf1)?;
                    let align = match buf1[0] {
                        1 => aurelia_core::brush::TextAlign::Center,
                        2 => aurelia_core::brush::TextAlign::Right,
                        _ => aurelia_core::brush::TextAlign::Left,
                    };

                    reader.read_exact(&mut buf4)?;
                    let line_spacing = f32::from_le_bytes(buf4);

                    reader.read_exact(&mut buf4)?;
                    let letter_spacing = f32::from_le_bytes(buf4);

                    reader.read_exact(&mut buf4)?;
                    let box_w = f32::from_le_bytes(buf4);

                    reader.read_exact(&mut buf4)?;
                    let box_h = f32::from_le_bytes(buf4);

                    reader.read_exact(&mut buf4)?;
                    let pos_x = f32::from_le_bytes(buf4);

                    reader.read_exact(&mut buf4)?;
                    let pos_y = f32::from_le_bytes(buf4);

                    text_config = Some(aurelia_core::layer::TextLayerConfig {
                        content,
                        font_family,
                        font_size,
                        color: col_buf,
                        align,
                        line_spacing,
                        letter_spacing,
                        box_w,
                        box_h,
                        pos_x,
                        pos_y,
                    });

                    aurelia_core::layer::LayerKind::Text
                }
                _ => aurelia_core::layer::LayerKind::Raster,
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
        } else if is_v4 {
            reader.read_exact(&mut buf1)?;
            let kind = match buf1[0] {
                1 => aurelia_core::layer::LayerKind::Group,
                2 => {
                    reader.read_exact(&mut buf1)?;
                    let adj_type = match buf1[0] {
                        0 => {
                            reader.read_exact(&mut buf4)?;
                            let brightness = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let contrast = f32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::BrightnessContrast { brightness, contrast }
                        }
                        1 => {
                            reader.read_exact(&mut buf4)?;
                            let hue_shift = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let saturation = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let lightness = f32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::Hsl { hue_shift, saturation, lightness }
                        }
                        2 => {
                            reader.read_exact(&mut buf4)?;
                            let cyan_red = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let magenta_green = f32::from_le_bytes(buf4);
                            reader.read_exact(&mut buf4)?;
                            let yellow_blue = f32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::ColorBalance { cyan_red, magenta_green, yellow_blue }
                        }
                        3 => aurelia_core::layer::AdjustmentType::Invert,
                        4 => {
                            reader.read_exact(&mut buf4)?;
                            let levels = u32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::Posterize { levels }
                        }
                        5 => {
                            reader.read_exact(&mut buf1)?;
                            let cutoff = buf1[0];
                            aurelia_core::layer::AdjustmentType::Threshold { cutoff }
                        }
                        6 => {
                            reader.read_exact(&mut buf4)?;
                            let strength = f32::from_le_bytes(buf4);
                            aurelia_core::layer::AdjustmentType::Sepia { strength }
                        }
                        _ => aurelia_core::layer::AdjustmentType::Invert,
                    };
                    adjustment = Some(aurelia_core::layer::AdjustmentConfig { adjustment_type: adj_type });
                    aurelia_core::layer::LayerKind::Adjustment
                }
                _ => aurelia_core::layer::LayerKind::Raster,
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
                aurelia_core::layer::LayerKind::Group
            } else {
                aurelia_core::layer::LayerKind::Raster
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

            (aurelia_core::layer::LayerKind::Raster, None, visible, locked, false, false, false, true, false)
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

        // v3: inherit the document format, or name this layer's own.
        let layer_format = if is_aur_v3 {
            reader.read_exact(&mut buf1)?;
            match buf1[0] {
                FORMAT_INHERIT => document_format,
                LAYER_FORMAT_SRGB8 => aurelia_core::layer::PixelFormat::Srgb8,
                LAYER_FORMAT_SRGB16 => aurelia_core::layer::PixelFormat::Srgb16,
                _ => return Err(ProjectError::CorruptData),
            }
        } else {
            aurelia_core::layer::PixelFormat::Srgb8
        };

        reader.read_exact(&mut buf4)?;
        let comp_len = u32::from_le_bytes(buf4) as usize;

        let mut layer = if kind == aurelia_core::layer::LayerKind::Adjustment {
            let adj_type = adjustment
                .as_ref()
                .map(|a| a.adjustment_type.clone())
                .unwrap_or(aurelia_core::layer::AdjustmentType::Invert);
            let mut adj_layer = Layer::new_adjustment(id, name, adj_type);
            adj_layer.is_expanded = is_expanded;
            adj_layer
        } else if kind == aurelia_core::layer::LayerKind::Group {
            let mut group = Layer::new_group(id, name);
            group.is_expanded = is_expanded;
            group.pass_through = pass_through;
            group
        } else {
            // The header check above caps width*height, so the exact payload size is known
            // before a single compressed byte is decoded. Bound the decoder to it: a small
            // high-ratio deflate stream must not be able to allocate gigabytes before the
            // length check below gets a chance to reject it.
            let expected = match layer_format {
                aurelia_core::layer::PixelFormat::Srgb8 => {
                    (width as usize) * (height as usize) * 4
                }
                aurelia_core::layer::PixelFormat::Srgb16 => {
                    (width as usize) * (height as usize) * 8
                }
            };

            let mut compressed = reader.by_ref().take(comp_len as u64);
            let mut raw = Vec::new();
            {
                let mut decoder =
                    DeflateDecoder::new(&mut compressed).take(expected as u64 + 1);
                decoder.read_to_end(&mut raw)?;
            }
            // A deflate stream can end before comp_len; discard the remainder so the reader
            // is positioned at the mask section that follows.
            std::io::copy(&mut compressed, &mut std::io::sink())?;

            if raw.len() != expected {
                return Err(ProjectError::CorruptData);
            }
            match layer_format {
                aurelia_core::layer::PixelFormat::Srgb8 => {
                    Layer::from_pixels(id, name, width, height, raw)
                }
                aurelia_core::layer::PixelFormat::Srgb16 => {
                    // A 16-bit payload that is short or long is corrupt, not a reason to
                    // reinterpret it as 8-bit.
                    let channels: Vec<u16> = raw
                        .chunks_exact(2)
                        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                        .collect();
                    let mut layer = Layer::from_pixels(id, name, width, height, Vec::new());
                    *layer.buffer_mut() = aurelia_core::layer::LayerBuffer::Srgb16(channels);
                    layer
                }
            }
        };

        layer.kind = kind;
        layer.adjustment = adjustment;
        layer.text = text_config;
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

        if is_aur_v2 || is_aur_v3 {
            reader.read_exact(&mut buf1)?;
            if buf1[0] == 1 {
                reader.read_exact(&mut buf1)?;
                let enabled = buf1[0] != 0;
                let mask_len = (width as usize) * (height as usize);
                let mut mask_pixels = vec![0u8; mask_len];
                reader.read_exact(&mut mask_pixels)?;
                layer.mask = Some(aurelia_core::layer::LayerMask {
                    width,
                    height,
                    pixels: mask_pixels,
                    enabled,
                });
            }
        }

        layers.push(layer);
    }

    let editor = read_editor_state(&mut reader).unwrap_or_default();

    Ok(Document {
        width,
        height,
        layers,
        active_layer_id,
        background_value,
        is_transparent,
        theme,
        next_layer_id: max_id + 1,
        // The tag is authoritative for new layers; per-layer tags above resolved to their own
        // formats, so a mixed file still loads exactly as it was saved.
        pixel_format: document_format,
        editor,
    })
}

pub fn load_project_file(path: impl AsRef<Path>) -> Result<Document, ProjectError> {
    let file = File::open(path)?;
    load_project_from_reader(file)
}
