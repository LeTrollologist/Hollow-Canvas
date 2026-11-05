use crate::blend::BlendMode;
use serde::{Deserialize, Serialize};

pub type LayerId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LayerKind {
    #[default]
    Raster,
    Group,
    Adjustment,
    Text,
}

/// Configuration for a non-destructive, editable Text Layer (similar to GIMP's text layer)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextLayerConfig {
    pub content: String,
    pub font_family: String,
    pub font_size: f32,
    pub color: [u8; 4],
    pub align: crate::brush::TextAlign,
    pub line_spacing: f32,
    pub letter_spacing: f32,
    pub box_w: f32, // 0.0 for auto-fit / unbounded, or >0.0 for bounded paragraph box
    pub box_h: f32,
    pub pos_x: f32, // Position on canvas (origin)
    pub pos_y: f32,
}

impl Default for TextLayerConfig {
    fn default() -> Self {
        Self {
            content: "Text Layer".to_string(),
            font_family: "System Default".to_string(),
            font_size: 32.0,
            color: [255, 255, 255, 255],
            align: crate::brush::TextAlign::Left,
            line_spacing: 1.15,
            letter_spacing: 0.0,
            box_w: 0.0,
            box_h: 0.0,
            pos_x: 40.0,
            pos_y: 40.0,
        }
    }
}

/// Dynamic non-destructive adjustment algorithm configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AdjustmentType {
    BrightnessContrast {
        brightness: f32, // -100.0 to +100.0
        contrast: f32,   // -100.0 to +100.0
    },
    Hsl {
        hue_shift: f32,   // -180.0 to +180.0
        saturation: f32,  // 0.0 to 3.0 (1.0 = normal)
        lightness: f32,   // -1.0 to +1.0 (0.0 = normal)
    },
    ColorBalance {
        cyan_red: f32,      // -100.0 to +100.0
        magenta_green: f32, // -100.0 to +100.0
        yellow_blue: f32,   // -100.0 to +100.0
    },
    Invert,
    Posterize {
        levels: u32, // 2 to 32
    },
    Threshold {
        cutoff: u8, // 0 to 255 (default 128)
    },
    Sepia {
        strength: f32, // 0.0 to 1.0 (default 1.0)
    },
}

