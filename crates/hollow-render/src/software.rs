use egui::epaint::Primitive;
use egui::{ClippedPrimitive, ImageData, Rect, TextureId, TexturesDelta};
use glam::Vec2;
use hollow_core::document::Document;
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
        tracing: Option<TracingReferenceConfig>,
    ) {
        if win_w == 0 || win_h == 0 || buffer.len() < win_w * win_h {
            return;
        }

        let bg_val = doc.background_value;
        let dark_app_bg = 0xFF04060E;

        // Clear frame to deep Hollow background
        buffer.fill(dark_app_bg);

        let doc_w = doc.width as f32;
        let doc_h = doc.height as f32;
        let center_x = (win_w as f32) * 0.5 + pan.x;
        let center_y = (win_h as f32) * 0.5 + pan.y;

        let canvas_w = doc_w * zoom;
        let canvas_h = doc_h * zoom;

        let x0 = (center_x - canvas_w * 0.5).floor() as isize;
        let y0 = (center_y - canvas_h * 0.5).floor() as isize;
        let x1 = (center_x + canvas_w * 0.5).ceil() as isize;
        let y1 = (center_y + canvas_h * 0.5).ceil() as isize;

        let min_x = x0.max(0).min(win_w as isize) as usize;
        let max_x = x1.max(0).min(win_w as isize) as usize;
        let min_y = y0.max(0).min(win_h as isize) as usize;
        let max_y = y1.max(0).min(win_h as isize) as usize;

        if min_x >= max_x || min_y >= max_y {
            return;
        }

        let req_size = (doc.width * doc.height * 4) as usize;
        if self.composite_buffer.len() != req_size {
            self.composite_buffer.resize(req_size, 0);
        }
        doc.composite_layers_into(&mut self.composite_buffer, false);
        let flat_pixels = &self.composite_buffer;
        let inv_zoom = 1.0 / zoom;

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
                if src_idx + 3 >= flat_pixels.len() {
                    continue;
                }

                let lr = flat_pixels[src_idx];
                let lg = flat_pixels[src_idx + 1];
                let lb = flat_pixels[src_idx + 2];
                let la = flat_pixels[src_idx + 3];

                let pixel_idx = row_offset + screen_x;
                if pixel_idx >= buffer.len() {
                    continue;
                }

                // Base paper / background color
                let (mut base_r, mut base_g, mut base_b) = if doc.is_transparent {
                    let tile_x = (doc_xi / 16) % 2;
                    let tile_y = (doc_yi / 16) % 2;
                    let is_dark = tile_x == tile_y;
                    let check_val = if is_dark { 18u8 } else { 28u8 };
                    (check_val, check_val, check_val)
                } else {
                    (bg_val, bg_val, bg_val)
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

                // 1. If Tracing as Underlay (Light Table): blend reference onto base first
                if let Some((rr, rg, rb, ra)) = ref_px {
                    if tracing.map_or(false, |t| t.is_underlay) && ra > 0 {
                        let a_u = ra as u32;
                        let inv_a = 255 - a_u;
                        base_r = ((base_r as u32 * inv_a + rr as u32 * a_u) / 255) as u8;
                        base_g = ((base_g as u32 * inv_a + rg as u32 * a_u) / 255) as u8;
                        base_b = ((base_b as u32 * inv_a + rb as u32 * a_u) / 255) as u8;
                    }
                }

                // 2. Blend Document Layers on top of base
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

                // 3. If Tracing as Ghost Overlay (Tracing Sheet): blend reference on top
                let (final_r, final_g, final_b) = if let Some((rr, rg, rb, ra)) = ref_px {
                    if tracing.map_or(false, |t| !t.is_underlay) && ra > 0 {
                        let a_u = ra as u32;
                        let inv_a = 255 - a_u;
                        let r = ((comp_r as u32 * inv_a + rr as u32 * a_u) / 255) as u8;
                        let g = ((comp_g as u32 * inv_a + rg as u32 * a_u) / 255) as u8;
                        let b = ((comp_b as u32 * inv_a + rb as u32 * a_u) / 255) as u8;
                        (r, g, b)
                    } else {
                        (comp_r, comp_g, comp_b)
                    }
                } else {
                    (comp_r, comp_g, comp_b)
                };

                buffer[pixel_idx] = 0xFF000000 | ((final_r as u32) << 16) | ((final_g as u32) << 8) | (final_b as u32);
            }
        }
    }

    pub fn render_grid(
        &self,
        buffer: &mut [u32],
        win_w: usize,
        win_h: usize,
        doc: &Document,
        pan: Vec2,
        zoom: f32,
        grid_size: u32,
        grid_opacity: f32,
    ) {
        if win_w == 0 || win_h == 0 || grid_size == 0 || grid_opacity <= 0.001 {
            return;
        }

        let doc_w = doc.width as f32;
        let doc_h = doc.height as f32;
        let center_x = (win_w as f32) * 0.5 + pan.x;
        let center_y = (win_h as f32) * 0.5 + pan.y;

        // Screen bounds of the canvas
        let x_start = (((0.0 - doc_w * 0.5) * zoom + center_x).round() as isize).max(0).min(win_w as isize) as usize;
        let x_end = (((doc_w - doc_w * 0.5) * zoom + center_x).round() as isize).max(0).min(win_w as isize) as usize;
        let y_start = (((0.0 - doc_h * 0.5) * zoom + center_y).round() as isize).max(0).min(win_h as isize) as usize;
        let y_end = (((doc_h - doc_h * 0.5) * zoom + center_y).round() as isize).max(0).min(win_h as isize) as usize;

        if x_start >= x_end || y_start >= y_end {
            return;
        }

        let alpha = grid_opacity.clamp(0.0, 1.0);
        let alpha_u = (alpha * 256.0) as u32;
        let inv_alpha_u = 256 - alpha_u;
        let gr = 168u32;
        let gg = 159u32;
        let gb = 216u32;

        // 1. Draw Vertical Grid Lines
        let mut gx = 0u32;
        while gx <= doc.width {
            let sx = ((gx as f32 - doc_w * 0.5) * zoom + center_x).round() as isize;
            if sx >= x_start as isize && sx < x_end as isize {
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
            gx += grid_size;
        }

        // 2. Draw Horizontal Grid Lines
        let mut gy = 0u32;
        while gy <= doc.height {
            let sy = ((gy as f32 - doc_h * 0.5) * zoom + center_y).round() as isize;
            if sy >= y_start as isize && sy < y_end as isize {
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
            gy += grid_size;
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
        cursor_pos: Vec2,
    ) {
        if win_w < 250 || win_h < 150 {
            return;
        }

        let ruler_bg = 0xFF080D1D;
        let ruler_border = 0xFF24335C;
        let ruler_tick = 0xFF5C72A0;
        let ruler_marker = 0xFF5CE0D8;

        let top_ruler_h = 16usize;
        let left_ruler_w = 16usize;
        let top_y_start = 42usize; // Under header dock
        let left_x_start = 200usize; // Beside tool dock

        let doc_w = doc.width as f32;
        let doc_h = doc.height as f32;
        let center_x = (win_w as f32) * 0.5 + pan.x;
        let center_y = (win_h as f32) * 0.5 + pan.y;

        // 1. Render Top Horizontal Ruler
        for ry in 0..top_ruler_h {
            let y = top_y_start + ry;
            if y >= win_h {
                continue;
            }
            let row = y * win_w;
            for x in left_x_start..win_w.saturating_sub(230) {
                let pixel_idx = row + x;
                if pixel_idx < buffer.len() {
                    buffer[pixel_idx] = if ry == top_ruler_h - 1 { ruler_border } else { ruler_bg };
                }
            }
        }

        // Ticks for top ruler
        let step = if zoom > 4.0 { 10.0 } else if zoom > 1.5 { 50.0 } else if zoom > 0.5 { 100.0 } else { 500.0 };
        let mut cur_tick = 0.0_f32;
        while cur_tick <= doc_w {
            let sx = ((cur_tick - doc_w * 0.5) * zoom + center_x).round() as isize;
            if sx >= left_x_start as isize && sx < (win_w - 230) as isize {
                let tick_len = if (cur_tick % (step * 2.0)).abs() < 0.1 { 10 } else { 5 };
                for ty in (top_ruler_h - tick_len)..top_ruler_h {
                    let y = top_y_start + ty;
                    let idx = y * win_w + sx as usize;
                    if idx < buffer.len() {
                        buffer[idx] = ruler_tick;
                    }
                }
            }
            cur_tick += step;
        }

        // Top Cursor Marker
        let cursor_sx = ((cursor_pos.x - doc_w * 0.5) * zoom + center_x).round() as isize;
        if cursor_sx >= left_x_start as isize && cursor_sx < (win_w - 230) as isize {
            for ty in 0..top_ruler_h {
                let y = top_y_start + ty;
                let idx = y * win_w + cursor_sx as usize;
                if idx < buffer.len() {
                    buffer[idx] = ruler_marker;
                }
            }
        }

        // 2. Render Left Vertical Ruler
        for y in (top_y_start + top_ruler_h)..win_h.saturating_sub(28) {
            let row = y * win_w;
            for rx in 0..left_ruler_w {
                let x = left_x_start + rx;
                let pixel_idx = row + x;
                if pixel_idx < buffer.len() {
                    buffer[pixel_idx] = if rx == left_ruler_w - 1 { ruler_border } else { ruler_bg };
                }
            }
        }

        // Ticks for left ruler
        let mut cur_tick_y = 0.0_f32;
        while cur_tick_y <= doc_h {
            let sy = ((cur_tick_y - doc_h * 0.5) * zoom + center_y).round() as isize;
            if sy >= (top_y_start + top_ruler_h) as isize && sy < (win_h - 28) as isize {
                let tick_len = if (cur_tick_y % (step * 2.0)).abs() < 0.1 { 10 } else { 5 };
                for tx in (left_ruler_w - tick_len)..left_ruler_w {
                    let x = left_x_start + tx;
                    let idx = sy as usize * win_w + x;
                    if idx < buffer.len() {
                        buffer[idx] = ruler_tick;
                    }
                }
            }
            cur_tick_y += step;
        }

        // Left Cursor Marker
        let cursor_sy = ((cursor_pos.y - doc_h * 0.5) * zoom + center_y).round() as isize;
        if cursor_sy >= (top_y_start + top_ruler_h) as isize && cursor_sy < (win_h - 28) as isize {
            for tx in 0..left_ruler_w {
                let x = left_x_start + tx;
                let idx = cursor_sy as usize * win_w + x;
                if idx < buffer.len() {
                    buffer[idx] = ruler_marker;
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

                                    let tx = ((u * (*tw as f32 - 1.0)).round() as usize).min(*tw as usize - 1);
                                    let ty = ((v * (*th as f32 - 1.0)).round() as usize).min(*th as usize - 1);
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

    pub fn draw_screen_line(&self, buffer: &mut [u32], win_w: usize, win_h: usize, p0: Vec2, p1: Vec2, color: u32, dashed: bool) {
        let dist = p0.distance(p1);
        let steps = (dist.ceil() as usize).max(1);
        for i in 0..=steps {
            if dashed && ((i / 6) % 2 == 1) {
                continue;
            }
            let t = i as f32 / steps as f32;
            let p = p0.lerp(p1, t);
            let x = p.x.round() as i32;
            let y = p.y.round() as i32;
            if x >= 0 && x < win_w as i32 && y >= 0 && y < win_h as i32 {
                buffer[y as usize * win_w + x as usize] = color;
            }
        }
    }

    pub fn draw_screen_rect(&self, buffer: &mut [u32], win_w: usize, win_h: usize, p0: Vec2, p1: Vec2, color: u32, dashed: bool) {
        let min_x = p0.x.min(p1.x);
        let max_x = p0.x.max(p1.x);
        let min_y = p0.y.min(p1.y);
        let max_y = p0.y.max(p1.y);

        let tl = Vec2::new(min_x, min_y);
        let tr = Vec2::new(max_x, min_y);
        let br = Vec2::new(max_x, max_y);
        let bl = Vec2::new(min_x, max_y);

        self.draw_screen_line(buffer, win_w, win_h, tl, tr, color, dashed);
        self.draw_screen_line(buffer, win_w, win_h, tr, br, color, dashed);
        self.draw_screen_line(buffer, win_w, win_h, br, bl, color, dashed);
        self.draw_screen_line(buffer, win_w, win_h, bl, tl, color, dashed);
    }

    pub fn draw_screen_ellipse(&self, buffer: &mut [u32], win_w: usize, win_h: usize, p0: Vec2, p1: Vec2, color: u32, dashed: bool) {
        let center = (p0 + p1) * 0.5;
        let rx = (p1.x - p0.x).abs() * 0.5;
        let ry = (p1.y - p0.y).abs() * 0.5;
        if rx < 1.0 || ry < 1.0 {
            return;
        }

        let circumference = std::f32::consts::PI * (3.0 * (rx + ry) - ((3.0 * rx + ry) * (rx + 3.0 * ry)).sqrt());
        let steps = (circumference.ceil() as usize).max(12);

        for i in 0..steps {
            if dashed && ((i / 6) % 2 == 1) {
                continue;
            }
            let theta = (i as f32 / steps as f32) * std::f32::consts::TAU;
            let x = (center.x + theta.cos() * rx).round() as i32;
            let y = (center.y + theta.sin() * ry).round() as i32;
            if x >= 0 && x < win_w as i32 && y >= 0 && y < win_h as i32 {
                buffer[y as usize * win_w + x as usize] = color;
            }
        }
    }

    pub fn draw_screen_crosshair(&self, buffer: &mut [u32], win_w: usize, win_h: usize, center: Vec2, color: u32) {
        let cx = center.x.round() as i32;
        let cy = center.y.round() as i32;
        let size = 7;

        for dx in -size..=size {
            let x = cx + dx;
            if x >= 0 && x < win_w as i32 && cy >= 0 && cy < win_h as i32 {
                buffer[cy as usize * win_w + x as usize] = color;
            }
        }
        for dy in -size..=size {
            let y = cy + dy;
            if cx >= 0 && cx < win_w as i32 && y >= 0 && y < win_h as i32 {
                buffer[y as usize * win_w + cx as usize] = color;
            }
        }
    }
}
