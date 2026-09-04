use crate::blend::BlendMode;
use serde::{Deserialize, Serialize};

pub type LayerId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LayerKind {
    #[default]
    Raster,
    Group,
    Adjustment,
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
}

/// Container for adjustment layer parameters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdjustmentConfig {
    pub adjustment_type: AdjustmentType,
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
    #[serde(skip)]
    pub pixels: Vec<u8>, // RGBA8, len = width * height * 4
}

fn default_expanded() -> bool {
    true
}

impl Layer {
    pub fn new(id: LayerId, name: impl Into<String>, width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize) * 4;
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Raster,
            adjustment: None,
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
            pixels: vec![0; size],
        }
    }

    pub fn new_group(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Group,
            adjustment: None,
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
            pixels: Vec::new(),
        }
    }

    pub fn new_adjustment(id: LayerId, name: impl Into<String>, adj_type: AdjustmentType) -> Self {
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Adjustment,
            adjustment: Some(AdjustmentConfig { adjustment_type: adj_type }),
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
            pixels: Vec::new(),
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
            pixels,
        }
    }

    #[inline]
    pub fn pixel_index(&self, x: u32, y: u32) -> Option<usize> {
        if self.is_raster() && x < self.width && y < self.height {
            let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
            if idx + 3 < self.pixels.len() {
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
        Some([
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ])
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if let Some(idx) = self.pixel_index(x, y) {
            self.pixels[idx] = color[0];
            self.pixels[idx + 1] = color[1];
            self.pixels[idx + 2] = color[2];
            self.pixels[idx + 3] = color[3];
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    pub fn resize(&mut self, new_w: u32, new_h: u32) {
        if self.width == new_w && self.height == new_h {
            return;
        }
        let mut new_pixels = vec![0u8; (new_w as usize) * (new_h as usize) * 4];
        let copy_w = self.width.min(new_w);
        let copy_h = self.height.min(new_h);

        for y in 0..copy_h {
            let src_start = ((y as usize) * (self.width as usize)) * 4;
            let src_end = src_start + (copy_w as usize) * 4;
            let dst_start = ((y as usize) * (new_w as usize)) * 4;
            let dst_end = dst_start + (copy_w as usize) * 4;
            new_pixels[dst_start..dst_end].copy_from_slice(&self.pixels[src_start..src_end]);
        }

        self.width = new_w;
        self.height = new_h;
        self.pixels = new_pixels;
    }

    pub fn duplicate(&self, new_id: LayerId) -> Self {
        Self {
            id: new_id,
            name: format!("{} Copy", self.name),
            kind: self.kind,
            adjustment: self.adjustment.clone(),
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
            pixels: self.pixels.clone(),
        }
    }
}