impl AdjustmentType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::BrightnessContrast { .. } => "Brightness / Contrast",
            Self::Hsl { .. } => "Hue / Saturation / Lightness",
            Self::ColorBalance { .. } => "Color Balance",
            Self::Invert => "Invert",
            Self::Posterize { .. } => "Posterize",
            Self::Threshold { .. } => "Threshold",
            Self::Sepia { .. } => "Sepia Tone",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::BrightnessContrast { .. } => "✦ Bright/Cont",
            Self::Hsl { .. } => "✦ HSL",
            Self::ColorBalance { .. } => "✦ Color Bal",
            Self::Invert => "✦ Invert",
            Self::Posterize { .. } => "✦ Posterize",
            Self::Threshold { .. } => "✦ Threshold",
            Self::Sepia { .. } => "✦ Sepia",
        }
    }

    /// Computes the adjusted (R, G, B) value for a single pixel
    #[inline]
    pub fn apply_to_rgb(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        match self {
            Self::BrightnessContrast { brightness, contrast } => {
                let c_factor = (259.0 * (*contrast + 255.0)) / (255.0 * (259.0 - *contrast));
                let r_b = (r as f32) + *brightness * 2.55;
                let g_b = (g as f32) + *brightness * 2.55;
                let b_b = (b as f32) + *brightness * 2.55;

                let nr = (c_factor * (r_b - 128.0) + 128.0).clamp(0.0, 255.0).round() as u8;
                let ng = (c_factor * (g_b - 128.0) + 128.0).clamp(0.0, 255.0).round() as u8;
                let nb = (c_factor * (b_b - 128.0) + 128.0).clamp(0.0, 255.0).round() as u8;
                (nr, ng, nb)
            }
            Self::Hsl { hue_shift, saturation, lightness } => {
                let (h, s, l) = crate::filter::rgb_to_hsl(r, g, b);
                let new_h = (h + *hue_shift + 360.0) % 360.0;
                let new_s = (s * *saturation).clamp(0.0, 1.0);
                let new_l = (l + *lightness).clamp(0.0, 1.0);
                crate::filter::hsl_to_rgb(new_h, new_s, new_l)
            }
            Self::ColorBalance { cyan_red, magenta_green, yellow_blue } => {
                let nr = ((r as f32) + *cyan_red * 1.28).clamp(0.0, 255.0).round() as u8;
                let ng = ((g as f32) + *magenta_green * 1.28).clamp(0.0, 255.0).round() as u8;
                let nb = ((b as f32) + *yellow_blue * 1.28).clamp(0.0, 255.0).round() as u8;
                (nr, ng, nb)
            }
            Self::Invert => (255 - r, 255 - g, 255 - b),
            Self::Posterize { levels } => {
                let lvls = (*levels).clamp(2, 32) as f32;
                let step = 255.0 / (lvls - 1.0);
                let nr = ((r as f32 / step).round() * step).clamp(0.0, 255.0).round() as u8;
                let ng = ((g as f32 / step).round() * step).clamp(0.0, 255.0).round() as u8;
                let nb = ((b as f32 / step).round() * step).clamp(0.0, 255.0).round() as u8;
                (nr, ng, nb)
            }
            Self::Threshold { cutoff } => {
                let gray = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as u8;
                let v = if gray >= *cutoff { 255 } else { 0 };
                (v, v, v)
            }
            Self::Sepia { strength } => {
                let rf = r as f32;
                let gf = g as f32;
                let bf = b as f32;
                let sr = (rf * 0.393 + gf * 0.769 + bf * 0.189).min(255.0);
                let sg = (rf * 0.349 + gf * 0.686 + bf * 0.168).min(255.0);
                let sb = (rf * 0.272 + gf * 0.534 + bf * 0.131).min(255.0);
                let st = strength.clamp(0.0, 1.0);
                let nr = (rf * (1.0 - st) + sr * st).clamp(0.0, 255.0).round() as u8;
                let ng = (gf * (1.0 - st) + sg * st).clamp(0.0, 255.0).round() as u8;
                let nb = (bf * (1.0 - st) + sb * st).clamp(0.0, 255.0).round() as u8;
                (nr, ng, nb)
            }
        }
    }

    /// Computes the adjusted (R, G, B) value for a single pixel in working precision.
    ///
    /// This is the 16-bit counterpart of [`AdjustmentType::apply_to_rgb`]. That function is
    /// deliberately **not** rewritten in terms of this one: it is the definition of the eight-bit
    /// output that has already shipped, and moving its arithmetic would move bytes. Instead the
    /// same formulas are expressed on normalised channels, so a 16-bit document can run an
    /// adjustment without first quantising the pixel to 8 bits.
    ///
    /// They are the same function of the same real numbers, but not bit-identical at the 8-bit
    /// boundary: the 8-bit version rounds after scaling to `0..=255`, so a result sitting on a
    /// rounding boundary can land one 8-bit step away. `adjustment_working_matches_eight_bit`
    /// pins the difference to at most one step across a swept grid.
    #[inline]
    pub fn apply_to_rgb_working(&self, rgb: [f32; 3]) -> [f32; 3] {
        let [r, g, b] = rgb;
        match self {
            Self::BrightnessContrast { brightness, contrast } => {
                let c_factor = (259.0 * (*contrast + 255.0)) / (255.0 * (259.0 - *contrast));
                // 2.55 / 255, the 8-bit offset expressed per unit channel.
                let offset = *brightness * (2.55 / 255.0);
                let apply = |c: f32| (c_factor * (c + offset - 0.5) + 0.5).clamp(0.0, 1.0);
                [apply(r), apply(g), apply(b)]
            }
            Self::Hsl { hue_shift, saturation, lightness } => {
                let (h, s, l) = crate::filter::rgb_to_hsl_f32([r, g, b]);
                let new_h = (h + *hue_shift + 360.0) % 360.0;
                let new_s = (s * *saturation).clamp(0.0, 1.0);
                let new_l = (l + *lightness).clamp(0.0, 1.0);
                crate::filter::hsl_to_rgb_f32(new_h, new_s, new_l)
            }
            Self::ColorBalance { cyan_red, magenta_green, yellow_blue } => {
                // 1.28 / 255, the 8-bit offset expressed per unit channel.
                let scale = 1.28 / 255.0;
                [
                    (r + *cyan_red * scale).clamp(0.0, 1.0),
                    (g + *magenta_green * scale).clamp(0.0, 1.0),
                    (b + *yellow_blue * scale).clamp(0.0, 1.0),
                ]
            }
            Self::Invert => [1.0 - r, 1.0 - g, 1.0 - b],
            Self::Posterize { levels } => {
                let levels = (*levels).clamp(2, 32) as f32;
                let step = 1.0 / (levels - 1.0);
                let apply = |c: f32| ((c / step).round() * step).clamp(0.0, 1.0);
                [apply(r), apply(g), apply(b)]
            }
            Self::Threshold { cutoff } => {
                // The cutoff is an 8-bit quantity, so the luminance it is compared against is
                // rounded to the same scale before the comparison.
                let gray = (0.299 * r * 255.0 + 0.587 * g * 255.0 + 0.114 * b * 255.0).round();
                let value = if gray >= *cutoff as f32 { 1.0 } else { 0.0 };
                [value, value, value]
            }
            Self::Sepia { strength } => {
                let sr = (r * 0.393 + g * 0.769 + b * 0.189).min(1.0);
                let sg = (r * 0.349 + g * 0.686 + b * 0.168).min(1.0);
                let sb = (r * 0.272 + g * 0.534 + b * 0.131).min(1.0);
                let st = strength.clamp(0.0, 1.0);
                [
                    (r * (1.0 - st) + sr * st).clamp(0.0, 1.0),
                    (g * (1.0 - st) + sg * st).clamp(0.0, 1.0),
                    (b * (1.0 - st) + sb * st).clamp(0.0, 1.0),
                ]
            }
        }
    }
}

