use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokePosition {
    Center,
    Inside,
    Outside,
}

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

    pub fn select_all(width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            mask: vec![255; size],
        }
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

    pub fn union(&mut self, other: &SelectionMask) {
        if self.width != other.width || self.height != other.height {
            return;
        }
        for (a, &b) in self.mask.iter_mut().zip(other.mask.iter()) {
            *a = (*a).max(b);
        }
    }

    pub fn subtract(&mut self, other: &SelectionMask) {
        if self.width != other.width || self.height != other.height {
            return;
        }
        for (a, &b) in self.mask.iter_mut().zip(other.mask.iter()) {
            *a = a.saturating_sub(b);
        }
    }

    #[inline]
    pub fn get_value(&self, x: u32, y: u32) -> u8 {
        if x < self.width && y < self.height {
            self.mask[(y as usize) * (self.width as usize) + (x as usize)]
        } else {
            0
        }
    }

    #[inline]
    pub fn is_selected(&self, x: u32, y: u32) -> bool {
        self.get_value(x, y) > 8
    }

    pub fn feather(&mut self, radius: u32) {
        if radius == 0 || !self.has_selection() {
            return;
        }
        let r = radius.min(50) as i32;
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
        let r = radius.min(50) as i32;
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
        let r = radius.min(50) as i32;
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

    /// Fills the selected region on a pixel buffer with the given RGBA color
    pub fn fill_selection(&self, pixels: &mut [u8], width: u32, height: u32, fill_color: [u8; 4]) {
        let max_w = width.min(self.width);
        let max_h = height.min(self.height);

        let src_r = fill_color[0] as f32;
        let src_g = fill_color[1] as f32;
        let src_b = fill_color[2] as f32;
        let src_a_base = fill_color[3] as f32 / 255.0;

        for y in 0..max_h {
            for x in 0..max_w {
                let mask_val = self.get_value(x, y);
                if mask_val > 0 {
                    let weight = (mask_val as f32 / 255.0) * src_a_base;
                    let idx = (y as usize * width as usize + x as usize) * 4;
                    if idx + 3 < pixels.len() {
                        let dst_r = pixels[idx] as f32;
                        let dst_g = pixels[idx + 1] as f32;
                        let dst_b = pixels[idx + 2] as f32;
                        let dst_a = pixels[idx + 3] as f32 / 255.0;

                        let out_a = weight + dst_a * (1.0 - weight);
                        if out_a > 0.0 {
                            let out_r = (src_r * weight + dst_r * dst_a * (1.0 - weight)) / out_a;
                            let out_g = (src_g * weight + dst_g * dst_a * (1.0 - weight)) / out_a;
                            let out_b = (src_b * weight + dst_b * dst_a * (1.0 - weight)) / out_a;

                            pixels[idx] = out_r.round().clamp(0.0, 255.0) as u8;
                            pixels[idx + 1] = out_g.round().clamp(0.0, 255.0) as u8;
                            pixels[idx + 2] = out_b.round().clamp(0.0, 255.0) as u8;
                            pixels[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
    }

    /// Strokes the selection border onto a pixel buffer with configurable width and position
    pub fn stroke_selection(
        &self,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        stroke_color: [u8; 4],
        stroke_width: u32,
        position: StrokePosition,
    ) {
        if stroke_width == 0 || !self.has_selection() {
            return;
        }

        let w = self.width as i32;
        let h = self.height as i32;
        let r = (stroke_width as f32 * 0.5).max(0.5);
        let r_ceil = r.ceil() as i32 + 1;

        // 1. Find border pixels of the selection
        let mut border_pts = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if self.is_selected(x as u32, y as u32) {
                    let is_border = x == 0
                        || x == w - 1
                        || y == 0
                        || y == h - 1
                        || !self.is_selected((x - 1) as u32, y as u32)
                        || !self.is_selected((x + 1) as u32, y as u32)
                        || !self.is_selected(x as u32, (y - 1) as u32)
                        || !self.is_selected(x as u32, (y + 1) as u32);
                    if is_border {
                        border_pts.push((x, y));
                    }
                }
            }
        }

        if border_pts.is_empty() {
            return;
        }

        // 2. Generate stroke coverage mask
        let mut stroke_mask = vec![0.0f32; (w * h) as usize];
        for (bx, by) in border_pts {
            let bxf = bx as f32 + 0.5;
            let byf = by as f32 + 0.5;

            for dy in -r_ceil..=r_ceil {
                for dx in -r_ceil..=r_ceil {
                    let nx = bx + dx;
                    let ny = by + dy;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        let is_inside = self.is_selected(nx as u32, ny as u32);
                        let is_valid_pos = match position {
                            StrokePosition::Center => true,
                            StrokePosition::Inside => is_inside,
                            StrokePosition::Outside => !is_inside,
                        };

                        if is_valid_pos {
                            let pxf = nx as f32 + 0.5;
                            let pyf = ny as f32 + 0.5;
                            let dist = ((pxf - bxf).powi(2) + (pyf - byf).powi(2)).sqrt();
                            if dist <= r + 0.5 {
                                let cov = (1.0 - (dist - (r - 0.5)).clamp(0.0, 1.0)).clamp(0.0, 1.0);
                                let idx = (ny * w + nx) as usize;
                                stroke_mask[idx] = stroke_mask[idx].max(cov);
                            }
                        }
                    }
                }
            }
        }

        // 3. Composite stroke onto target pixels
        let src_r = stroke_color[0] as f32;
        let src_g = stroke_color[1] as f32;
        let src_b = stroke_color[2] as f32;
        let src_a_base = stroke_color[3] as f32 / 255.0;

        let max_w = width.min(self.width) as i32;
        let max_h = height.min(self.height) as i32;

        for y in 0..max_h {
            for x in 0..max_w {
                let mask_cov = stroke_mask[(y * w + x) as usize];
                if mask_cov > 0.0 {
                    let weight = mask_cov * src_a_base;
                    let idx = (y as usize * width as usize + x as usize) * 4;
                    if idx + 3 < pixels.len() {
                        let dst_r = pixels[idx] as f32;
                        let dst_g = pixels[idx + 1] as f32;
                        let dst_b = pixels[idx + 2] as f32;
                        let dst_a = pixels[idx + 3] as f32 / 255.0;

                        let out_a = weight + dst_a * (1.0 - weight);
                        if out_a > 0.0 {
                            let out_r = (src_r * weight + dst_r * dst_a * (1.0 - weight)) / out_a;
                            let out_g = (src_g * weight + dst_g * dst_a * (1.0 - weight)) / out_a;
                            let out_b = (src_b * weight + dst_b * dst_a * (1.0 - weight)) / out_a;

                            pixels[idx] = out_r.round().clamp(0.0, 255.0) as u8;
                            pixels[idx + 1] = out_g.round().clamp(0.0, 255.0) as u8;
                            pixels[idx + 2] = out_b.round().clamp(0.0, 255.0) as u8;
                            pixels[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
    }
}
