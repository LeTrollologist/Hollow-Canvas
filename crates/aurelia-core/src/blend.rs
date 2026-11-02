use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum BlendMode {
    #[default]
    #[serde(rename = "source-over")]
    Normal,
    #[serde(rename = "multiply")]
    Multiply,
    #[serde(rename = "screen")]
    Screen,
    #[serde(rename = "overlay")]
    Overlay,
    #[serde(rename = "darken")]
    Darken,
    #[serde(rename = "lighten")]
    Lighten,
    #[serde(rename = "color-dodge")]
    ColorDodge,
    #[serde(rename = "color-burn")]
    ColorBurn,
    #[serde(rename = "hard-light")]
    HardLight,
    #[serde(rename = "soft-light")]
    SoftLight,
    #[serde(rename = "difference")]
    Difference,
    #[serde(rename = "exclusion")]
    Exclusion,
    #[serde(rename = "luminosity")]
    Luminosity,
}

impl BlendMode {
    pub const ALL: &'static [BlendMode] = &[
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Luminosity,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::Darken => "Darken",
            Self::Lighten => "Lighten",
            Self::ColorDodge => "Color Dodge",
            Self::ColorBurn => "Color Burn",
            Self::HardLight => "Hard Light",
            Self::SoftLight => "Soft Light",
            Self::Difference => "Difference",
            Self::Exclusion => "Exclusion",
            Self::Luminosity => "Luminosity",
        }
    }

    pub fn from_str_name(s: &str) -> Self {
        match s {
            "multiply" => Self::Multiply,
            "screen" => Self::Screen,
            "overlay" => Self::Overlay,
            "darken" => Self::Darken,
            "lighten" => Self::Lighten,
            "color-dodge" => Self::ColorDodge,
            "color-burn" => Self::ColorBurn,
            "hard-light" => Self::HardLight,
            "soft-light" => Self::SoftLight,
            "difference" => Self::Difference,
            "exclusion" => Self::Exclusion,
            "luminosity" => Self::Luminosity,
            _ => Self::Normal,
        }
    }

    /// Blend single RGB component in 0.0..=1.0 range
    #[inline]
    pub fn blend_channel(&self, cb: f32, cs: f32) -> f32 {
        match self {
            Self::Normal => cs,
            Self::Multiply => cb * cs,
            Self::Screen => cb + cs - cb * cs,
            Self::Overlay => {
                if cb <= 0.5 {
                    2.0 * cb * cs
                } else {
                    1.0 - 2.0 * (1.0 - cb) * (1.0 - cs)
                }
            }
            Self::Darken => cb.min(cs),
            Self::Lighten => cb.max(cs),
            Self::ColorDodge => {
                if cb <= 0.0 {
                    0.0
                } else if cs >= 1.0 {
                    1.0
                } else {
                    (cb / (1.0 - cs)).min(1.0)
                }
            }
            Self::ColorBurn => {
                if cb >= 1.0 {
                    1.0
                } else if cs <= 0.0 {
                    0.0
                } else {
                    1.0 - ((1.0 - cb) / cs).min(1.0)
                }
            }
            Self::HardLight => {
                if cs <= 0.5 {
                    2.0 * cb * cs
                } else {
                    1.0 - 2.0 * (1.0 - cb) * (1.0 - cs)
                }
            }
            Self::SoftLight => {
                if cs <= 0.5 {
                    cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
                } else {
                    let d = if cb <= 0.25 {
                        ((16.0 * cb - 12.0) * cb + 4.0) * cb
                    } else {
                        cb.sqrt()
                    };
                    cb + (2.0 * cs - 1.0) * (d - cb)
                }
            }
            Self::Difference => (cb - cs).abs(),
            Self::Exclusion => cb + cs - 2.0 * cb * cs,
            Self::Luminosity => cs, // Luminosity handled in full RGB conversion
        }
    }

    /// Full pixel compositing: dst is background, src is foreground with layer opacity
    pub fn composite_pixel(&self, dst: [u8; 4], src: [u8; 4], layer_opacity: f32) -> [u8; 4] {
        let sa = (src[3] as f32 / 255.0) * layer_opacity.clamp(0.0, 1.0);
        if sa <= 0.0001 {
            return dst;
        }

        let da = dst[3] as f32 / 255.0;
        let out_a = sa + da * (1.0 - sa);
        if out_a <= 0.0001 {
            return [0, 0, 0, 0];
        }

        let sr = src[0] as f32 / 255.0;
        let sg = src[1] as f32 / 255.0;
        let sb = src[2] as f32 / 255.0;

        let dr = dst[0] as f32 / 255.0;
        let dg = dst[1] as f32 / 255.0;
        let db = dst[2] as f32 / 255.0;

        let (br, bg, bb) = match self {
            Self::Luminosity => {
                let lum_dst = 0.299 * dr + 0.587 * dg + 0.114 * db;
                let lum_src = 0.299 * sr + 0.587 * sg + 0.114 * sb;
                let diff = lum_src - lum_dst;
                let mut cr = dr + diff;
                let mut cg = dg + diff;
                let mut cb_val = db + diff;
                // W3C ClipColor: proportionally scale towards luma to stay in gamut
                let lum = 0.299 * cr + 0.587 * cg + 0.114 * cb_val;
                let cmin = cr.min(cg).min(cb_val);
                let cmax = cr.max(cg).max(cb_val);
                if cmin < 0.0 && (lum - cmin).abs() > 1e-6 {
                    let s = lum / (lum - cmin);
                    cr = lum + (cr - lum) * s;
                    cg = lum + (cg - lum) * s;
                    cb_val = lum + (cb_val - lum) * s;
                }
                if cmax > 1.0 && (cmax - lum).abs() > 1e-6 {
                    let s = (1.0 - lum) / (cmax - lum);
                    cr = lum + (cr - lum) * s;
                    cg = lum + (cg - lum) * s;
                    cb_val = lum + (cb_val - lum) * s;
                }
                (cr.clamp(0.0, 1.0), cg.clamp(0.0, 1.0), cb_val.clamp(0.0, 1.0))
            }
            _ => (
                self.blend_channel(dr, sr).clamp(0.0, 1.0),
                self.blend_channel(dg, sg).clamp(0.0, 1.0),
                self.blend_channel(db, sb).clamp(0.0, 1.0),
            ),
        };

        // Alpha compositing formula:
        // C_out = (1 - sa) * C_dst + (1 - da) * C_src + sa * da * B(C_dst, C_src)
        let cr = ((1.0 - sa) * da * dr + (1.0 - da) * sa * sr + sa * da * br) / out_a;
        let cg = ((1.0 - sa) * da * dg + (1.0 - da) * sa * sg + sa * da * bg) / out_a;
        let cb = ((1.0 - sa) * da * db + (1.0 - da) * sa * sb + sa * da * bb) / out_a;

        [
            (cr.clamp(0.0, 1.0) * 255.0).round() as u8,
            (cg.clamp(0.0, 1.0) * 255.0).round() as u8,
            (cb.clamp(0.0, 1.0) * 255.0).round() as u8,
            (out_a.clamp(0.0, 1.0) * 255.0).round() as u8,
        ]
    }
}