/// Container for adjustment layer parameters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdjustmentConfig {
    pub adjustment_type: AdjustmentType,
}

/// The pixel storage of one layer.
///
/// One variant today. `Srgb16` - gamma-encoded sRGB at 16 bits, *not* linear light - is the
/// planned second, and this enum is where that lands. The point of introducing it now, with
/// a single variant, is that every caller already goes through `Layer::as_srgb8()`; adding a
/// representation then becomes a change inside this file plus an explicit conversion at the
/// edges, instead of a repo-wide migration.
///
/// Note what it deliberately does **not** carry: no width, no height, no colour space. Those
/// belong to the layer (logical size) and to the compositor (interpretation), so storage can
/// change without silently changing meaning.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerBuffer {
    /// RGBA8, row-major, `len == width * height * 4` for the layer's size.
    Srgb8(Vec<u8>),
    /// RGBA16, row-major, **gamma-encoded sRGB at 16 bits - not linear light.**
    ///
    /// The name carries the constraint: storage precision and colour interpretation are
    /// separate decisions, and blending happens on the same encoded values as `Srgb8`. Content
    /// promoted from 8-bit is stored as `v * 257`, which lands exactly on this lattice.
    Srgb16(Vec<u16>),
}

/// Which representation a buffer holds.
///
/// Cheap to copy and compare, so a compositor can resolve the format once per layer rather
/// than matching inside a pixel loop.
///
/// `Default` is `Srgb8`: everything that existed before 16-bit documents is 8-bit, which is
/// also what a serde-defaulted [`Document`](crate::document::Document) field means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PixelFormat {
    #[default]
    Srgb8,
    Srgb16,
}

