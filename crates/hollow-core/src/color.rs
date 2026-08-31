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
    pub const HOLLOW_PURPLE: Self = Self::new(0.6588, 0.6235, 0.8471, 1.0); // #a89fd8

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
        Self::HOLLOW_PURPLE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    DeepMist,
    Moonlit,
    EmberGlow,
}

impl ThemeMode {
    pub fn accent_color(&self) -> Color {
        match self {
            Self::DeepMist => Color::from_hex("#a89fd8").unwrap(),
            Self::Moonlit => Color::from_hex("#7eb8f7").unwrap(),
            Self::EmberGlow => Color::from_hex("#f0a96a").unwrap(),
        }
    }
}

pub const DEFAULT_PALETTE: &[&str] = &[
    "#ffffff", "#d0d8f0", "#a89fd8", "#7c6fb0", "#453575", "#130f30",
    "#7eb8f7", "#38bdf8", "#22d3ee", "#2dd4bf", "#4ade80", "#a3e635",
    "#fdba74", "#f97316", "#f87171", "#e879f9", "#c084fc", "#818cf8",
    "#fef08a", "#fcd34d", "#6ee7b7", "#f0abfc", "#f9a8d4", "#060810",
];
