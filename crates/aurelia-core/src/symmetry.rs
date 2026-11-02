use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SymmetryMode {
    #[default]
    None,
    Horizontal,
    Vertical,
    Quad,
}

impl SymmetryMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Horizontal => "Horizontal",
            Self::Vertical => "Vertical",
            Self::Quad => "Quad",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SymmetryConfig {
    pub mode: SymmetryMode,
    pub mandala_segments: u32, // 0 = off, 2..=24
}

impl Default for SymmetryConfig {
    fn default() -> Self {
        Self {
            mode: SymmetryMode::None,
            mandala_segments: 0,
        }
    }
}

impl SymmetryConfig {
    pub fn transform_points(&self, p: Vec2, width: f32, height: f32) -> Vec<Vec2> {
        let mut pts = vec![p];

        match self.mode {
            SymmetryMode::None => {}
            SymmetryMode::Horizontal => {
                pts.push(Vec2::new(width - p.x, p.y));
            }
            SymmetryMode::Vertical => {
                pts.push(Vec2::new(p.x, height - p.y));
            }
            SymmetryMode::Quad => {
                pts.push(Vec2::new(width - p.x, p.y));
                pts.push(Vec2::new(p.x, height - p.y));
                pts.push(Vec2::new(width - p.x, height - p.y));
            }
        }

        if self.mandala_segments > 1 {
            let center = Vec2::new(width * 0.5, height * 0.5);
            let angle_step = std::f32::consts::TAU / self.mandala_segments as f32;
            let mut mandala_pts = Vec::with_capacity(pts.len() * self.mandala_segments as usize);

            for base in pts {
                let rel = base - center;
                for i in 0..self.mandala_segments {
                    let angle = angle_step * (i as f32);
                    let cos = angle.cos();
                    let sin = angle.sin();
                    let rotated = Vec2::new(
                        rel.x * cos - rel.y * sin,
                        rel.x * sin + rel.y * cos,
                    );
                    mandala_pts.push(center + rotated);
                }
            }
            mandala_pts
        } else {
            pts
        }
    }
}