/// Copies the overlapping region of a buffer into a new size, zero-filling the rest.
///
/// Row-wise, channel-agnostic and non-interpolating: the destination pixel at `(x, y)` is the
/// source pixel at `(x, y)` when both exist. Sharing it between widths is what keeps an 8-bit
/// resize and a 16-bit resize doing the same thing.
fn resized_channels<T: Copy + Default>(
    source: &[T],
    old_w: u32,
    old_h: u32,
    new_w: u32,
    new_h: u32,
) -> Vec<T> {
    let mut out = vec![T::default(); (new_w as usize) * (new_h as usize) * 4];
    let copy_w = (old_w.min(new_w) as usize) * 4;
    let copy_h = old_h.min(new_h) as usize;
    for y in 0..copy_h {
        let src_start = y * (old_w as usize) * 4;
        let dst_start = y * (new_w as usize) * 4;
        let available = source.len().saturating_sub(src_start).min(copy_w);
        if available == 0 {
            break;
        }
        out[dst_start..dst_start + available]
            .copy_from_slice(&source[src_start..src_start + available]);
    }
    out
}

impl Default for LayerBuffer {
    fn default() -> Self {
        LayerBuffer::Srgb8(Vec::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    #[serde(default)]
    pub kind: LayerKind,
    #[serde(default)]
    pub adjustment: Option<AdjustmentConfig>,
    #[serde(default)]
    pub text: Option<TextLayerConfig>,
    #[serde(default)]
    pub parent_id: Option<LayerId>,
    #[serde(default = "default_expanded")]
    pub is_expanded: bool,
    #[serde(default)]
    pub pass_through: bool,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub locked: bool,
    #[serde(default)]
    pub alpha_locked: bool,
    #[serde(default)]
    pub clipping_mask: bool,
    #[serde(default)]
    pub is_reference: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub offset_x: i32,
    pub offset_y: i32,
    /// Private: reach it through `as_srgb8()` or `as_srgb8_mut()`, so the representation can
    /// change without touching callers.
    #[serde(skip)]
    buffer: LayerBuffer,
    /// Optional layer mask applied when compositing.
    #[serde(default)]
    pub mask: Option<LayerMask>,
}

fn default_expanded() -> bool {
    true
}

/// An 8-bit layer mask: 0 hides the layer's pixels, 255 shows them.
///
/// Masks are per-layer and non-destructive: the layer keeps its own pixels and the
/// mask only scales the alpha used when compositing. `enabled` lets a mask be kept
/// but ignored without throwing it away.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerMask {
    pub width: u32,
    pub height: u32,
    /// `width * height` bytes, row-major.
    pub pixels: Vec<u8>,
    pub enabled: bool,
}

impl LayerMask {
    /// A uniform mask (`255` = fully visible).
    pub fn full(width: u32, height: u32, value: u8) -> Self {
        Self {
            width,
            height,
            pixels: vec![value; (width as usize) * (height as usize)],
            enabled: true,
        }
    }

    /// Mask alpha at a canvas coordinate; anything outside the mask is transparent,
    /// and a disabled mask is fully visible.
    #[inline]
    pub fn alpha_at(&self, x: u32, y: u32) -> u8 {
        if !self.enabled {
            return 255;
        }
        if x >= self.width || y >= self.height {
            return 255;
        }
        self.pixels
            .get((y as usize) * (self.width as usize) + x as usize)
            .copied()
            .unwrap_or(255)
    }

    pub fn invert(&mut self) {
        for p in &mut self.pixels {
            *p = 255 - *p;
        }
    }

