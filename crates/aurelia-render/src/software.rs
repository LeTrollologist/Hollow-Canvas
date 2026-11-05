use egui::epaint::Primitive;
use egui::{ClippedPrimitive, ImageData, Rect, TextureId, TexturesDelta};
use glam::Vec2;
use aurelia_core::document::Document;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct TracingReferenceConfig<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub opacity: f32,
    pub offset: Vec2,
    pub scale: f32,
    pub is_underlay: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct OnionSkinFrame<'a> {
    pub rgba: &'a [u8],
    pub tint_r: u8,
    pub tint_g: u8,
    pub tint_b: u8,
    pub opacity: f32,
    pub is_prev: bool,
}

pub struct SoftwareRenderer {
    textures: HashMap<TextureId, (u32, u32, Vec<u8>)>,
    composite_buffer: Vec<u8>,
}

impl SoftwareRenderer {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            composite_buffer: Vec::new(),
        }
    }

    pub fn update_textures(&mut self, textures_delta: &TexturesDelta) {
        for (id, delta) in &textures_delta.set {
            let (w, h, rgba) = match &delta.image {
                ImageData::Color(img) => {
                    let mut bytes = Vec::with_capacity(img.pixels.len() * 4);
                    for p in &img.pixels {
                        let [r, g, b, a] = p.to_array();
                        bytes.extend_from_slice(&[r, g, b, a]);
                    }
                    (img.size[0] as u32, img.size[1] as u32, bytes)
                }
                ImageData::Font(img) => {
                    let mut bytes = Vec::with_capacity(img.pixels.len() * 4);
                    for &alpha in &img.pixels {
                        let a = (alpha * 255.0).clamp(0.0, 255.0) as u8;
                        bytes.extend_from_slice(&[255, 255, 255, a]);
                    }
                    (img.size[0] as u32, img.size[1] as u32, bytes)
                }
            };

            if let Some(pos) = delta.pos {
                if let Some((tex_w, _tex_h, target)) = self.textures.get_mut(id) {
                    let (x_offset, y_offset) = (pos[0], pos[1]);
                    for y in 0..h {
                        for x in 0..w {
                            let src_idx = ((y * w + x) * 4) as usize;
                            let dst_idx = (((y_offset as u32 + y) * *tex_w + (x_offset as u32 + x)) * 4) as usize;
                            if src_idx + 4 <= rgba.len() && dst_idx + 4 <= target.len() {
                                target[dst_idx..dst_idx + 4].copy_from_slice(&rgba[src_idx..src_idx + 4]);
                            }
                        }
                    }
                }
            } else {
                self.textures.insert(*id, (w, h, rgba));
            }
        }

        for id in &textures_delta.free {
            self.textures.remove(id);
        }
    }

    pub fn render_canvas(
        &mut self,
        buffer: &mut [u32],
        win_w: usize,
        win_h: usize,
        doc: &Document,
        pan: Vec2,
        zoom: f32,
        center_offset: Vec2,
        viewport_clip: [usize; 4],
        tracing: Option<TracingReferenceConfig>,
        onion_skins: &[OnionSkinFrame],
        selection_mask: Option<&[u8]>,
    ) {
        if win_w == 0 || win_h == 0 || buffer.len() < win_w * win_h {
            return;
        }

        let [vp_min_x, vp_min_y, vp_max_x, vp_max_y] = [
            viewport_clip[0].min(win_w),
            viewport_clip[1].min(win_h),
            viewport_clip[2].min(win_w),
            viewport_clip[3].min(win_h),
        ];

        let paper = doc.paper_rgb8();

        // Studio backdrop: a soft vertical gradient (lamplight above, deep ink below)
        // so the artboard reads as paper resting on a lit surface rather than floating
        // in a void. Panels and the canvas paint over this.
        // Kept clearly lighter than the darkest paper (value 13) so the artboard
        // always reads as a recessed well instead of vanishing into the backdrop.
        let top = (38.0_f32, 31.0, 24.0);
        let bottom = (26.0_f32, 21.0, 16.0);
        for y in 0..win_h {
            let t = y as f32 / (win_h.max(1) - 1) as f32;
            let r = (top.0 + (bottom.0 - top.0) * t).round() as u32;
            let g = (top.1 + (bottom.1 - top.1) * t).round() as u32;
            let b = (top.2 + (bottom.2 - top.2) * t).round() as u32;
            let color = 0xFF00_0000 | (r << 16) | (g << 8) | b;
            let row_start = y * win_w;
            let row_end = row_start + win_w;
            if row_end <= buffer.len() {
                buffer[row_start..row_end].fill(color);
            }
        }

        let doc_w = doc.width as f32;
        let doc_h = doc.height as f32;
        let center_x = (win_w as f32) * 0.5 + pan.x + center_offset.x;
        let center_y = (win_h as f32) * 0.5 + pan.y + center_offset.y;

        let canvas_w = doc_w * zoom;
        let canvas_h = doc_h * zoom;

        let x0 = (center_x - canvas_w * 0.5).floor() as isize;
        let y0 = (center_y - canvas_h * 0.5).floor() as isize;
        let x1 = (center_x + canvas_w * 0.5).ceil() as isize;
        let y1 = (center_y + canvas_h * 0.5).ceil() as isize;

        let min_x = (x0.max(vp_min_x as isize).min(vp_max_x as isize)) as usize;
        let max_x = (x1.max(vp_min_x as isize).min(vp_max_x as isize)) as usize;
        let min_y = (y0.max(vp_min_y as isize).min(vp_max_y as isize)) as usize;
        let max_y = (y1.max(vp_min_y as isize).min(vp_max_y as isize)) as usize;

        if min_x >= max_x || min_y >= max_y {
            return;
        }

        // 1. Soft Drop Shadow around canvas boundary (strictly clipped to viewport)
        let shadow_blur = 18isize;
        let shadow_offset_y = 6isize;
        let s_x0 = (x0 - shadow_blur).max(vp_min_x as isize).min(vp_max_x as isize) as usize;
        let s_x1 = (x1 + shadow_blur).max(vp_min_x as isize).min(vp_max_x as isize) as usize;
        let s_y0 = (y0 - shadow_blur + shadow_offset_y).max(vp_min_y as isize).min(vp_max_y as isize) as usize;
        let s_y1 = (y1 + shadow_blur + shadow_offset_y).max(vp_min_y as isize).min(vp_max_y as isize) as usize;

        let cx0 = x0 as f32;
        let cx1 = x1 as f32;
        let cy0 = y0 as f32;
        let cy1 = y1 as f32;

        for sy in s_y0..s_y1 {
            let row = sy * win_w;
            let sy_f = sy as f32;
            for sx in s_x0..s_x1 {
                let sx_f = sx as f32;
                let dx = if sx_f < cx0 { cx0 - sx_f } else if sx_f > cx1 { sx_f - cx1 } else { 0.0 };
                let dy = if sy_f < cy0 { cy0 - sy_f } else if sy_f > cy1 { sy_f - cy1 } else { 0.0 };
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 0.0 && dist <= shadow_blur as f32 {
                    let factor = (1.0 - dist / (shadow_blur as f32)).powi(2);
                    let shadow_alpha = (factor * 0.55).clamp(0.0, 1.0);
                    let p_idx = row + sx;
                    if p_idx < buffer.len() {
                        let cur_px = buffer[p_idx];
                        let r = (cur_px >> 16) & 0xFF;
                        let g = (cur_px >> 8) & 0xFF;
                        let b = cur_px & 0xFF;
                        let inv_a = 1.0 - shadow_alpha;
                        let out_r = ((r as f32 * inv_a) as u32).min(255);
                        let out_g = ((g as f32 * inv_a) as u32).min(255);
                        let out_b = ((b as f32 * inv_a) as u32).min(255);
                        buffer[p_idx] = 0xFF000000 | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            }
        }

        let req_size = (doc.width * doc.height * 4) as usize;
        if self.composite_buffer.len() != req_size {
            self.composite_buffer.resize(req_size, 0);
        }
        doc.composite_layers_into(&mut self.composite_buffer, false);
        let flat_pixels = &self.composite_buffer;
        let inv_zoom = 1.0 / zoom;
        let is_exact_1to1 = (zoom - 1.0).abs() < 0.001 && center_x.fract().abs() < 0.01 && center_y.fract().abs() < 0.01;
        let use_bilinear = !is_exact_1to1 && zoom < 2.5;

        for screen_y in min_y..max_y {
            let doc_yf = (screen_y as f32 - center_y) * inv_zoom + doc_h * 0.5;
            let doc_yi = doc_yf.floor() as isize;

            if doc_yi < 0 || doc_yi >= doc.height as isize {
                continue;
            }

            let row_offset = screen_y * win_w;
            let src_row_offset = (doc_yi as usize) * (doc.width as usize) * 4;

            for screen_x in min_x..max_x {
                let doc_xf = (screen_x as f32 - center_x) * inv_zoom + doc_w * 0.5;
                let doc_xi = doc_xf.floor() as isize;

                if doc_xi < 0 || doc_xi >= doc.width as isize {
                    continue;
                }

                let src_idx = src_row_offset + (doc_xi as usize) * 4;

                let (lr, lg, lb, la) = if use_bilinear {
                    let px = aurelia_core::rasterizer::sample_bilinear_rgba(flat_pixels, doc.width, doc.height, doc_xf - 0.5, doc_yf - 0.5);
                    (px[0], px[1], px[2], px[3])
                } else {
                    if src_idx + 3 >= flat_pixels.len() {
                        continue;
                    }
                    (flat_pixels[src_idx], flat_pixels[src_idx + 1], flat_pixels[src_idx + 2], flat_pixels[src_idx + 3])
                };

                let pixel_idx = row_offset + screen_x;
                if pixel_idx >= buffer.len() {
                    continue;
                }

                // Base paper / background color
                let (mut base_r, mut base_g, mut base_b) = if doc.is_transparent {
                    let [r, g, b] =
                        doc.transparency_checker_rgb8(doc_xi as u32, doc_yi as u32);
                    (r, g, b)
                } else {
                    (paper[0], paper[1], paper[2])
                };

                // Tracing reference pixel sampling
                let ref_px = tracing.and_then(|cfg| {
                    if cfg.scale <= 0.001 || cfg.opacity <= 0.001 {
                        return None;
                    }
                    let rx_f = (doc_xf - cfg.offset.x) / cfg.scale;
                    let ry_f = (doc_yf - cfg.offset.y) / cfg.scale;
                    let rx = rx_f.floor() as isize;
                    let ry = ry_f.floor() as isize;
                    if rx >= 0 && rx < cfg.width as isize && ry >= 0 && ry < cfg.height as isize {
                        let idx = (ry as usize * cfg.width as usize + rx as usize) * 4;
                        if idx + 3 < cfg.rgba.len() {
                            let r = cfg.rgba[idx];
                            let g = cfg.rgba[idx + 1];
                            let b = cfg.rgba[idx + 2];
                            let a = ((cfg.rgba[idx + 3] as f32) * cfg.opacity).clamp(0.0, 255.0) as u8;
                            return Some((r, g, b, a));
                        }
                    }
                    None
                });

                // 1. If Tracing as Underlay: blend reference onto base first
                if let Some((rr, rg, rb, ra)) = ref_px {
                    if tracing.map_or(false, |t| t.is_underlay) && ra > 0 {
                        let a_u = ra as u32;
                        let inv_a = 255 - a_u;
                        base_r = ((base_r as u32 * inv_a + rr as u32 * a_u) / 255) as u8;
                        base_g = ((base_g as u32 * inv_a + rg as u32 * a_u) / 255) as u8;
                        base_b = ((base_b as u32 * inv_a + rb as u32 * a_u) / 255) as u8;
                    }
                }

                // 2. Blend Previous Onion Skin Frames (under active frame)
                for skin in onion_skins.iter().filter(|s| s.is_prev) {
                    if src_idx + 3 < skin.rgba.len() {
                        let sa = skin.rgba[src_idx + 3];
                        if sa > 0 {
                            let eff_a = ((sa as f32 * skin.opacity).clamp(0.0, 255.0)) as u32;
                            let inv_a = 255 - eff_a;
                            base_r = ((base_r as u32 * inv_a + skin.tint_r as u32 * eff_a) / 255) as u8;
                            base_g = ((base_g as u32 * inv_a + skin.tint_g as u32 * eff_a) / 255) as u8;
                            base_b = ((base_b as u32 * inv_a + skin.tint_b as u32 * eff_a) / 255) as u8;
                        }
                    }
                }

                // 3. Blend Active Document Layers on top
                let (comp_r, comp_g, comp_b) = if la == 255 {
                    (lr, lg, lb)
                } else if la == 0 {
                    (base_r, base_g, base_b)
                } else {
                    let la_u = la as u32;
                    let inv_la = 255 - la_u;
                    let r = ((base_r as u32 * inv_la + lr as u32 * la_u) / 255) as u8;
                    let g = ((base_g as u32 * inv_la + lg as u32 * la_u) / 255) as u8;
                    let b = ((base_b as u32 * inv_la + lb as u32 * la_u) / 255) as u8;
                    (r, g, b)
                };

                // 4. Blend Next Onion Skin Frames (over active frame with green tint)
                let (mut curr_r, mut curr_g, mut curr_b) = (comp_r, comp_g, comp_b);
                for skin in onion_skins.iter().filter(|s| !s.is_prev) {
                    if src_idx + 3 < skin.rgba.len() {
                        let sa = skin.rgba[src_idx + 3];
                        if sa > 0 {
                            let eff_a = ((sa as f32 * skin.opacity).clamp(0.0, 255.0)) as u32;
                            let inv_a = 255 - eff_a;
                            curr_r = ((curr_r as u32 * inv_a + skin.tint_r as u32 * eff_a) / 255) as u8;
                            curr_g = ((curr_g as u32 * inv_a + skin.tint_g as u32 * eff_a) / 255) as u8;
                            curr_b = ((curr_b as u32 * inv_a + skin.tint_b as u32 * eff_a) / 255) as u8;
                        }
                    }
                }

                // 5. If Tracing as Ghost Overlay: blend reference on top
                let (final_r, final_g, final_b) = if let Some((rr, rg, rb, ra)) = ref_px {
                    if tracing.map_or(false, |t| !t.is_underlay) && ra > 0 {
                        let a_u = ra as u32;
                        let inv_a = 255 - a_u;
                        let r = ((curr_r as u32 * inv_a + rr as u32 * a_u) / 255) as u8;
                        let g = ((curr_g as u32 * inv_a + rg as u32 * a_u) / 255) as u8;
                        let b = ((curr_b as u32 * inv_a + rb as u32 * a_u) / 255) as u8;
                        (r, g, b)
                    } else {
                        (curr_r, curr_g, curr_b)
                    }
                } else {
                    (curr_r, curr_g, curr_b)
                };

                // 6. Quick Mask / Selection Tint Overlay (translucent ruby red tint over selected pixels)
                let (out_r, out_g, out_b) = if let Some(mask) = selection_mask {
                    let m_idx = (doc_yi as usize) * (doc.width as usize) + (doc_xi as usize);
                    if m_idx < mask.len() && mask[m_idx] > 0 {
                        let m_val = mask[m_idx] as u32;
                        let tint_a = ((m_val * 75) / 255) as u32; // ~30% max opacity ruby tint
                        let inv_ta = 255 - tint_a;
                        let r = ((final_r as u32 * inv_ta + 255 * tint_a) / 255) as u8;
                        let g = ((final_g as u32 * inv_ta + 55 * tint_a) / 255) as u8;
                        let b = ((final_b as u32 * inv_ta + 95 * tint_a) / 255) as u8;
                        (r, g, b)
                    } else {
                        (final_r, final_g, final_b)
                    }
                } else {
                    (final_r, final_g, final_b)
                };

                buffer[pixel_idx] = 0xFF000000 | ((out_r as u32) << 16) | ((out_g as u32) << 8) | (out_b as u32);
            }
        }

        // 7. Crisp Canvas Border Frame Outline
        let border_color = 0xFF8A7458;
        let top = min_y;
        let btm = if max_y > 0 { max_y - 1 } else { 0 };
        let left = min_x;
        let right = if max_x > 0 { max_x - 1 } else { 0 };

        for x in left..=right {
            if top < win_h && x < win_w {
                buffer[top * win_w + x] = border_color;
            }
            if btm < win_h && x < win_w {
                buffer[btm * win_w + x] = border_color;
            }
        }
        for y in top..=btm {
            if y < win_h && left < win_w {
                buffer[y * win_w + left] = border_color;
            }
            if y < win_h && right < win_w {
                buffer[y * win_w + right] = border_color;
            }
        }
    }

    /// Downscaled copy of the last composite for the Navigator thumbnail.
    ///
    /// Reuses the composite `render_canvas` already produced this frame, so it costs one
    /// small sampling pass rather than a second full-document composite. Call it after
    /// `render_canvas`. Returns `None` until a composite for the current document size exists.
    pub fn navigator_thumbnail(&self, doc: &Document, max_w: usize) -> Option<egui::ColorImage> {
        let (dw, dh) = (doc.width as usize, doc.height as usize);
        if dw == 0 || dh == 0 || self.composite_buffer.len() != dw * dh * 4 {
            return None;
        }
        let tw = max_w.clamp(8, dw.max(8)).min(dw);
        let th = ((tw * dh) / dw).clamp(1, dh);
        let mut rgba = vec![0u8; tw * th * 4];
        const N: usize = 3; // N x N samples per thumbnail pixel
        for ty in 0..th {
            for tx in 0..tw {
                let (mut sr, mut sg, mut sb) = (0u32, 0u32, 0u32);
                for iy in 0..N {
                    let sy = ((ty * N + iy) * dh / (th * N)).min(dh - 1);
                    for ix in 0..N {
                        let sx = ((tx * N + ix) * dw / (tw * N)).min(dw - 1);
                        let [br, bg, bb] = if doc.is_transparent {
                            doc.transparency_checker_rgb8(sx as u32, sy as u32)
                        } else {
                            doc.paper_rgb8()
                        };
                        let i = (sy * dw + sx) * 4;
                        let a = self.composite_buffer[i + 3] as u32;
                        let inv = 255 - a;
                        sr += (br as u32 * inv + self.composite_buffer[i] as u32 * a) / 255;
                        sg += (bg as u32 * inv + self.composite_buffer[i + 1] as u32 * a) / 255;
                        sb += (bb as u32 * inv + self.composite_buffer[i + 2] as u32 * a) / 255;
                    }
                }
                let n = (N * N) as u32;
                let o = (ty * tw + tx) * 4;
                rgba[o] = (sr / n) as u8;
                rgba[o + 1] = (sg / n) as u8;
                rgba[o + 2] = (sb / n) as u8;
                rgba[o + 3] = 255;
            }
        }
        Some(egui::ColorImage::from_rgba_unmultiplied([tw, th], &rgba))
    }

    pub fn render_grid(
        &self,
        buffer: &mut [u32],
        win_w: usize,
        win_h: usize,
        doc: &Document,
        pan: Vec2,
        zoom: f32,
        center_offset: Vec2,
        viewport_clip: [usize; 4],
        grid_size: u32,
        grid_opacity: f32,
        grid_origin: Vec2,
    ) {
        if win_w == 0 || win_h == 0 || grid_size == 0 || grid_opacity <= 0.001 {
            return;
        }

        let [vp_min_x, vp_min_y, vp_max_x, vp_max_y] = [
            viewport_clip[0].min(win_w),
            viewport_clip[1].min(win_h),
            viewport_clip[2].min(win_w),
            viewport_clip[3].min(win_h),
        ];

        let doc_w = doc.width as f32;
        let doc_h = doc.height as f32;
        let center_x = (win_w as f32) * 0.5 + pan.x + center_offset.x;
        let center_y = (win_h as f32) * 0.5 + pan.y + center_offset.y;

        // Screen bounds of the canvas strictly clipped to viewport
        let x_start = (((0.0 - doc_w * 0.5) * zoom + center_x).round() as isize).max(vp_min_x as isize).min(vp_max_x as isize) as usize;
        let x_end = (((doc_w - doc_w * 0.5) * zoom + center_x).round() as isize).max(vp_min_x as isize).min(vp_max_x as isize) as usize;
        let y_start = (((0.0 - doc_h * 0.5) * zoom + center_y).round() as isize).max(vp_min_y as isize).min(vp_max_y as isize) as usize;
        let y_end = (((doc_h - doc_h * 0.5) * zoom + center_y).round() as isize).max(vp_min_y as isize).min(vp_max_y as isize) as usize;

        if x_start >= x_end || y_start >= y_end {
            return;
        }

        let alpha = grid_opacity.clamp(0.0, 1.0);
        let alpha_u = (alpha * 256.0) as u32;
        let inv_alpha_u = 256 - alpha_u;
        let gr = 168u32;
        let gg = 159u32;
        let gb = 216u32;

        let size = grid_size as f32;
        // Lines are at origin + k*size. Starting one step before zero lets a shifted
        // origin still draw the line just outside the canvas edge without a separate
        // modulo branch in the pixel loop.
        let mut gx = grid_origin.x.rem_euclid(size) - size;
        while gx <= doc_w {
            let sx = ((gx - doc_w * 0.5) * zoom + center_x).round() as isize;
            if gx >= 0.0 && sx >= x_start as isize && sx < x_end as isize {
                let x = sx as usize;
                for y in y_start..y_end {
                    let pixel_idx = y * win_w + x;
                    if pixel_idx < buffer.len() {
                        let cur = buffer[pixel_idx];
                        let cr = (cur >> 16) & 0xFF;
                        let cg = (cur >> 8) & 0xFF;
                        let cb = cur & 0xFF;

                        let nr = (cr * inv_alpha_u + gr * alpha_u) >> 8;
                        let ng = (cg * inv_alpha_u + gg * alpha_u) >> 8;
                        let nb = (cb * inv_alpha_u + gb * alpha_u) >> 8;

                        buffer[pixel_idx] = 0xFF000000 | (nr << 16) | (ng << 8) | nb;
                    }
                }
            }
            gx += size;
        }

        // 2. Draw Horizontal Grid Lines
        let mut gy = grid_origin.y.rem_euclid(size) - size;
        while gy <= doc_h {
            let sy = ((gy - doc_h * 0.5) * zoom + center_y).round() as isize;
            if gy >= 0.0 && sy >= y_start as isize && sy < y_end as isize {
                let y = sy as usize;
                let row = y * win_w;
                for x in x_start..x_end {
                    let pixel_idx = row + x;
                    if pixel_idx < buffer.len() {
                        let cur = buffer[pixel_idx];
                        let cr = (cur >> 16) & 0xFF;
                        let cg = (cur >> 8) & 0xFF;
                        let cb = cur & 0xFF;

                        let nr = (cr * inv_alpha_u + gr * alpha_u) >> 8;
                        let ng = (cg * inv_alpha_u + gg * alpha_u) >> 8;
                        let nb = (cb * inv_alpha_u + gb * alpha_u) >> 8;

                        buffer[pixel_idx] = 0xFF000000 | (nr << 16) | (ng << 8) | nb;
                    }
                }
            }
            gy += size;
        }
    }

    pub fn render_perspective_guides(
        &self,
        buffer: &mut [u32],
        win_w: usize,
        win_h: usize,
        doc: &Document,
        pan: Vec2,
        zoom: f32,
        center_offset: Vec2,
        viewport_clip: [usize; 4],
        perspective: &aurelia_core::perspective::PerspectiveConfig,
    ) {
        if win_w == 0 || win_h == 0 || !perspective.show_guides || perspective.p_type == aurelia_core::perspective::PerspectiveType::None || perspective.guide_opacity <= 0.001 {
            return;
        }

        let doc_w = doc.width as f32;
        let doc_h = doc.height as f32;
        let center_x = (win_w as f32) * 0.5 + pan.x + center_offset.x;
        let center_y = (win_h as f32) * 0.5 + pan.y + center_offset.y;

        let canvas_to_screen = |pt: Vec2| -> Vec2 {
            let sx = (pt.x - doc_w * 0.5) * zoom + center_x;
            let sy = (pt.y - doc_h * 0.5) * zoom + center_y;
            Vec2::new(sx, sy)
        };

        let guide_color_u32 = 0xFF000000
            | ((perspective.guide_color[0] as u32) << 16)
            | ((perspective.guide_color[1] as u32) << 8)
            | (perspective.guide_color[2] as u32);
        let horizon_color_u32 = 0xFF000000
            | ((perspective.horizon_color[0] as u32) << 16)
            | ((perspective.horizon_color[1] as u32) << 8)
            | (perspective.horizon_color[2] as u32);

        // 1. Draw Horizon Line
        let h_rad = perspective.horizon_angle.to_radians();
        let h_dir = Vec2::new(h_rad.cos(), h_rad.sin());
        let h_center_canvas = Vec2::new(doc_w * 0.5, perspective.horizon_y);
        let span = (doc_w * doc_w + doc_h * doc_h).sqrt() * 3.0;
        let h_p0_canvas = h_center_canvas - h_dir * span;
        let h_p1_canvas = h_center_canvas + h_dir * span;
        let h_p0_screen = canvas_to_screen(h_p0_canvas);
        let h_p1_screen = canvas_to_screen(h_p1_canvas);

        self.draw_screen_line(buffer, win_w, win_h, h_p0_screen, h_p1_screen, horizon_color_u32, false, viewport_clip);

        // 2. Draw Radiating Guide Rays from Active Vanishing Points
        let active_vps = perspective.get_active_vps();
        for (vp_idx, &vp_canvas) in active_vps.iter().enumerate() {
            let rays = perspective.generate_rays_for_vp(vp_canvas, doc_w, doc_h, perspective.guide_density);
            let vp_color = match vp_idx {
                0 => 0xFFF6C45A, // Left VP: Aurelia Gold
                1 => 0xFF3FA796, // Right VP: Aurelia Teal
                2 => 0xFFE4705F, // Vertical VP: Aurelia Coral
                _ => guide_color_u32,
            };

            for (start_canvas, end_canvas) in rays {
                let start_screen = canvas_to_screen(start_canvas);
                let end_screen = canvas_to_screen(end_canvas);
                self.draw_screen_line(buffer, win_w, win_h, start_screen, end_screen, vp_color, true, viewport_clip);
            }

            // Draw Vanishing Point Gizmo Handle
            let vp_screen = canvas_to_screen(vp_canvas);
            self.draw_screen_crosshair(buffer, win_w, win_h, vp_screen, 0xFFFFFFFF, viewport_clip);
            self.draw_screen_ellipse(
                buffer,
                win_w,
                win_h,
                vp_screen - Vec2::new(7.0, 7.0),
                vp_screen + Vec2::new(7.0, 7.0),
                vp_color,
                false,
                viewport_clip,
            );
        }
    }

    pub fn render_rulers(
        &self,
        buffer: &mut [u32],
        win_w: usize,
        win_h: usize,
        doc: &Document,
        pan: Vec2,
        zoom: f32,
        center_offset: Vec2,
        viewport_clip: [usize; 4],
        cursor_pos: Vec2,
    ) {
        if win_w < 250 || win_h < 150 {
            return;
        }

        let [vp_min_x, vp_min_y, vp_max_x, vp_max_y] = [
            viewport_clip[0].min(win_w),
            viewport_clip[1].min(win_h),
            viewport_clip[2].min(win_w),
            viewport_clip[3].min(win_h),
        ];
        if vp_max_x <= vp_min_x + 4 || vp_max_y <= vp_min_y + 4 {
            return;
        }

        // 24 px = 1 px pad + 14 px label + 1 px gap + 6 px major tick + 1 px border.
        // Keep in sync with `aurelia_ui::design::RULER_PX`.
        let top_ruler_h = 24usize;
        let left_ruler_w = 24usize;
        let top_y_start = vp_min_y;
        let left_x_start = vp_min_x;
        let right_x_end = vp_max_x;

        let ruler_bg = 0xFF14110D;
        // Beyond the canvas the strip is recessed instead of merely dimmed.
        let ruler_bg_off = 0xFF0D0A07;
        // 1 px bevel along the outer edge of each strip.
        let ruler_highlight = 0xFF1E1913;
        let ruler_border = 0xFF3A2E24;
        let tick_minor = 0xFF5E4E3D;
        let tick_mid = 0xFF8A7458;
        let tick_major = 0xFFC0A886;
        let tick_off = 0xFF352C24;
        let ruler_marker = 0xFFF6C45A;
        let ruler_accent = 0xFFF6C45A;

        let doc_w = doc.width as f32;
        let doc_h = doc.height as f32;
        let zoom = zoom.max(0.02);
        let center_x = (win_w as f32) * 0.5 + pan.x + center_offset.x;
        let center_y = (win_h as f32) * 0.5 + pan.y + center_offset.y;
        let bottom_y_end = vp_max_y;

        // Geometry comes from the pure helpers so placement can be unit-tested
        // without a framebuffer. Numbers are painted by egui on top of the frame.
        let top_ticks = ruler_ticks(doc_w, zoom, center_x, left_x_start as isize, right_x_end as isize);
        let left_ticks = ruler_ticks(
            doc_h,
            zoom,
            center_y,
            (top_y_start + top_ruler_h) as isize,
            bottom_y_end as isize,
        );

        // Canvas edges in screen space, used to recess the off-canvas part.
        let doc_x0 = doc_to_screen(0.0, doc_w, zoom, center_x);
        let doc_x1 = doc_to_screen(doc_w, doc_w, zoom, center_x);
        let doc_y0 = doc_to_screen(0.0, doc_h, zoom, center_y);
        let doc_y1 = doc_to_screen(doc_h, doc_h, zoom, center_y);

        // ── Top strip ───────────────────────────────────────────────────────
        for ry in 0..top_ruler_h {
            let y = top_y_start + ry;
            if y >= win_h {
                continue;
            }
            let row = y * win_w;
            for x in left_x_start..right_x_end {
                let centre = x as f32 + 0.5;
                let on_canvas = centre >= doc_x0 && centre < doc_x1;
                let color = if ry == top_ruler_h - 1 {
                    ruler_border
                } else if ry == 0 {
                    ruler_highlight
                } else if on_canvas {
                    ruler_bg
                } else {
                    ruler_bg_off
                };
                let idx = row + x;
                if idx < buffer.len() {
                    buffer[idx] = color;
                }
            }
        }

        for tick in &top_ticks {
            let len = tick.level.length();
            let color = if tick.value > doc_w {
                tick_off
            } else {
                match tick.level {
                    TickLevel::Major => tick_major,
                    TickLevel::Mid => tick_mid,
                    TickLevel::Minor => tick_minor,
                }
            };
            for ty in (top_ruler_h - 1 - len)..(top_ruler_h - 1) {
                let idx = (top_y_start + ty) * win_w + tick.screen as usize;
                if idx < buffer.len() {
                    buffer[idx] = color;
                }
            }
        }


        // Canvas bounds are emphasised: 0 and the far edge.
        for bound in [0.0_f32, doc_w] {
            let sx = doc_to_screen(bound, doc_w, zoom, center_x).round() as isize;
            if sx >= left_x_start as isize && sx < right_x_end as isize {
                for ty in 1..(top_ruler_h - 1) {
                    let idx = (top_y_start + ty) * win_w + sx as usize;
                    if idx < buffer.len() {
                        buffer[idx] = ruler_accent;
                    }
                }
            }
        }

        // Cursor marker: 1 px accent line with a soft 5 px glow.
        let cursor_sx = doc_to_screen(cursor_pos.x, doc_w, zoom, center_x).round() as isize;
        if cursor_sx >= left_x_start as isize && cursor_sx < right_x_end as isize {
            for ty in 0..top_ruler_h {
                for d in -2isize..=2 {
                    let x = cursor_sx + d;
                    if x < left_x_start as isize || x >= right_x_end as isize {
                        continue;
                    }
                    let idx = (top_y_start + ty) * win_w + x as usize;
                    if idx >= buffer.len() {
                        continue;
                    }
                    if d == 0 {
                        buffer[idx] = ruler_marker;
                    } else {
                        blend_px(buffer, idx, ruler_marker, if d.abs() == 1 { 96 } else { 40 });
                    }
                }
            }
        }

        // ── Left strip ──────────────────────────────────────────────────────
        for y in (top_y_start + top_ruler_h)..bottom_y_end {
            let row = y * win_w;
            let centre = y as f32 + 0.5;
            let on_canvas = centre >= doc_y0 && centre < doc_y1;
            for rx in 0..left_ruler_w {
                let color = if rx == left_ruler_w - 1 {
                    ruler_border
                } else if rx == 0 {
                    ruler_highlight
                } else if on_canvas {
                    ruler_bg
                } else {
                    ruler_bg_off
                };
                let pixel_idx = row + left_x_start + rx;
                if pixel_idx < buffer.len() {
                    buffer[pixel_idx] = color;
                }
            }
        }

        for tick in &left_ticks {
            let len = tick.level.length();
            let color = if tick.value > doc_h {
                tick_off
            } else {
                match tick.level {
                    TickLevel::Major => tick_major,
                    TickLevel::Mid => tick_mid,
                    TickLevel::Minor => tick_minor,
                }
            };
            for tx in (left_ruler_w - 1 - len)..(left_ruler_w - 1) {
                let idx = tick.screen as usize * win_w + left_x_start + tx;
                if idx < buffer.len() {
                    buffer[idx] = color;
                }
            }
        }


        // Canvas bounds on the vertical ruler
        for bound in [0.0_f32, doc_h] {
            let sy = doc_to_screen(bound, doc_h, zoom, center_y).round() as isize;
            if sy >= (top_y_start + top_ruler_h) as isize && sy < bottom_y_end as isize {
                for tx in 1..(left_ruler_w - 1) {
                    let idx = sy as usize * win_w + left_x_start + tx;
                    if idx < buffer.len() {
                        buffer[idx] = ruler_accent;
                    }
                }
            }
        }

        // Cursor marker with the same soft glow as the top strip.
        let cursor_sy = doc_to_screen(cursor_pos.y, doc_h, zoom, center_y).round() as isize;
        if cursor_sy >= (top_y_start + top_ruler_h) as isize && cursor_sy < bottom_y_end as isize {
            for tx in 0..left_ruler_w {
                for d in -2isize..=2 {
                    let y = cursor_sy + d;
                    if y < (top_y_start + top_ruler_h) as isize || y >= bottom_y_end as isize {
                        continue;
                    }
                    let idx = y as usize * win_w + left_x_start + tx;
                    if idx >= buffer.len() {
                        continue;
                    }
                    if d == 0 {
                        buffer[idx] = ruler_marker;
                    } else {
                        blend_px(buffer, idx, ruler_marker, if d.abs() == 1 { 96 } else { 40 });
                    }
                }
            }
        }

        // Beveled corner cap, painted last so nothing spills into it.
        for ry in 0..top_ruler_h {
            for rx in 0..left_ruler_w {
                let (x, y) = (left_x_start + rx, top_y_start + ry);
                if x >= win_w || y >= win_h {
                    continue;
                }
                let color = if rx == left_ruler_w - 1 || ry == top_ruler_h - 1 {
                    ruler_border
                } else if rx == 0 || ry == 0 {
                    ruler_highlight
                } else {
                    ruler_bg
                };
                let idx = y * win_w + x;
                if idx < buffer.len() {
                    buffer[idx] = color;
                }
            }
        }
    }

    pub fn render_egui_primitives(
        &self,
        buffer: &mut [u32],
        win_w: usize,
        win_h: usize,
        primitives: &[ClippedPrimitive],
    ) {
        if win_w == 0 || win_h == 0 || buffer.len() < win_w * win_h {
            return;
        }

        for clipped in primitives {
            let Rect { min, max } = clipped.clip_rect;
            let clip_min_x = (min.x.floor() as isize).max(0).min(win_w as isize) as usize;
            let clip_max_x = (max.x.ceil() as isize).max(0).min(win_w as isize) as usize;
            let clip_min_y = (min.y.floor() as isize).max(0).min(win_h as isize) as usize;
            let clip_max_y = (max.y.ceil() as isize).max(0).min(win_h as isize) as usize;

            if clip_min_x >= clip_max_x || clip_min_y >= clip_max_y {
                continue;
            }

            if let Primitive::Mesh(mesh) = &clipped.primitive {
                let texture = self.textures.get(&mesh.texture_id);

                for triangle in mesh.indices.chunks_exact(3) {
                    let i0 = triangle[0] as usize;
                    let i1 = triangle[1] as usize;
                    let i2 = triangle[2] as usize;

                    if i0 >= mesh.vertices.len() || i1 >= mesh.vertices.len() || i2 >= mesh.vertices.len() {
                        continue;
                    }

                    let v0 = &mesh.vertices[i0];
                    let v1 = &mesh.vertices[i1];
                    let v2 = &mesh.vertices[i2];

                    let min_tx = (v0.pos.x.min(v1.pos.x).min(v2.pos.x).floor() as isize)
                        .max(clip_min_x as isize)
                        .min(clip_max_x as isize) as usize;
                    let max_tx = (v0.pos.x.max(v1.pos.x).max(v2.pos.x).ceil() as isize)
                        .max(clip_min_x as isize)
                        .min(clip_max_x as isize) as usize;
                    let min_ty = (v0.pos.y.min(v1.pos.y).min(v2.pos.y).floor() as isize)
                        .max(clip_min_y as isize)
                        .min(clip_max_y as isize) as usize;
                    let max_ty = (v0.pos.y.max(v1.pos.y).max(v2.pos.y).ceil() as isize)
                        .max(clip_min_y as isize)
                        .min(clip_max_y as isize) as usize;

                    if min_tx >= max_tx || min_ty >= max_ty {
                        continue;
                    }

                    let p0 = v0.pos;
                    let p1 = v1.pos;
                    let p2 = v2.pos;

                    let denom = (p1.y - p2.y) * (p0.x - p2.x) + (p2.x - p1.x) * (p0.y - p2.y);
                    if denom.abs() < 1e-5 {
                        continue;
                    }
                    let inv_denom = 1.0 / denom;

                    let c0 = v0.color.to_array();
                    let c1 = v1.color.to_array();
                    let c2 = v2.color.to_array();

                    for y in min_ty..max_ty {
                        let yf = y as f32 + 0.5;
                        let row_offset = y * win_w;

                        for x in min_tx..max_tx {
                            let xf = x as f32 + 0.5;

                            let w0 = ((p1.y - p2.y) * (xf - p2.x) + (p2.x - p1.x) * (yf - p2.y)) * inv_denom;
                            let w1 = ((p2.y - p0.y) * (xf - p2.x) + (p0.x - p2.x) * (yf - p2.y)) * inv_denom;
                            let w2 = 1.0 - w0 - w1;

                            if w0 >= -0.001 && w1 >= -0.001 && w2 >= -0.001 {
                                let mut r = w0 * c0[0] as f32 + w1 * c1[0] as f32 + w2 * c2[0] as f32;
                                let mut g = w0 * c0[1] as f32 + w1 * c1[1] as f32 + w2 * c2[1] as f32;
                                let mut b = w0 * c0[2] as f32 + w1 * c1[2] as f32 + w2 * c2[2] as f32;
                                let mut a = (w0 * c0[3] as f32 + w1 * c1[3] as f32 + w2 * c2[3] as f32) / 255.0;

                                if let Some((tw, th, tbytes)) = texture {
                                    let u = (w0 * v0.uv.x + w1 * v1.uv.x + w2 * v2.uv.x).clamp(0.0, 1.0);
                                    let v = (w0 * v0.uv.y + w1 * v1.uv.y + w2 * v2.uv.y).clamp(0.0, 1.0);

                                    let tx = ((u * ((*tw).max(1) as f32 - 1.0)).round() as usize).min((*tw as usize).saturating_sub(1));
                                    let ty = ((v * ((*th).max(1) as f32 - 1.0)).round() as usize).min((*th as usize).saturating_sub(1));
                                    let tidx = (ty * *tw as usize + tx) * 4;

                                    if tidx + 3 < tbytes.len() {
                                        let tr = tbytes[tidx] as f32 / 255.0;
                                        let tg = tbytes[tidx + 1] as f32 / 255.0;
                                        let tb = tbytes[tidx + 2] as f32 / 255.0;
                                        let ta = tbytes[tidx + 3] as f32 / 255.0;

                                        r = r * tr;
                                        g = g * tg;
                                        b = b * tb;
                                        a = a * ta;
                                    }
                                }

                                if a > 0.001 {
                                    let pixel_idx = row_offset + x;
                                    if pixel_idx < buffer.len() {
                                        let dst_color = buffer[pixel_idx];
                                        let dst_r = ((dst_color >> 16) & 0xFF) as f32;
                                        let dst_g = ((dst_color >> 8) & 0xFF) as f32;
                                        let dst_b = (dst_color & 0xFF) as f32;

                                        let out_r = (r * a + dst_r * (1.0 - a)).round() as u32;
                                        let out_g = (g * a + dst_g * (1.0 - a)).round() as u32;
                                        let out_b = (b * a + dst_b * (1.0 - a)).round() as u32;

                                        buffer[pixel_idx] = 0xFF000000 | (out_r << 16) | (out_g << 8) | out_b;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn draw_screen_line(&self, buffer: &mut [u32], win_w: usize, win_h: usize, p0: Vec2, p1: Vec2, color: u32, dashed: bool, clip: [usize; 4]) {
        let dist = p0.distance(p1);
        let steps = (dist.ceil() as usize).max(1);
        let [vx0, vy0, vx1, vy1] = clip;
        for i in 0..=steps {
            if dashed && ((i / 6) % 2 == 1) {
                continue;
            }
            let t = i as f32 / steps as f32;
            let p = p0.lerp(p1, t);
            let x = p.x.round() as usize;
            let y = p.y.round() as usize;
            if x >= vx0 && x < vx1 && y >= vy0 && y < vy1 && x < win_w && y < win_h {
                buffer[y * win_w + x] = color;
            }
        }
    }

    pub fn draw_screen_rect(&self, buffer: &mut [u32], win_w: usize, win_h: usize, p0: Vec2, p1: Vec2, color: u32, dashed: bool, clip: [usize; 4]) {
        let min_x = p0.x.min(p1.x);
        let max_x = p0.x.max(p1.x);
        let min_y = p0.y.min(p1.y);
        let max_y = p0.y.max(p1.y);

        let tl = Vec2::new(min_x, min_y);
        let tr = Vec2::new(max_x, min_y);
        let br = Vec2::new(max_x, max_y);
        let bl = Vec2::new(min_x, max_y);

        self.draw_screen_line(buffer, win_w, win_h, tl, tr, color, dashed, clip);
        self.draw_screen_line(buffer, win_w, win_h, tr, br, color, dashed, clip);
        self.draw_screen_line(buffer, win_w, win_h, br, bl, color, dashed, clip);
        self.draw_screen_line(buffer, win_w, win_h, bl, tl, color, dashed, clip);
    }

    pub fn draw_screen_ellipse(&self, buffer: &mut [u32], win_w: usize, win_h: usize, p0: Vec2, p1: Vec2, color: u32, dashed: bool, clip: [usize; 4]) {
        let center = (p0 + p1) * 0.5;
        let rx = (p1.x - p0.x).abs() * 0.5;
        let ry = (p1.y - p0.y).abs() * 0.5;
        if rx < 1.0 || ry < 1.0 {
            return;
        }

        let circumference = std::f32::consts::PI * (3.0 * (rx + ry) - ((3.0 * rx + ry) * (rx + 3.0 * ry)).sqrt());
        let steps = (circumference.ceil() as usize).max(12);
        let [vx0, vy0, vx1, vy1] = clip;

        for i in 0..steps {
            if dashed && ((i / 6) % 2 == 1) {
                continue;
            }
            let theta = (i as f32 / steps as f32) * std::f32::consts::TAU;
            let x = (center.x + theta.cos() * rx).round() as usize;
            let y = (center.y + theta.sin() * ry).round() as usize;
            if x >= vx0 && x < vx1 && y >= vy0 && y < vy1 && x < win_w && y < win_h {
                buffer[y * win_w + x] = color;
            }
        }
    }

    pub fn draw_screen_crosshair(&self, buffer: &mut [u32], win_w: usize, win_h: usize, center: Vec2, color: u32, clip: [usize; 4]) {
        let cx = center.x.round() as usize;
        let cy = center.y.round() as usize;
        let [vx0, vy0, vx1, vy1] = clip;
        let size = 7isize;

        for dx in -size..=size {
            let x = (cx as isize + dx) as usize;
            if x >= vx0 && x < vx1 && cy >= vy0 && cy < vy1 && x < win_w && cy < win_h {
                buffer[cy * win_w + x] = color;
            }
        }
        for dy in -size..=size {
            let y = (cy as isize + dy) as usize;
            if cx >= vx0 && cx < vx1 && y >= vy0 && y < vy1 && cx < win_w && y < win_h {
                buffer[y * win_w + cx] = color;
            }
        }
    }
}

/// Visual weight of a ruler tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickLevel {
    Minor,
    Mid,
    Major,
}

impl TickLevel {
    /// Tick length in pixels, measured inward from the strip's border.
    pub fn length(self) -> usize {
        match self {
            TickLevel::Minor => 3,
            TickLevel::Mid => 5,
            TickLevel::Major => 7,
        }
    }
}

/// One tick on a ruler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RulerTick {
    /// Position along the canvas axis, in document pixels.
    pub value: f32,
    /// Position along the strip, in screen pixels.
    pub screen: isize,
    pub level: TickLevel,
}

/// Document coordinate -> screen coordinate (unrounded).
pub fn doc_to_screen(value: f32, doc_len: f32, zoom: f32, centre: f32) -> f32 {
    (value - doc_len * 0.5) * zoom.max(0.02) + centre
}

/// Screen coordinate -> document coordinate (unrounded).
pub fn screen_to_doc(screen: f32, doc_len: f32, zoom: f32, centre: f32) -> f32 {
    (screen - centre) / zoom.max(0.02) + doc_len * 0.5
}

/// Tick ladder: `(major, minor, subdivisions_per_major)`.
///
/// Majors carry the labels; each major is split `subdivisions` times. The ladder
/// is picked so labels never collide and minors stay at least 4 px apart.
pub fn ruler_steps(zoom: f32) -> (f32, f32, u32) {
    const LADDER: [f32; 13] = [
        1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0,
    ];
    let zoom = zoom.max(0.02);
    let mut major = *LADDER.last().unwrap();
    for step in LADDER {
        if step * zoom >= 64.0 {
            major = step;
            break;
        }
    }
    // Prefer tenths (which give a mid tick at the half), then fifths, then halves.
    for subs in [10u32, 5, 2] {
        if (major / subs as f32) * zoom >= 4.0 {
            return (major, major / subs as f32, subs);
        }
    }
    (major, major, 1)
}

/// Every tick visible in the strip `[lo, hi)`, counted from canvas 0.
///
/// Ticks continue past the document edge so the canvas boundary reads clearly;
/// callers dim them via `RulerTick::value > doc_len`.
pub fn ruler_ticks(doc_len: f32, zoom: f32, centre: f32, lo: isize, hi: isize) -> Vec<RulerTick> {
    let zoom = zoom.max(0.02);
    let (_, minor, subs) = ruler_steps(zoom);
    if !(doc_len > 0.0) || hi <= lo || !zoom.is_finite() || !centre.is_finite() {
        return Vec::new();
    }

    let first = (screen_to_doc(lo as f32, doc_len, zoom, centre) / minor).floor().max(0.0) as i64;
    let last = (screen_to_doc(hi as f32, doc_len, zoom, centre) / minor).ceil().max(0.0) as i64;
    // Hard ceiling: a pathological zoom must not spin out here.
    let last = last.min(first.saturating_add(8192));

    let mut ticks = Vec::new();
    for i in first..=last {
        let value = i as f32 * minor;
        if !value.is_finite() {
            break;
        }
        let screen = doc_to_screen(value, doc_len, zoom, centre).round() as isize;
        if screen < lo || screen >= hi {
            continue;
        }
        let level = if subs > 1 && i % subs as i64 == 0 {
            TickLevel::Major
        } else if subs >= 4 && i % (subs as i64 / 2) == 0 {
            TickLevel::Mid
        } else {
            TickLevel::Minor
        };
        ticks.push(RulerTick { value, screen, level });
    }
    ticks
}

/// Width in pixels of `value` when drawn with the ruler font.
/// Alpha-composites `color` over one pixel (0xAARRGGBB, opaque output).
#[inline]
fn blend_px(buffer: &mut [u32], idx: usize, color: u32, alpha: u32) {
    let Some(&dst) = buffer.get(idx) else { return };
    if alpha == 0 {
        return;
    }
    if alpha >= 255 {
        buffer[idx] = color;
        return;
    }
    let mix = |src: u32, dst: u32| ((src * alpha + dst * (255 - alpha)) / 255) & 0xFF;
    let r = mix((color >> 16) & 0xFF, (dst >> 16) & 0xFF);
    let g = mix((color >> 8) & 0xFF, (dst >> 8) & 0xFF);
    let b = mix(color & 0xFF, dst & 0xFF);
    buffer[idx] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
}

#[cfg(test)]
mod ruler_tests {
    use super::*;

    #[test]
    fn ruler_step_keeps_labelled_ticks_apart_at_every_zoom() {
        // Label spacing must stay >= 64 px, using the usual 1/2/5 ladder.
        for (zoom, expected_major) in [(0.1_f32, 1000.0_f32), (0.41, 200.0), (1.0, 100.0), (2.0, 50.0), (8.0, 10.0)] {
            let (major, minor, subs) = ruler_steps(zoom);
            assert_eq!(major, expected_major, "major step at zoom {zoom}");
            assert!(major * zoom >= 64.0, "labels collide at zoom {zoom}");
            assert!(minor > 0.0 && minor <= major, "minor step out of range at zoom {zoom}");
            assert!(minor * zoom >= 4.0 || subs == 1, "minor ticks too dense at zoom {zoom}");
            assert!(
                (minor * subs as f32 - major).abs() < 1e-3,
                "minor x subs must equal major"
            );
        }
    }

    #[test]
    fn ruler_step_is_monotonic_as_you_zoom_in() {
        let mut previous = f32::INFINITY;
        let mut zoom = 0.05_f32;
        while zoom <= 8.0 {
            let (major, _, _) = ruler_steps(zoom);
            assert!(major <= previous, "step grew while zooming in at {zoom}");
            previous = major;
            zoom += 0.01;
        }
    }

    #[test]
    fn ticks_are_evenly_spaced_and_ordered() {
        for zoom in [0.05f32, 0.2, 0.41, 1.0, 2.5, 8.0] {
            let (_, minor, _) = ruler_steps(zoom);
            let ticks = ruler_ticks(1920.0, zoom, 700.0, 0, 900);
            assert!(ticks.len() > 1, "no ticks at zoom {zoom}");
            for pair in ticks.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                assert!(b.value > a.value, "values must ascend at zoom {zoom}");
                assert!(
                    (b.value - a.value - minor).abs() < 1e-2,
                    "uneven step at zoom {zoom}: {} then {}",
                    a.value,
                    b.value
                );
                assert!(b.screen >= a.screen, "screen positions must ascend at zoom {zoom}");
                let expected = (b.value - a.value) * zoom;
                assert!(
                    ((b.screen - a.screen) as f32 - expected).abs() <= 1.01,
                    "tick drift at zoom {zoom}: expected {expected}, got {}",
                    b.screen - a.screen
                );
            }
        }
    }

    #[test]
    fn tick_levels_follow_the_ladder() {
        let zoom = 1.0;
        let (major, minor, subs) = ruler_steps(zoom);
        let ticks = ruler_ticks(1920.0, zoom, 500.0, 0, 1200);
        for tick in &ticks {
            let index = (tick.value / minor).round() as i64;
            let expected = if index % subs as i64 == 0 {
                TickLevel::Major
            } else if subs >= 4 && index % (subs as i64 / 2) == 0 {
                TickLevel::Mid
            } else {
                TickLevel::Minor
            };
            assert_eq!(tick.level, expected, "level for value {}", tick.value);
        }
        let majors: Vec<f32> = ticks
            .iter()
            .filter(|t| t.level == TickLevel::Major)
            .map(|t| t.value)
            .collect();
        assert!(majors.len() > 2);
        for pair in majors.windows(2) {
            assert!(
                (pair[1] - pair[0] - major).abs() < 1e-2,
                "majors {} apart, expected {major}",
                pair[1] - pair[0]
            );
        }
    }

    #[test]
    fn screen_and_document_coordinates_round_trip() {
        for zoom in [0.05f32, 0.41, 1.0, 3.3, 8.0] {
            for value in [0.0_f32, 1.0, 37.5, 959.5, 1920.0] {
                let screen = doc_to_screen(value, 1920.0, zoom, 512.0);
                let back = screen_to_doc(screen, 1920.0, zoom, 512.0);
                assert!(
                    (back - value).abs() < 0.01,
                    "round trip drifted at zoom {zoom}: {value} -> {back}"
                );
            }
        }
    }

    #[test]
    fn canvas_bounds_map_to_the_canvas_edges() {
        // Centre 500, 1920 px wide at 50 %: the canvas spans 500 +/- 480.
        assert!((doc_to_screen(0.0, 1920.0, 0.5, 500.0) - 20.0).abs() < 1e-3);
        assert!((doc_to_screen(1920.0, 1920.0, 0.5, 500.0) - 980.0).abs() < 1e-3);
        assert!((doc_to_screen(960.0, 1920.0, 0.5, 500.0) - 500.0).abs() < 1e-3);
    }

    #[test]
    fn degenerate_inputs_do_not_panic() {
        assert!(ruler_ticks(0.0, 1.0, 0.0, 0, 100).is_empty());
        assert!(ruler_ticks(100.0, 1.0, 0.0, 50, 50).is_empty());
        assert!(ruler_ticks(100.0, f32::MIN_POSITIVE, 1e9, 0, 100).len() < 8193);
        assert!(ruler_ticks(1e9, 8.0, -1e9, 0, 100).len() < 8193);
        assert!(ruler_ticks(100.0, f32::NAN, 0.0, 0, 100).len() < 8193);
    }

    #[test]
    fn empty_sixteen_bit_document_renders_white_paper() {
        let doc = Document::new_with_format(16, 16, aurelia_core::layer::PixelFormat::Srgb16);
        let mut renderer = SoftwareRenderer::new();
        let mut buffer = vec![0u32; 32 * 32];
        renderer.render_canvas(
            &mut buffer,
            32,
            32,
            &doc,
            glam::Vec2::ZERO,
            1.0,
            glam::Vec2::ZERO,
            [0, 0, 32, 32],
            None,
            &[],
            None,
        );
        assert_eq!(buffer[16 * 32 + 16], 0xFFFF_FFFF, "empty 16-bit document must show white paper");
    }

    #[test]
    fn default_document_renders_white_paper() {
        let doc = Document::new(16, 16);
        let mut renderer = SoftwareRenderer::new();
        let mut buffer = vec![0u32; 32 * 32];
        renderer.render_canvas(
            &mut buffer,
            32,
            32,
            &doc,
            glam::Vec2::ZERO,
            1.0,
            glam::Vec2::ZERO,
            [0, 0, 32, 32],
            None,
            &[],
            None,
        );
        assert_eq!(buffer[16 * 32 + 16], 0xFFFF_FFFF, "empty new document must show white paper");
    }

    #[test]
    fn blending_respects_alpha() {
        let mut buffer = [0xFF00_0000u32];
        blend_px(&mut buffer, 0, 0xFFFF_FFFF, 0);
        assert_eq!(buffer[0], 0xFF00_0000, "alpha 0 must not change the pixel");

        let mut buffer = [0xFF00_0000u32];
        blend_px(&mut buffer, 0, 0xFFFF_FFFF, 255);
        assert_eq!(buffer[0], 0xFFFF_FFFF, "alpha 255 must replace the pixel");

        let mut buffer = [0xFF00_0000u32];
        blend_px(&mut buffer, 0, 0xFFFF_FFFF, 128);
        let mid = (buffer[0] >> 16) & 0xFF;
        assert!((120..=136).contains(&mid), "midpoint not blended: {mid}");
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;

    fn render(doc: &Document, win: usize, pan: Vec2, zoom: f32) -> Vec<u32> {
        let mut renderer = SoftwareRenderer::new();
        let mut buffer = vec![0u32; win * win];
        renderer.render_canvas(
            &mut buffer,
            win,
            win,
            doc,
            pan,
            zoom,
            Vec2::ZERO,
            [0, 0, win, win],
            None,
            &[],
            None,
        );
        buffer
    }

    fn px(buffer: &[u32], win: usize, x: usize, y: usize) -> u32 {
        buffer[y * win + x]
    }

    /// A transparent document must show the same checkerboard in the viewport that the
    /// Navigator samples, not an accidental black or white paper.
    #[test]
    fn transparent_document_draws_the_checkerboard() {
        let mut doc = Document::new(32, 32);
        doc.is_transparent = true;
        let buffer = render(&doc, 40, Vec2::ZERO, 1.0);

        // Doc pixel (8, 8) is in the dark tile; (24, 8) is in the light tile.
        assert_eq!(px(&buffer, 40, 12, 12), 0xFF12_1212);
        assert_eq!(px(&buffer, 40, 28, 12), 0xFF1C_1C1C);
    }

    /// The paper value is data, not a theme constant: a dark paper renders dark.
    #[test]
    fn dark_paper_renders_its_document_value() {
        let mut doc = Document::new(32, 32);
        doc.background_value = 13;
        let buffer = render(&doc, 40, Vec2::ZERO, 1.0);
        assert_eq!(px(&buffer, 40, 20, 20), 0xFF0D_0D0D);
    }

    /// Zoom and pan move the canvas bounds without changing the paper inside them.
    #[test]
    fn canvas_bounds_follow_zoom_and_pan() {
        let doc = Document::new(32, 32);
        let buffer = render(&doc, 64, Vec2::new(10.0, 0.0), 0.5);

        // 32 px at 50% is 16 px, centred at x=42, y=32: bounds 34..50 x 24..40.
        assert_eq!(px(&buffer, 64, 40, 32), 0xFFFF_FFFF, "inside the canvas");
        assert_ne!(px(&buffer, 64, 10, 32), 0xFFFF_FFFF, "left of the canvas");
        assert_ne!(px(&buffer, 64, 60, 32), 0xFFFF_FFFF, "right of the canvas");
    }

    /// The grid origin moves the lines; it must not silently snap back to the canvas corner.
    #[test]
    fn grid_origin_offsets_the_grid_lines() {
        let doc = Document::new(64, 64);
        let mut renderer = SoftwareRenderer::new();
        let mut buffer = vec![0xFF00_0000u32; 64 * 64];
        renderer.render_grid(
            &mut buffer,
            64,
            64,
            &doc,
            Vec2::ZERO,
            1.0,
            Vec2::ZERO,
            [0, 0, 64, 64],
            8,
            1.0,
            Vec2::new(4.0, 4.0),
        );
        assert_ne!(px(&buffer, 64, 4, 10), 0xFF00_0000, "line at origin x=4");
        assert_eq!(px(&buffer, 64, 0, 10), 0xFF00_0000, "no line at x=0");
        assert_ne!(px(&buffer, 64, 10, 4), 0xFF00_0000, "line at origin y=4");
        assert_eq!(px(&buffer, 64, 10, 0), 0xFF00_0000, "no line at y=0");
    }

    /// The Navigator thumbnail must use the same paper and transparency rules as the viewport.
    #[test]
    fn navigator_thumbnail_uses_the_document_paper() {
        let mut renderer = SoftwareRenderer::new();
        let mut doc = Document::new(32, 32);
        let mut buffer = vec![0u32; 40 * 40];
        renderer.render_canvas(
            &mut buffer,
            40,
            40,
            &doc,
            Vec2::ZERO,
            1.0,
            Vec2::ZERO,
            [0, 0, 40, 40],
            None,
            &[],
            None,
        );
        let image = renderer
            .navigator_thumbnail(&doc, 16)
            .expect("thumbnail from the rendered composite");
        assert_eq!(image.size, [16, 16]);
        assert!(image.pixels.iter().all(|p| *p == egui::Color32::WHITE));

        doc.background_value = 13;
        let image = renderer
            .navigator_thumbnail(&doc, 16)
            .expect("thumbnail after paper change");
        assert!(
            image
                .pixels
                .iter()
                .all(|p| *p == egui::Color32::from_rgb(13, 13, 13)),
            "the Navigator must follow the paper, not retain white"
        );
    }
}

