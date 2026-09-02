use glam::Vec2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineTransform2D {
    pub translation: Vec2,
    pub pivot: Vec2,
    pub scale: Vec2,
    pub rotation_rad: f32,
    pub flip_h: bool,
    pub flip_v: bool,
}

impl Default for AffineTransform2D {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
            pivot: Vec2::ZERO,
            scale: Vec2::ONE,
            rotation_rad: 0.0,
            flip_h: false,
            flip_v: false,
        }
    }
}

impl AffineTransform2D {
    pub fn new(pivot: Vec2) -> Self {
        Self {
            translation: Vec2::ZERO,
            pivot,
            scale: Vec2::ONE,
            rotation_rad: 0.0,
            flip_h: false,
            flip_v: false,
        }
    }

    /// Maps a point from source local patch coordinates to transformed canvas coordinates
    pub fn forward(&self, pt: Vec2) -> Vec2 {
        // 1. Shift relative to pivot
        let mut p = pt - self.pivot;

        // 2. Flip
        if self.flip_h {
            p.x = -p.x;
        }
        if self.flip_v {
            p.y = -p.y;
        }

        // 3. Scale
        p *= self.scale;

        // 4. Rotate
        let cos_r = self.rotation_rad.cos();
        let sin_r = self.rotation_rad.sin();
        let rot_p = Vec2::new(
            p.x * cos_r - p.y * sin_r,
            p.x * sin_r + p.y * cos_r,
        );

        // 5. Restore pivot and apply translation
        rot_p + self.pivot + self.translation
    }

    /// Maps a canvas coordinate backward to source local patch coordinates
    pub fn inverse(&self, pt: Vec2) -> Vec2 {
        // 1. Shift back translation and pivot
        let p = pt - self.translation - self.pivot;

        // 2. Inverse Rotate
        let cos_r = (-self.rotation_rad).cos();
        let sin_r = (-self.rotation_rad).sin();
        let mut rot_p = Vec2::new(
            p.x * cos_r - p.y * sin_r,
            p.x * sin_r + p.y * cos_r,
        );

        // 3. Inverse Scale
        let sx = if self.scale.x.abs() > 1e-6 { self.scale.x } else if self.scale.x < 0.0 { -1e-6 } else { 1e-6 };
        let sy = if self.scale.y.abs() > 1e-6 { self.scale.y } else if self.scale.y < 0.0 { -1e-6 } else { 1e-6 };
        rot_p.x /= sx;
        rot_p.y /= sy;

        // 4. Inverse Flip
        if self.flip_h {
            rot_p.x = -rot_p.x;
        }
        if self.flip_v {
            rot_p.y = -rot_p.y;
        }

        // 5. Restore pivot
        rot_p + self.pivot
    }
}

/// Bilinear sub-pixel sampling helper
#[inline]
pub fn sample_bilinear(src: &[u8], w: u32, h: u32, x: f32, y: f32) -> (u8, u8, u8, u8) {
    if x < 0.0 || y < 0.0 || x >= w as f32 || y >= h as f32 {
        let ix = x.round() as isize;
        let iy = y.round() as isize;
        if ix >= 0 && ix < w as isize && iy >= 0 && iy < h as isize {
            let idx = (iy as usize * w as usize + ix as usize) * 4;
            if idx + 3 < src.len() {
                return (src[idx], src[idx + 1], src[idx + 2], src[idx + 3]);
            }
        }
        return (0, 0, 0, 0);
    }

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w as usize - 1);
    let y1 = (y0 + 1).min(h as usize - 1);

    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let inv_fx = 1.0 - fx;
    let inv_fy = 1.0 - fy;

    let w00 = inv_fx * inv_fy;
    let w10 = fx * inv_fy;
    let w01 = inv_fx * fy;
    let w11 = fx * fy;

    let get_px = |px: usize, py: usize| -> (f32, f32, f32, f32) {
        let idx = (py * w as usize + px) * 4;
        (
            src[idx] as f32,
            src[idx + 1] as f32,
            src[idx + 2] as f32,
            src[idx + 3] as f32,
        )
    };

    let p00 = get_px(x0, y0);
    let p10 = get_px(x1, y0);
    let p01 = get_px(x0, y1);
    let p11 = get_px(x1, y1);

    let r = (p00.0 * w00 + p10.0 * w10 + p01.0 * w01 + p11.0 * w11).round() as u8;
    let g = (p00.1 * w00 + p10.1 * w10 + p01.1 * w01 + p11.1 * w11).round() as u8;
    let b = (p00.2 * w00 + p10.2 * w10 + p01.2 * w01 + p11.2 * w11).round() as u8;
    let a = (p00.3 * w00 + p10.3 * w10 + p01.3 * w01 + p11.3 * w11).round() as u8;

    (r, g, b, a)
}