    /// True when the mask would change nothing (all visible).
    pub fn is_all_visible(&self) -> bool {
        self.pixels.iter().all(|&p| p == 255)
    }
}

impl Layer {
    /// The layer's pixels: RGBA8, row-major, `width * height * 4` bytes.
    ///
    /// The buffer is an implementation detail. Everything outside this module reads and
    /// writes pixels through here, so the storage can gain a 16-bit variant later without
    /// every caller changing.
    /// The representation this layer holds.
    #[inline]
    pub fn format(&self) -> PixelFormat {
        match &self.buffer {
            LayerBuffer::Srgb8(_) => PixelFormat::Srgb8,
            LayerBuffer::Srgb16(_) => PixelFormat::Srgb16,
        }
    }

    /// Read-only view of the buffer, for callers that dispatch on the representation.
    #[inline]
    pub fn buffer(&self) -> &LayerBuffer {
        &self.buffer
    }

    /// Converts the buffer to another storage format.
    ///
    /// 8 -> 16 is exact (`v * 257` lands on the lattice). 16 -> 8 rounds with
    /// `srgb16_to_srgb8`, which *loses* sub-lattice detail, so it is only for a caller that has
    /// deliberately asked to flatten. An empty buffer stays empty in either direction.
    pub fn convert_format(&mut self, format: PixelFormat) {
        if self.format() == format {
            return;
        }
        self.buffer = match (&self.buffer, format) {
            (LayerBuffer::Srgb8(bytes), PixelFormat::Srgb16) => LayerBuffer::Srgb16(
                bytes
                    .iter()
                    .map(|&value| crate::pixel::srgb8_to_srgb16(value))
                    .collect(),
            ),
            (LayerBuffer::Srgb16(channels), PixelFormat::Srgb8) => LayerBuffer::Srgb8(
                channels
                    .iter()
                    .map(|&channel| crate::pixel::srgb16_to_srgb8(channel))
                    .collect(),
            ),
            // Only reachable if a third representation is added; keep the buffer as it is
            // rather than inventing pixels.
            _ => return,
        };
    }

    /// Mutable view of the buffer, for callers that dispatch on the representation.
    #[inline]
    pub fn buffer_mut(&mut self) -> &mut LayerBuffer {
        &mut self.buffer
    }

    /// The pixels, if the layer holds 8-bit sRGB storage.
    ///
    /// Fallible on purpose. A caller that needs 8-bit storage has to say what it does when
    /// the layer is not 8-bit: convert at a boundary, refuse the operation, or use
    /// `as_srgb8_exact()` and document where its invariant comes from.
    #[inline]
    pub fn as_srgb8(&self) -> Option<&[u8]> {
        match &self.buffer {
            LayerBuffer::Srgb8(bytes) => Some(bytes),
            LayerBuffer::Srgb16(_) => None,
        }
    }

    /// Mutable pixels, if the layer holds 8-bit sRGB storage. See `as_srgb8()`.
    #[inline]
    pub fn as_srgb8_mut(&mut self) -> Option<&mut [u8]> {
        match &mut self.buffer {
            LayerBuffer::Srgb8(bytes) => Some(bytes),
            LayerBuffer::Srgb16(_) => None,
        }
    }

    /// The pixels, asserting 8-bit storage. **Never converts, never allocates** - it returns
    /// a view or panics.
    ///
    /// Every current caller is safe for one reason, and it is worth stating once rather than
    /// 170 times: nothing in the application creates a non-`Srgb8` layer yet. Project loading
    /// produces `Srgb8`, new layers are `Srgb8`, and the only `Srgb16` buffers in existence
    /// are the ones the gate harness builds. When AUR v3 can produce 16-bit documents, each
    /// of these call sites must be revisited - which is exactly why they are greppable by
    /// name instead of hidden behind a blanket accessor.
    #[inline]
    pub fn as_srgb8_exact(&self) -> &[u8] {
        match &self.buffer {
            LayerBuffer::Srgb8(bytes) => bytes,
            LayerBuffer::Srgb16(_) => panic!(
                "layer {} is 16-bit storage; this call site assumed 8-bit - convert at the                  boundary or handle both formats",
                self.id
            ),
        }
    }

