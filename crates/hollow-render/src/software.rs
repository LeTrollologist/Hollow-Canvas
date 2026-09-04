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

        let bg_val = doc.background_value;
        let dark_app_bg = 0xFF04060E;

        // Clear frame to deep Hollow background
        buffer.fill(dark_app_bg);

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
        let shadow_blur = 14isize;
        let shadow_offset_y = 5isize;
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
        let border_color = 0xFF354468;
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
        perspective: &hollow_core::perspective::PerspectiveConfig,
    ) {
        if win_w == 0 || win_h == 0 || !perspective.show_guides || perspective.p_type == hollow_core::perspective::PerspectiveType::None || perspective.guide_opacity <= 0.001 {
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
                0 => 0xFF4A90E2, // Left VP: Azure Blue
                1 => 0xFF50E3C2, // Right VP: Turquoise Green
                2 => 0xFFE056FD, // Vertical VP: Violet Pink
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

        let ruler_bg = 0xFF080D1D;
        let ruler_border = 0xFF24335C;
        let ruler_tick = 0xFF5C72A0;
        let ruler_marker = 0xFF5CE0D8;

        let top_ruler_h = 16usize;
        let left_ruler_w = 16usize;
        let top_y_start = vp_min_y;
        let left_x_start = vp_min_x;
        let right_x_end = vp_max_x;

        let doc_w = doc.width as f32;
        let doc_h = doc.height as f32;
        let center_x = (win_w as f32) * 0.5 + pan.x + center_offset.x;
        let center_y = (win_h as f32) * 0.5 + pan.y + center_offset.y;

        // 1. Render Top Horizontal Ruler
        for ry in 0..top_ruler_h {
            let y = top_y_start + ry;
            if y >= win_h {
                continue;
            }
            let row = y * win_w;
            for x in left_x_start..right_x_end {
                let pixel_idx = row + x;
                if pixel_idx < buffer.len() {
                    buffer[pixel_idx] = if ry == top_ruler_h - 1 { ruler_border } else { ruler_bg };
                }
            }
        }

        let bottom_y_end = vp_max_y;

        // Ticks for top ruler
        let step = if zoom > 4.0 { 10.0 } else if zoom > 1.5 { 50.0 } else if zoom > 0.5 { 100.0 } else { 500.0 };
        let mut cur_tick = 0.0_f32;
        while cur_tick <= doc_w {
            let sx = ((cur_tick - doc_w * 0.5) * zoom + center_x).round() as isize;
            if sx >= left_x_start as isize && sx < right_x_end as isize {
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
        if cursor_sx >= left_x_start as isize && cursor_sx < right_x_end as isize {
            for ty in 0..top_ruler_h {
                let y = top_y_start + ty;
                let idx = y * win_w + cursor_sx as usize;
                if idx < buffer.len() {
                    buffer[idx] = ruler_marker;
                }
            }
        }

        // 2. Render Left Vertical Ruler
        for y in (top_y_start + top_ruler_h)..bottom_y_end {
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
            if sy >= (top_y_start + top_ruler_h) as isize && sy < bottom_y_end as isize {
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
        if cursor_sy >= (top_y_start + top_ruler_h) as isize && cursor_sy < bottom_y_end as isize {
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