/// Nearest-neighbor sampling helper for crisp pixel art
#[inline]
pub fn sample_nearest(src: &[u8], w: u32, h: u32, x: f32, y: f32) -> (u8, u8, u8, u8) {
    let ix = x.floor() as isize;
    let iy = y.floor() as isize;
    if ix >= 0 && ix < w as isize && iy >= 0 && iy < h as isize {
        let idx = (iy as usize * w as usize + ix as usize) * 4;
        if idx + 3 < src.len() {
            return (src[idx], src[idx + 1], src[idx + 2], src[idx + 3]);
        }
    }
    (0, 0, 0, 0)
}

/// Transforms a patch and composites it onto a target layer canvas
pub fn render_transformed_patch(
    src_patch: &[u8],
    patch_w: u32,
    patch_h: u32,
    patch_origin: Vec2, // Where the patch was extracted from in canvas space
    transform: &AffineTransform2D,
    is_bilinear: bool,
    dst_layer_pixels: &mut [u8],
    doc_w: u32,
    doc_h: u32,
) {
    if patch_w == 0 || patch_h == 0 || src_patch.is_empty() {
        return;
    }

    // Compute destination bounding box of the 4 transformed corners
    let corners = [
        patch_origin,
        patch_origin + Vec2::new(patch_w as f32, 0.0),
        patch_origin + Vec2::new(patch_w as f32, patch_h as f32),
        patch_origin + Vec2::new(0.0, patch_h as f32),
    ];

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for &c in &corners {
        let tc = transform.forward(c);
        min_x = min_x.min(tc.x);
        max_x = max_x.max(tc.x);
        min_y = min_y.min(tc.y);
        max_y = max_y.max(tc.y);
    }

    let start_x = (min_x.floor() as isize).max(0).min(doc_w as isize) as usize;
    let end_x = (max_x.ceil() as isize + 1).max(0).min(doc_w as isize) as usize;
    let start_y = (min_y.floor() as isize).max(0).min(doc_h as isize) as usize;
    let end_y = (max_y.ceil() as isize + 1).max(0).min(doc_h as isize) as usize;

    for y in start_y..end_y {
        for x in start_x..end_x {
            let canvas_pt = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            // Backward-map from canvas to source patch
            let src_canvas_pt = transform.inverse(canvas_pt);
            let local_x = src_canvas_pt.x - patch_origin.x;
            let local_y = src_canvas_pt.y - patch_origin.y;

            let (r, g, b, a) = if is_bilinear {
                sample_bilinear(src_patch, patch_w, patch_h, local_x, local_y)
            } else {
                sample_nearest(src_patch, patch_w, patch_h, local_x, local_y)
            };

            if a > 0 {
                let dst_idx = (y * doc_w as usize + x) * 4;
                if dst_idx + 3 < dst_layer_pixels.len() {
                    let cur_a = dst_layer_pixels[dst_idx + 3] as u32;
                    let new_a = a as u32;
                    if new_a == 255 || cur_a == 0 {
                        dst_layer_pixels[dst_idx] = r;
                        dst_layer_pixels[dst_idx + 1] = g;
                        dst_layer_pixels[dst_idx + 2] = b;
                        dst_layer_pixels[dst_idx + 3] = a;
                    } else {
                        // Alpha composite
                        let out_a = new_a + cur_a * (255 - new_a) / 255;
                        if out_a > 0 {
                            let out_r = ((r as u32 * new_a + dst_layer_pixels[dst_idx] as u32 * cur_a * (255 - new_a) / 255) / out_a) as u8;
                            let out_g = ((g as u32 * new_a + dst_layer_pixels[dst_idx + 1] as u32 * cur_a * (255 - new_a) / 255) / out_a) as u8;
                            let out_b = ((b as u32 * new_a + dst_layer_pixels[dst_idx + 2] as u32 * cur_a * (255 - new_a) / 255) / out_a) as u8;
                            dst_layer_pixels[dst_idx] = out_r;
                            dst_layer_pixels[dst_idx + 1] = out_g;
                            dst_layer_pixels[dst_idx + 2] = out_b;
                            dst_layer_pixels[dst_idx + 3] = out_a.min(255) as u8;
                        }
                    }
                }
            }
        }
    }
}
