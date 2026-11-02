use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32, // 0.0 - 1.0
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
    pub const RED: Self = Self::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Self = Self::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Self = Self::new(0.0, 0.0, 1.0, 1.0);
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    pub const BRAND_GOLD: Self = Self::new(0.9647, 0.7686, 0.3529, 1.0); // #F6C45A
    pub const BRAND_AMBER: Self = Self::new(0.9098, 0.5882, 0.2471, 1.0); // #E8963F
    pub const BRAND_CORAL: Self = Self::new(0.8941, 0.4392, 0.3725, 1.0); // #E4705F
    pub const BRAND_ROSE: Self = Self::new(0.7882, 0.3216, 0.4980, 1.0); // #C9527F
    pub const BRAND_TEAL: Self = Self::new(0.2471, 0.6549, 0.5882, 1.0); // #3FA796
    pub const INK: Self = Self::new(0.0431, 0.0314, 0.0235, 1.0); // #0B0806
    pub const PARCHMENT: Self = Self::new(0.9529, 0.9059, 0.8392, 1.0); // #F3E7D6

    pub fn lerp(&self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::new(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    pub fn to_rgba8(&self) -> [u8; 4] {
        [
            (self.r.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.g.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.b.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        ]
    }

    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim().trim_start_matches('#');
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                Some(Self::from_rgba8(r * 17, g * 17, b * 17, 255))
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4], 16).ok()?;
                Some(Self::from_rgba8(r * 17, g * 17, b * 17, a * 17))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::from_rgba8(r, g, b, 255))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::from_rgba8(r, g, b, a))
            }
            _ => None,
        }
    }

    pub fn to_hex(&self) -> String {
        let [r, g, b, _] = self.to_rgba8();
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    }

    pub fn to_hex_with_alpha(&self) -> String {
        let [r, g, b, a] = self.to_rgba8();
        format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
    }

    pub fn to_hsl(&self) -> (f32, f32, f32) {
        let r = self.r.clamp(0.0, 1.0);
        let g = self.g.clamp(0.0, 1.0);
        let b = self.b.clamp(0.0, 1.0);

        let max = r.max(g.max(b));
        let min = r.min(g.min(b));
        let mut h = 0.0;
        let mut s = 0.0;
        let l = (max + min) / 2.0;

        let d = max - min;
        if d > 0.00001 {
            s = if l > 0.5 {
                d / (2.0 - max - min)
            } else {
                d / (max + min)
            };

            if (max - r).abs() < f32::EPSILON {
                h = (g - b) / d + if g < b { 6.0 } else { 0.0 };
            } else if (max - g).abs() < f32::EPSILON {
                h = (b - r) / d + 2.0;
            } else {
                h = (r - g) / d + 4.0;
            }
            h /= 6.0;
        }

        (h * 360.0, s, l)
    }

    pub fn from_hsl(h: f32, s: f32, l: f32, a: f32) -> Self {
        let h = (h % 360.0 + 360.0) % 360.0;
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);

        if s <= 0.00001 {
            return Self::new(l, l, l, a);
        }

        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let hk = h / 360.0;

        let tc = [hk + 1.0 / 3.0, hk, hk - 1.0 / 3.0];
        let mut rgb = [0.0; 3];

        for (i, t) in tc.iter().enumerate() {
            let mut t = *t;
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }

            if t < 1.0 / 6.0 {
                rgb[i] = p + (q - p) * 6.0 * t;
            } else if t < 1.0 / 2.0 {
                rgb[i] = q;
            } else if t < 2.0 / 3.0 {
                rgb[i] = p + (q - p) * (2.0 / 3.0 - t) * 6.0;
            } else {
                rgb[i] = p;
            }
        }

        Self::new(rgb[0], rgb[1], rgb[2], a)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BRAND_GOLD
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    /// Golden-hour default: amber/gold accents on warm ink.
    Dawn,
    /// Cooler evening variant: rose accents.
    Dusk,
    /// High-contrast warm variant: coral accents.
    Ember,
}

impl ThemeMode {
    pub fn accent_color(&self) -> Color {
        match self {
            Self::Dawn => Color::BRAND_GOLD,
            Self::Dusk => Color::BRAND_ROSE,
            Self::Ember => Color::BRAND_CORAL,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Dawn => "Aurelia Dawn",
            Self::Dusk => "Aurelia Dusk",
            Self::Ember => "Aurelia Ember",
        }
    }
}

/// Aurelia studio palette: warm ink darks, gold/amber primaries, teal complement.
pub const DEFAULT_PALETTE: &[&str] = &[
    "#ffffff", "#f3e7d6", "#e6d3b3", "#d4b483", "#b08d57", "#6b4f32", "#2a201a",
    "#f6c45a", "#e8963f", "#e4705f", "#c9527f", "#9c3d54", "#6d2b3f", "#3fa796",
    "#2f7d74", "#1f5c55", "#a8d5c9", "#7fb3a6", "#f7e08a", "#f2b95c", "#d98c4a",
    "#bf6b3f", "#8c4a2f", "#0b0806",
];