    /// Mutable variant of `as_srgb8_exact()`. Same invariant, same panic.
    #[inline]
    pub fn as_srgb8_exact_mut(&mut self) -> &mut [u8] {
        match &mut self.buffer {
            LayerBuffer::Srgb8(bytes) => bytes,
            LayerBuffer::Srgb16(_) => panic!(
                "layer {} is 16-bit storage; this call site assumed 8-bit - convert at the                  boundary or handle both formats",
                self.id
            ),
        }
    }

    /// The pixels, if the layer holds 16-bit sRGB storage.
    #[inline]
    pub fn as_srgb16(&self) -> Option<&[u16]> {
        match &self.buffer {
            LayerBuffer::Srgb16(channels) => Some(channels),
            LayerBuffer::Srgb8(_) => None,
        }
    }

    /// Mutable 16-bit pixels. See `as_srgb16()`.
    #[inline]
    pub fn as_srgb16_mut(&mut self) -> Option<&mut [u16]> {
        match &mut self.buffer {
            LayerBuffer::Srgb16(channels) => Some(channels),
            LayerBuffer::Srgb8(_) => None,
        }
    }

    /// Number of storage bytes, whatever the representation.
    #[inline]
    pub fn storage_len(&self) -> usize {
        match &self.buffer {
            LayerBuffer::Srgb8(bytes) => bytes.len(),
            LayerBuffer::Srgb16(channels) => channels.len() * 2,
        }
    }

    /// Copies `out.len()` storage bytes at `offset` into `out`, little-endian per channel.
    ///
    /// This is the format-agnostic byte view history needs: it interprets nothing, so a
    /// delta recorded against 8-bit bytes stays correct if the layer later holds 16-bit
    /// storage. Returns false if the range is out of bounds.
    pub fn read_storage_bytes(&self, offset: usize, out: &mut [u8]) -> bool {
        if offset + out.len() > self.storage_len() {
            return false;
        }
        match &self.buffer {
            LayerBuffer::Srgb8(bytes) => out.copy_from_slice(&bytes[offset..offset + out.len()]),
            LayerBuffer::Srgb16(channels) => {
                for (index, byte) in out.iter_mut().enumerate() {
                    let absolute = offset + index;
                    let channel = channels[absolute / 2];
                    *byte = if absolute % 2 == 0 {
                        channel as u8
                    } else {
                        (channel >> 8) as u8
                    };
                }
            }
        }
        true
    }

