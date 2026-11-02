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

        let bg_val = doc.background_value;
        let dark_app_bg = 0xFF0B0806;

        // Clear frame to deep Aurelia background
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
        let border_color = 0xFF3E2F25;
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

        let ruler_bg = 0xFF120E0A;
        let ruler_border = 0xFF3E2F25;
        let ruler_tick = 0xFF8A7458;
        // Ticks past the document edge are dimmed: that area is off-canvas.
        let ruler_tick_off = 0xFF4A3D2D;
        let ruler_label = 0xFFC9B48F;
        let ruler_marker = 0xFFF6C45A;
        let ruler_accent = 0xFFF6C45A;

        let doc_w = doc.width as f32;
        let doc_h = doc.height as f32;
        let zoom = zoom.max(0.02);
        let center_x = (win_w as f32) * 0.5 + pan.x + center_offset.x;
        let center_y = (win_h as f32) * 0.5 + pan.y + center_offset.y;
        let to_sx = |v: f32| ((v - doc_w * 0.5) * zoom + center_x).round() as isize;
        let to_sy = |v: f32| ((v - doc_h * 0.5) * zoom + center_y).round() as isize;
        let to_cx = |sx: f32| (sx - center_x) / zoom + doc_w * 0.5;
        let to_cy = |sy: f32| (sy - center_y) / zoom + doc_h * 0.5;

        // Label spacing drives the step, so numbers never collide at any zoom.
        let (major_step, minor_step) = ruler_steps(zoom);
        let steps_per_major = (major_step / minor_step).round().max(1.0) as usize;

        // 1. Top horizontal ruler band
        for ry in 0..top_ruler_h {
            let y = top_y_start + ry;
            if y >= win_h {
                continue;
            }
            let row = y * win_w;
            let color = if ry == top_ruler_h - 1 { ruler_border } else { ruler_bg };
            for x in left_x_start..right_x_end {
                let pixel_idx = row + x;
                if pixel_idx < buffer.len() {
                    buffer[pixel_idx] = color;
                }
            }
        }

        let bottom_y_end = vp_max_y;

        // Ticks for the top ruler: minor every `minor_step`, labelled major every 5th.
        // The range spans the visible strip, so ticks past the canvas edge dim out.
        let first_i = (to_cx(vp_min_x as f32) / minor_step).floor().max(0.0) as usize;
        let last_i = (to_cx(right_x_end as f32) / minor_step).ceil().max(0.0) as usize;
        let mut next_label_x = f32::NEG_INFINITY;
        for i in first_i..=last_i {
            let value = i as f32 * minor_step;
            let sx = to_sx(value);
            if sx < left_x_start as isize || sx >= right_x_end as isize {
                continue;
            }
            let is_major = i % steps_per_major == 0;
            let tick_len = if is_major { 6 } else { 3 };
            let tick_color = if value > doc_w { ruler_tick_off } else { ruler_tick };
            for ty in (top_ruler_h - 1 - tick_len)..(top_ruler_h - 1) {
                let idx = (top_y_start + ty) * win_w + sx as usize;
                if idx < buffer.len() {
                    buffer[idx] = tick_color;
                }
            }

            if is_major && value <= doc_w {
                let budget = right_x_end as f32 - sx as f32 - 3.0;
                if sx as f32 >= next_label_x {
                    let used = draw_ruler_label_h(
                        buffer,
                        win_w,
                        win_h,
                        sx + 3,
                        (top_y_start + 1) as isize,
                        value.round() as i64,
                        ruler_label,
                        budget,
                    );
                    if used > 0 {
                        next_label_x = sx as f32 + 3.0 + used as f32 + 8.0;
                    }
                }
            }
        }

        // Document bounds are emphasised: 0 and the far edge.
        for bound in [0.0_f32, doc_w] {
            let sx = to_sx(bound);
            if sx >= left_x_start as isize && sx < right_x_end as isize {
                for ty in 1..(top_ruler_h - 1) {
                    let idx = (top_y_start + ty) * win_w + sx as usize;
                    if idx < buffer.len() {
                        buffer[idx] = ruler_accent;
                    }
                }
            }
        }

        // Top cursor marker (2 px so it reads at a glance)
        let cursor_sx = to_sx(cursor_pos.x);
        if cursor_sx >= left_x_start as isize && cursor_sx < right_x_end as isize {
            for ty in 0..top_ruler_h {
                for dx in 0..2 {
                    let idx = (top_y_start + ty) * win_w + (cursor_sx + dx).max(0) as usize;
                    if idx < buffer.len() && (cursor_sx + dx) < right_x_end as isize {
                        buffer[idx] = ruler_marker;
                    }
                }
            }
        }

        // 2. Left vertical ruler band (starts below the corner)
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

        // Ticks for the left ruler, mirrored from the top ruler.
        let first_j = (to_cy((top_y_start + top_ruler_h) as f32) / minor_step).floor().max(0.0) as usize;
        let last_j = (to_cy(bottom_y_end as f32) / minor_step).ceil().max(0.0) as usize;
        let mut next_label_y = f32::NEG_INFINITY;
        for i in first_j..=last_j {
            let value = i as f32 * minor_step;
            let sy = to_sy(value);
            if sy < (top_y_start + top_ruler_h) as isize || sy >= bottom_y_end as isize {
                continue;
            }
            let is_major = i % steps_per_major == 0;
            let tick_len = if is_major { 6 } else { 3 };
            let tick_color = if value > doc_h { ruler_tick_off } else { ruler_tick };
            for tx in (left_ruler_w - 1 - tick_len)..(left_ruler_w - 1) {
                let idx = sy as usize * win_w + left_x_start + tx;
                if idx < buffer.len() {
                    buffer[idx] = tick_color;
                }
            }

            if is_major && value <= doc_h {
                let budget = bottom_y_end as f32 - sy as f32 - 3.0;
                if sy as f32 >= next_label_y {
                    let used = draw_ruler_label_v(
                        buffer,
                        win_w,
                        win_h,
                        (left_x_start + 1) as isize,
                        sy + 3,
                        value.round() as i64,
                        ruler_label,
                        budget,
                    );
                    if used > 0 {
                        next_label_y = sy as f32 + 3.0 + used as f32 + 8.0;
                    }
                }
            }
        }

        // Document bounds on the vertical ruler
        for bound in [0.0_f32, doc_h] {
            let sy = to_sy(bound);
            if sy >= (top_y_start + top_ruler_h) as isize && sy < bottom_y_end as isize {
                for tx in 1..(left_ruler_w - 1) {
                    let idx = sy as usize * win_w + left_x_start + tx;
                    if idx < buffer.len() {
                        buffer[idx] = ruler_accent;
                    }
                }
            }
        }

        // Left cursor marker
        let cursor_sy = to_sy(cursor_pos.y);
        if cursor_sy >= (top_y_start + top_ruler_h) as isize && cursor_sy < bottom_y_end as isize {
            for tx in 0..left_ruler_w {
                for dy in 0..2 {
                    let idx = (cursor_sy + dy).max(0) as usize * win_w + left_x_start + tx;
                    if idx < buffer.len() && (cursor_sy + dy) < bottom_y_end as isize {
                        buffer[idx] = ruler_marker;
                    }
                }
            }
        }

        // Corner block, painted last so nothing spills into it.
        for ry in 0..top_ruler_h {
            for rx in 0..left_ruler_w {
                let (x, y) = (left_x_start + rx, top_y_start + ry);
                if x >= win_w || y >= win_h {
                    continue;
                }
                let edge = rx == left_ruler_w - 1 || ry == top_ruler_h - 1;
                let idx = y * win_w + x;
                if idx < buffer.len() {
                    buffer[idx] = if edge { ruler_border } else { ruler_bg };
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

// ── Ruler typography ────────────────────────────────────────────────────────
//
// The ruler labels are drawn straight into the framebuffer, so they use a tiny
// built-in 5x7 bitmap font scaled 2x (10x14 px per glyph). 5x7 is the smallest
// size where 2/5 and 3/8 stay unmistakably different — a 3x5 font renders "200"
// and "500" identically enough to mislead.

/// Digits 0-9 as 5x7 bitmaps; bit 4 is the leftmost column of each row.
const DIGIT_5X7: [[u8; 7]; 10] = [
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111], // 2
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110], // 3
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // 5
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // 7
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // 9
];

const RULER_GLYPH_SCALE: usize = 2;
/// Rendered size of one glyph in pixels.
const RULER_GLYPH_W: usize = 5 * RULER_GLYPH_SCALE;
const RULER_GLYPH_H: usize = 7 * RULER_GLYPH_SCALE;
/// Advance of one glyph including the inter-digit gap.
const RULER_ADVANCE: usize = RULER_GLYPH_W + RULER_GLYPH_SCALE;

/// Picks a (major, minor) tick step such that labelled ticks sit far enough apart
/// at the current zoom to hold a 5-digit label — the 1/2/5 ladder cartography uses.
fn ruler_steps(zoom: f32) -> (f32, f32) {
    const LADDER: [f32; 13] = [
        1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0,
    ];
    let zoom = zoom.max(0.01);
    let mut major = *LADDER.last().unwrap();
    for step in LADDER {
        if step * zoom >= 64.0 {
            major = step;
            break;
        }
    }
    // Five subdivisions per labelled tick, unless they would be sub-pixel.
    let minor = if (major / 5.0) * zoom >= 3.0 { major / 5.0 } else { major };
    (major, minor)
}

fn ruler_digits(value: i64) -> ([u8; 6], usize) {
    let mut out = [0u8; 6];
    let mut n = value.max(0) as u64;
    if n == 0 {
        return (out, 1);
    }
    let mut tmp = [0u8; 6];
    let mut len = 0;
    while n > 0 && len < 6 {
        tmp[len] = (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    for i in 0..len {
        out[i] = tmp[len - 1 - i];
    }
    (out, len)
}

#[inline]
fn ruler_put(buffer: &mut [u32], win_w: usize, win_h: usize, x: isize, y: isize, color: u32) {
    if x < 0 || y < 0 {
        return;
    }
    let (x, y) = (x as usize, y as usize);
    if x >= win_w || y >= win_h {
        return;
    }
    let idx = y * win_w + x;
    if idx < buffer.len() {
        buffer[idx] = color;
    }
}

/// Draws `value` left-to-right with its top-left corner at (x, y).
/// Returns the width in pixels, or 0 when it would not fit in `budget`.
fn draw_ruler_label_h(
    buffer: &mut [u32],
    win_w: usize,
    win_h: usize,
    x: isize,
    y: isize,
    value: i64,
    color: u32,
    budget: f32,
) -> usize {
    let (digits, len) = ruler_digits(value);
    let width = len * RULER_GLYPH_W + len.saturating_sub(1) * RULER_GLYPH_SCALE;
    if width as f32 > budget || x < 0 {
        return 0;
    }
    let s = RULER_GLYPH_SCALE;
    for (i, &digit) in digits[..len].iter().enumerate() {
        let glyph = DIGIT_5X7[digit as usize];
        let gx = x + (i * RULER_ADVANCE) as isize;
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5usize {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                for dy in 0..s {
                    for dx in 0..s {
                        ruler_put(
                            buffer,
                            win_w,
                            win_h,
                            gx + (col * s + dx) as isize,
                            y + (row * s + dy) as isize,
                            color,
                        );
                    }
                }
            }
        }
    }
    width
}

/// Draws `value` rotated 90° clockwise, reading downwards, top-left at (x, y).
/// Returns the height in pixels, or 0 when it would not fit in `budget`.
fn draw_ruler_label_v(
    buffer: &mut [u32],
    win_w: usize,
    win_h: usize,
    x: isize,
    y: isize,
    value: i64,
    color: u32,
    budget: f32,
) -> usize {
    let (digits, len) = ruler_digits(value);
    let height = len * RULER_GLYPH_W + len.saturating_sub(1) * RULER_GLYPH_SCALE;
    if height as f32 > budget || y < 0 {
        return 0;
    }
    let s = RULER_GLYPH_SCALE;
    for (i, &digit) in digits[..len].iter().enumerate() {
        let glyph = DIGIT_5X7[digit as usize];
        let gy = y + (i * RULER_ADVANCE) as isize;
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5usize {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                for dy in 0..s {
                    for dx in 0..s {
                        // Rotate (col, row) -> (6 - row, col) so the text runs downward.
                        let rx = (6 - row) * s + dx;
                        let ry = col * s + dy;
                        ruler_put(
                            buffer,
                            win_w,
                            win_h,
                            x + rx as isize,
                            gy + ry as isize,
                            color,
                        );
                    }
                }
            }
        }
    }
    height
}

#[cfg(test)]
mod ruler_tests {
    use super::*;

    /// Vertical mirror of a 5x7 glyph (row-order kept, columns reversed).
    fn mirror_columns(glyph: &[u8; 7]) -> [u8; 7] {
        let mut out = [0u8; 7];
        for (i, row) in glyph.iter().enumerate() {
            out[i] = row.reverse_bits() >> 3;
        }
        out
    }

    #[test]
    fn no_digit_reads_as_another_when_mirrored() {
        // This is the defect the old 3x5 font had: "2" was the mirror of "5", so a
        // 200 label read as 500. 5x7 must keep every digit unique under mirroring.
        for a in 0..10usize {
            let mirrored = mirror_columns(&DIGIT_5X7[a]);
            for b in 0..10usize {
                if a == b {
                    continue;
                }
                assert_ne!(
                    mirrored, DIGIT_5X7[b],
                    "digit {a} mirrored renders as digit {b}"
                );
            }
        }
    }

    #[test]
    fn glyphs_are_five_by_seven_and_inked() {
        for (digit, glyph) in DIGIT_5X7.iter().enumerate() {
            assert_eq!(glyph.len(), 7, "digit {digit} must have 7 rows");
            for row in glyph {
                assert!(*row <= 0b11111, "digit {digit} uses bits outside 5 columns");
            }
            let ink: u32 = glyph.iter().map(|r| r.count_ones()).sum();
            // "1" is the sparsest digit by design (a stem plus a base serif).
            assert!(ink >= 9, "digit {digit} is too faint: {ink} pixels");
        }
        assert_eq!(RULER_GLYPH_W, 10);
        assert_eq!(RULER_GLYPH_H, 14);
        // "1920" must fit the 64 px label budget the step ladder guarantees.
        let four_digit_width = 4 * RULER_GLYPH_W + 3 * RULER_GLYPH_SCALE;
        assert!(four_digit_width <= 64);
    }

    #[test]
    fn ruler_step_keeps_labelled_ticks_apart_at_every_zoom() {
        // Label spacing must stay >= 56 px, using the usual 1/2/5 ladder.
        for (zoom, expected_major) in [(0.1_f32, 1000.0_f32), (0.41, 200.0), (1.0, 100.0), (2.0, 50.0), (8.0, 10.0)] {
            let (major, minor) = ruler_steps(zoom);
            assert_eq!(major, expected_major, "major step at zoom {zoom}");
            assert!(major * zoom >= 56.0, "labels collide at zoom {zoom}");
            assert!(minor > 0.0 && minor <= major, "minor step out of range at zoom {zoom}");
        }
    }

    #[test]
    fn ruler_step_is_monotonic_as_you_zoom_in() {
        let mut previous = f32::INFINITY;
        let mut zoom = 0.05_f32;
        while zoom <= 8.0 {
            let (major, _) = ruler_steps(zoom);
            assert!(major <= previous, "step grew while zooming in at {zoom}");
            previous = major;
            zoom += 0.01;
        }
    }

    #[test]
    fn digits_come_out_most_significant_first() {
        let (digits, len) = ruler_digits(1920);
        assert_eq!(&digits[..len], &[1, 9, 2, 0]);

        let (digits, len) = ruler_digits(0);
        assert_eq!(&digits[..len], &[0]);

        let (digits, len) = ruler_digits(7);
        assert_eq!(&digits[..len], &[7]);
    }

    #[test]
    fn labels_refuse_to_draw_when_they_would_be_clipped() {
        let mut buffer = vec![0u32; 120 * 40];
        let white = 0xFFFFFFFF;

        // 90 -> 90 + 30 px of label overflows a 10 px budget: nothing may be drawn.
        let used = draw_ruler_label_h(&mut buffer, 120, 40, 90, 1, 1920, white, 10.0);
        assert_eq!(used, 0);
        assert!(buffer.iter().all(|&p| p == 0));

        let used = draw_ruler_label_h(&mut buffer, 120, 40, 2, 1, 40, white, 60.0);
        assert!(used > 0);
        assert!(buffer.iter().any(|&p| p == white));
    }

    #[test]
    fn vertical_labels_rotate_inside_their_budget() {
        let mut buffer = vec![0u32; 40 * 120];
        let white = 0xFFFFFFFF;

        let used = draw_ruler_label_v(&mut buffer, 40, 120, 1, 2, 1080, white, 12.0);
        assert_eq!(used, 0, "label taller than the budget must be skipped");

        let used = draw_ruler_label_v(&mut buffer, 40, 120, 1, 2, 1080, white, 60.0);
        assert!(used > 0);
        assert!(buffer.iter().any(|&p| p == white));
    }
}


