use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionMask {
    pub width: u32,
    pub height: u32,
    pub mask: Vec<u8>, // 0..=255 mask intensity
}

impl SelectionMask {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            mask: vec![0; size],
        }
    }

    pub fn from_rect(width: u32, height: u32, min: Vec2, max: Vec2) -> Self {
        let mut sm = Self::new(width, height);
        let x0 = (min.x.floor().max(0.0) as u32).min(width);
        let y0 = (min.y.floor().max(0.0) as u32).min(height);
        let x1 = (max.x.ceil().max(0.0) as u32).min(width);
        let y1 = (max.y.ceil().max(0.0) as u32).min(height);

        for y in y0..y1 {
            for x in x0..x1 {
                let idx = (y as usize) * (width as usize) + (x as usize);
                sm.mask[idx] = 255;
            }
        }
        sm
    }

    pub fn from_polygon(width: u32, height: u32, points: &[Vec2]) -> Self {
        let mut sm = Self::new(width, height);
        if points.len() < 3 {
            return sm;
        }

        // Ray casting point-in-polygon fill
        for y in 0..height {
            let py = y as f32 + 0.5;
            for x in 0..width {
                let px = x as f32 + 0.5;
                let mut inside = false;
                let mut j = points.len() - 1;
                for i in 0..points.len() {
                    let pi = points[i];
                    let pj = points[j];
                    if ((pi.y > py) != (pj.y > py))
                        && (px < (pj.x - pi.x) * (py - pi.y) / (pj.y - pi.y) + pi.x)
                    {
                        inside = !inside;
                    }
                    j = i;
                }
                if inside {
                    let idx = (y as usize) * (width as usize) + (x as usize);
                    sm.mask[idx] = 255;
                }
            }
        }
        sm
    }

    pub fn from_mask_vec(width: u32, height: u32, mask: Vec<u8>) -> Self {
        Self { width, height, mask }
    }

    pub fn has_selection(&self) -> bool {
        self.mask.iter().any(|&v| v > 8)
    }

    pub fn invert(&mut self) {
        for v in &mut self.mask {
            *v = 255 - *v;
        }
    }

    pub fn clear(&mut self) {
        self.mask.fill(0);
    }

    #[inline]
    pub fn get_value(&self, x: u32, y: u32) -> u8 {
        if x < self.width && y < self.height {
            self.mask[(y as usize) * (self.width as usize) + (x as usize)]
        } else {
            0
        }
    }

    pub fn feather(&mut self, radius: u32) {
        if radius == 0 || !self.has_selection() {
            return;
        }
        let r = radius.min(24) as i32;
        let w = self.width as i32;
        let h = self.height as i32;
        let mut tmp = self.mask.clone();
        let mut out = vec![0u8; self.mask.len()];

        // 2-pass separable box blur
        for _ in 0..2 {
            // Horizontal
            for y in 0..h {
                for x in 0..w {
                    let mut sum = 0u32;
                    let mut count = 0u32;
                    for dx in -r..=r {
                        let nx = x + dx;
                        if nx >= 0 && nx < w {
                            sum += tmp[(y * w + nx) as usize] as u32;
                            count += 1;
                        }
                    }
                    out[(y * w + x) as usize] = if count > 0 { (sum / count) as u8 } else { 0 };
                }
            }
            tmp.copy_from_slice(&out);

            // Vertical
            for y in 0..h {
                for x in 0..w {
                    let mut sum = 0u32;
                    let mut count = 0u32;
                    for dy in -r..=r {
                        let ny = y + dy;
                        if ny >= 0 && ny < h {
                            sum += tmp[(ny * w + x) as usize] as u32;
                            count += 1;
                        }
                    }
                    out[(y * w + x) as usize] = if count > 0 { (sum / count) as u8 } else { 0 };
                }
            }
            tmp.copy_from_slice(&out);
        }

        self.mask = tmp;
    }

    pub fn expand(&mut self, radius: u32) {
        if radius == 0 {
            return;
        }
        let r = radius as i32;
        let w = self.width as i32;
        let h = self.height as i32;
        let src = self.mask.clone();

        for y in 0..h {
            for x in 0..w {
                if src[(y * w + x) as usize] > 8 {
                    for dy in -r..=r {
                        for dx in -r..=r {
                            let nx = x + dx;
                            let ny = y + dy;
                            if nx >= 0 && nx < w && ny >= 0 && ny < h {
                                self.mask[(ny * w + nx) as usize] = 255;
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn contract(&mut self, radius: u32) {
        if radius == 0 {
            return;
        }
        let r = radius as i32;
        let w = self.width as i32;
        let h = self.height as i32;
        let src = self.mask.clone();
        self.mask.fill(0);

        for y in 0..h {
            for x in 0..w {
                let mut all_set = true;
                'check: for dy in -r..=r {
                    for dx in -r..=r {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || nx >= w || ny < 0 || ny >= h || src[(ny * w + nx) as usize] <= 8 {
                            all_set = false;
                            break 'check;
                        }
                    }
                }
                if all_set {
                    self.mask[(y * w + x) as usize] = 255;
                }
            }
        }
    }
}