    /// The whole buffer as storage bytes, in the format's own layout.
    ///
    /// The format-agnostic counterpart of `as_srgb8_exact().to_vec()`: it lets a caller capture
    /// an edit for history without knowing — or changing — whether the layer is 8-bit or
    /// 16-bit. Pair it with [`LayerPixelsDeltaCommand`](crate::history::LayerPixelsDeltaCommand),
    /// which replays byte spans through `write_storage_bytes`.
    pub fn storage_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0u8; self.storage_len()];
        // `storage_len` is the buffer's own length, so this cannot be out of range.
        self.read_storage_bytes(0, &mut bytes);
        bytes
    }

    /// Writes `src` as storage bytes at `offset`, the inverse of `read_storage_bytes`.
    pub fn write_storage_bytes(&mut self, offset: usize, src: &[u8]) -> bool {
        if offset + src.len() > self.storage_len() {
            return false;
        }
        match &mut self.buffer {
            LayerBuffer::Srgb8(bytes) => bytes[offset..offset + src.len()].copy_from_slice(src),
            LayerBuffer::Srgb16(channels) => {
                for (index, byte) in src.iter().enumerate() {
                    let absolute = offset + index;
                    let channel = &mut channels[absolute / 2];
                    if absolute % 2 == 0 {
                        *channel = (*channel & 0xFF00) | *byte as u16;
                    } else {
                        *channel = (*channel & 0x00FF) | ((*byte as u16) << 8);
                    }
                }
            }
        }
        true
    }

    /// Mutable access to the 8-bit backing store, for changes that alter its length.
    ///
    /// Returns `None` for other representations: a resize must know the format to know what
    /// it is resizing.
    #[inline]
    pub fn pixels_vec_mut(&mut self) -> Option<&mut Vec<u8>> {
        match &mut self.buffer {
            LayerBuffer::Srgb8(bytes) => Some(bytes),
            LayerBuffer::Srgb16(_) => None,
        }
    }

    /// Rebuilds the buffer through a format-preserving mapping.
    ///
    /// Each closure sees the variant's own channels and returns a buffer of the same channel
    /// type, so a structural edit (flip, rotate, crop) is written once per storage width and
    /// never converts. The format is preserved exactly: an `Srgb8` layer stays `Srgb8`, an
    /// `Srgb16` layer stays `Srgb16`.
    ///
    /// The map runs before the assignment, so the closures borrow immutably and the new buffer
    /// replaces the old one at the end.
    pub fn map_buffer<F8, F16>(&mut self, narrow: F8, wide: F16)
    where
        F8: FnOnce(&[u8]) -> Vec<u8>,
        F16: FnOnce(&[u16]) -> Vec<u16>,
    {
        let next = match &self.buffer {
            LayerBuffer::Srgb8(bytes) => LayerBuffer::Srgb8(narrow(bytes)),
            LayerBuffer::Srgb16(channels) => LayerBuffer::Srgb16(wide(channels)),
        };
        self.buffer = next;
    }

    /// Replaces the whole pixel buffer.
    #[inline]
    pub fn set_pixels(&mut self, pixels: Vec<u8>) {
        self.buffer = LayerBuffer::Srgb8(pixels);
    }

    pub fn new(id: LayerId, name: impl Into<String>, width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize) * 4;
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Raster,
            adjustment: None,
            text: None,
            parent_id: None,
            is_expanded: true,
            pass_through: false,
            width,
            height,
            visible: true,
            locked: false,
            alpha_locked: false,
            clipping_mask: false,
            is_reference: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            offset_x: 0,
            offset_y: 0,
            buffer: LayerBuffer::Srgb8(vec![0; size]),
            mask: None,
        }
    }

    pub fn new_group(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Group,
            adjustment: None,
            text: None,
            parent_id: None,
            is_expanded: true,
            pass_through: true,
            width: 0,
            height: 0,
            visible: true,
            locked: false,
            alpha_locked: false,
            clipping_mask: false,
            is_reference: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            offset_x: 0,
            offset_y: 0,
            buffer: LayerBuffer::Srgb8(Vec::new()),
            mask: None,
        }
    }

    pub fn new_adjustment(id: LayerId, name: impl Into<String>, adj_type: AdjustmentType) -> Self {
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Adjustment,
            adjustment: Some(AdjustmentConfig { adjustment_type: adj_type }),
            text: None,
            parent_id: None,
            is_expanded: true,
            pass_through: false,
            width: 0,
            height: 0,
            visible: true,
            locked: false,
            alpha_locked: false,
            clipping_mask: false,
            is_reference: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            offset_x: 0,
            offset_y: 0,
            buffer: LayerBuffer::Srgb8(Vec::new()),
            mask: None,
        }
    }

    pub fn new_text(id: LayerId, name: impl Into<String>, width: u32, height: u32, text_config: TextLayerConfig) -> Self {
        let size = (width as usize) * (height as usize) * 4;
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Text,
            adjustment: None,
            text: Some(text_config),
            parent_id: None,
            is_expanded: true,
            pass_through: false,
            width,
            height,
            visible: true,
            locked: false,
            alpha_locked: false,
            clipping_mask: false,
            is_reference: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            offset_x: 0,
            offset_y: 0,
            buffer: LayerBuffer::Srgb8(vec![0; size]),
            mask: None,
        }
    }

    #[inline]
    pub fn is_group(&self) -> bool {
        self.kind == LayerKind::Group
    }

    #[inline]
    pub fn is_adjustment(&self) -> bool {
        self.kind == LayerKind::Adjustment
    }

    #[inline]
    pub fn is_raster(&self) -> bool {
        self.kind == LayerKind::Raster
    }

    #[inline]
    pub fn is_text(&self) -> bool {
        self.kind == LayerKind::Text
    }

    /// Converts a Text layer into a standard Raster pixel layer, discarding editable text metadata
    pub fn rasterize_text(&mut self) {
        if self.is_text() {
            self.kind = LayerKind::Raster;
            self.text = None;
        }
    }

    pub fn from_pixels(
        id: LayerId,
        name: impl Into<String>,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Raster,
            adjustment: None,
            text: None,
            parent_id: None,
            is_expanded: true,
            pass_through: false,
            width,
            height,
            visible: true,
            locked: false,
            alpha_locked: false,
            clipping_mask: false,
            is_reference: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            offset_x: 0,
            offset_y: 0,
            buffer: LayerBuffer::Srgb8(pixels),
            mask: None,
        }
    }

    #[inline]
    pub fn pixel_index(&self, x: u32, y: u32) -> Option<usize> {
        if (self.is_raster() || self.is_text()) && x < self.width && y < self.height {
            let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
            if idx + 3 < self.storage_len() {
                Some(idx)
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let idx = self.pixel_index(x, y)?;
        let mut bytes = [0u8; 4];
        if !self.read_storage_bytes(idx, &mut bytes) {
            return None;
        }
        Some([
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
        ])
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if let Some(idx) = self.pixel_index(x, y) {
            self.write_storage_bytes(idx, &color);
        }
    }

    pub fn clear(&mut self) {
        match &mut self.buffer {
            LayerBuffer::Srgb8(bytes) => bytes.fill(0),
            LayerBuffer::Srgb16(channels) => channels.fill(0),
        }
    }

    pub fn resize(&mut self, new_w: u32, new_h: u32) {
        if self.width == new_w && self.height == new_h {
            return;
        }
        let (old_w, old_h) = (self.width, self.height);

        // One copy loop for both storage widths: `resized_channels` moves rows of 4 channels and
        // fills new space with zero, which is what the 8-bit path has always done. No sampling,
        // no interpolation - so no interpolation semantics can drift here.
        self.buffer = match &self.buffer {
            LayerBuffer::Srgb8(bytes) => {
                LayerBuffer::Srgb8(resized_channels(bytes, old_w, old_h, new_w, new_h))
            }
            LayerBuffer::Srgb16(channels) => {
                LayerBuffer::Srgb16(resized_channels(channels, old_w, old_h, new_w, new_h))
            }
        };
        self.width = new_w;
        self.height = new_h;
    }

    pub fn duplicate(&self, new_id: LayerId) -> Self {
        Self {
            id: new_id,
            name: format!("{} Copy", self.name),
            kind: self.kind,
            adjustment: self.adjustment.clone(),
            text: self.text.clone(),
            parent_id: self.parent_id,
            is_expanded: self.is_expanded,
            pass_through: self.pass_through,
            width: self.width,
            height: self.height,
            visible: self.visible,
            locked: false,
            alpha_locked: self.alpha_locked,
            clipping_mask: self.clipping_mask,
            is_reference: self.is_reference,
            opacity: self.opacity,
            blend_mode: self.blend_mode,
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            buffer: self.buffer.clone(),
            mask: self.mask.clone(),
        }
    }
}
