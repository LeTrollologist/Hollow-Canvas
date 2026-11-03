use crate::blend::BlendMode;
use crate::color::{Color, ThemeMode};
use crate::layer::{Layer, LayerId, LayerBuffer, LayerMask};
use crate::pixel::Channel;
use crate::selection::SelectionMask;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub active_layer_id: LayerId,
    pub background_value: u8,
    pub is_transparent: bool,
    pub theme: ThemeMode,
    pub next_layer_id: u64,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        let first_id = 1;
        let base_layer = Layer::new(first_id, "Layer 1", width, height);
        Self {
            width,
            height,
            layers: vec![base_layer],
            active_layer_id: first_id,
            background_value: 255, // Default pristine white studio canvas paper
            is_transparent: false,
            theme: ThemeMode::Dawn,
            next_layer_id: 2,
        }
    }

    pub fn background_color(&self) -> Color {
        if self.is_transparent {
            Color::TRANSPARENT
        } else {
            let v = self.background_value as f32 / 255.0;
            Color::new(v, v, v, 1.0)
        }
    }

    pub fn active_layer(&self) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == self.active_layer_id)
    }

    pub fn active_layer_mut(&mut self) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == self.active_layer_id)
    }

    pub fn get_layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn get_layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn reference_layer(&self) -> Option<&Layer> {
        self.layers.iter().find(|l| l.is_reference && l.visible)
    }

    pub fn add_layer(&mut self, name: Option<String>) -> LayerId {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        let layer_name = name.unwrap_or_else(|| format!("Layer {}", id));
        let layer = Layer::new(id, layer_name, self.width, self.height);
        self.layers.push(layer);
        self.active_layer_id = id;
        id
    }

    pub fn add_group(&mut self, name: Option<String>) -> LayerId {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        let group_name = name.unwrap_or_else(|| format!("Group {}", id));
        let group = Layer::new_group(id, group_name);
        self.layers.push(group);
        self.active_layer_id = id;
        id
    }

    pub fn add_adjustment_layer(&mut self, adj_type: crate::layer::AdjustmentType, name: Option<String>) -> LayerId {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        let layer_name = name.unwrap_or_else(|| format!("{} {}", adj_type.name(), id));
        let layer = Layer::new_adjustment(id, layer_name, adj_type);
        self.layers.push(layer);
        self.active_layer_id = id;
        id
    }

    pub fn add_text_layer(&mut self, name: Option<String>, config: crate::layer::TextLayerConfig, parent_id: Option<LayerId>) -> LayerId {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        let layer_name = name.unwrap_or_else(|| {
            if config.content.trim().is_empty() {
                format!("Text {}", id)
            } else {
                config.content.chars().take(20).collect::<String>()
            }
        });
        let mut layer = Layer::new_text(id, layer_name, self.width, self.height, config);
        layer.parent_id = parent_id;
        self.layers.push(layer);
        self.active_layer_id = id;
        id
    }

    pub fn update_text_layer_config(
        &mut self,
        layer_id: LayerId,
        config: crate::layer::TextLayerConfig,
        font_ref: &ab_glyph::FontArc,
    ) -> bool {
        if let Some(layer) = self.get_layer_mut(layer_id) {
            if layer.is_text() {
                layer.text = Some(config.clone());
                crate::rasterizer::StrokeRasterizer::rasterize_text_layer_config(layer, &config, font_ref);
                return true;
            }
        }
        false
    }

    pub fn rasterize_text_layer(&mut self, layer_id: LayerId) -> bool {
        if let Some(layer) = self.get_layer_mut(layer_id) {
            if layer.is_text() {
                layer.rasterize_text();
                return true;
            }
        }
        false
    }

    pub fn group_children(&self, parent_id: LayerId) -> Vec<LayerId> {
        self.layers
            .iter()
            .filter(|l| l.parent_id == Some(parent_id))
            .map(|l| l.id)
            .collect()
    }

    pub fn all_descendants(&self, parent_id: LayerId) -> Vec<LayerId> {
        let mut result = Vec::new();
        let mut queue = self.group_children(parent_id);
        while let Some(child_id) = queue.pop() {
            result.push(child_id);
            let sub_children = self.group_children(child_id);
            queue.extend(sub_children);
        }
        result
    }

    pub fn is_layer_effective_visible(&self, id: LayerId) -> bool {
        let mut curr_id = id;
        while let Some(layer) = self.get_layer(curr_id) {
            if !layer.visible {
                return false;
            }
            if let Some(parent_id) = layer.parent_id {
                curr_id = parent_id;
            } else {
                break;
            }
        }
        true
    }

    pub fn effective_opacity(&self, id: LayerId) -> f32 {
        let mut opacity = 1.0_f32;
        let mut curr_id = id;
        while let Some(layer) = self.get_layer(curr_id) {
            opacity *= layer.opacity.clamp(0.0, 1.0);
            if let Some(parent_id) = layer.parent_id {
                curr_id = parent_id;
            } else {
                break;
            }
        }
        opacity.clamp(0.0, 1.0)
    }

    pub fn set_layer_parent(&mut self, layer_id: LayerId, new_parent_id: Option<LayerId>) -> bool {
        if let Some(pid) = new_parent_id {
            if pid == layer_id {
                return false;
            }
            let descendants = self.all_descendants(layer_id);
            if descendants.contains(&pid) {
                return false;
            }
            if !self.layers.iter().any(|l| l.id == pid && l.is_group()) {
                return false;
            }
        }

        if let Some(layer) = self.get_layer_mut(layer_id) {
            layer.parent_id = new_parent_id;
            true
        } else {
            false
        }
    }

    pub fn ungroup(&mut self, group_id: LayerId) -> bool {
        let target_parent = self.get_layer(group_id).and_then(|g| g.parent_id);
        for l in &mut self.layers {
            if l.parent_id == Some(group_id) {
                l.parent_id = target_parent;
            }
        }
        self.delete_layer(group_id).is_some()
    }

    pub fn duplicate_active_layer(&mut self) -> Option<LayerId> {
        let active = self.active_layer()?.clone();
        if active.is_group() {
            let new_group_id = self.next_layer_id;
            self.next_layer_id += 1;
            let mut dup_group = active.duplicate(new_group_id);
            dup_group.name = format!("{} Copy", active.name);

            let group_idx = self.layers.iter().position(|l| l.id == active.id).unwrap();
            self.layers.insert(group_idx + 1, dup_group);

            // Duplicate all descendants
            let children = self.group_children(active.id);
            for child_id in children {
                if let Some(child) = self.get_layer(child_id).cloned() {
                    let new_child_id = self.next_layer_id;
                    self.next_layer_id += 1;
                    let mut dup_child = child.duplicate(new_child_id);
                    dup_child.parent_id = Some(new_group_id);
                    self.layers.push(dup_child);
                }
            }

            self.active_layer_id = new_group_id;
            Some(new_group_id)
        } else {
            let new_id = self.next_layer_id;
            self.next_layer_id += 1;
            let dup = active.duplicate(new_id);

            let idx = self.layers.iter().position(|l| l.id == active.id).unwrap();
            self.layers.insert(idx + 1, dup);
            self.active_layer_id = new_id;
            Some(new_id)
        }
    }

    pub fn delete_layer(&mut self, id: LayerId) -> Option<Layer> {
        if self.layers.len() <= 1 {
            return None; // Maintain at least 1 layer
        }
        // If deleting a group, recursively collect all descendants
        let descendants = self.all_descendants(id);
        self.layers.retain(|l| !descendants.contains(&l.id));

        let idx = self.layers.iter().position(|l| l.id == id)?;
        let removed = self.layers.remove(idx);
        if self.active_layer_id == id || descendants.contains(&self.active_layer_id) {
            let next_idx = idx.min(self.layers.len() - 1);
            self.active_layer_id = self.layers[next_idx].id;
        }
        Some(removed)
    }

    /// Replaces `layer_id`'s mask with one derived from `selection`.
    ///
    /// `None` (or a selection that does not cover the canvas) produces a fully
    /// visible mask, which is the state a fresh mask starts in before it is painted.
    pub fn add_layer_mask(&mut self, layer_id: LayerId, selection: Option<&SelectionMask>) -> bool {
        let (w, h) = (self.width, self.height);
        let Some(layer) = self.layers.iter_mut().find(|l| l.id == layer_id) else {
            return false;
        };
        let mut mask = LayerMask::full(w, h, 255);
        if let Some(sel) = selection.filter(|s| s.has_selection()) {
            if sel.width == w && sel.height == h && sel.mask.len() == mask.pixels.len() {
                mask.pixels.copy_from_slice(&sel.mask);
            }
        }
        layer.mask = Some(mask);
        true
    }

    pub fn invert_layer_mask(&mut self, layer_id: LayerId) -> bool {
        match self.layers.iter_mut().find(|l| l.id == layer_id).and_then(|l| l.mask.as_mut()) {
            Some(mask) => {
                mask.invert();
                true
            }
            None => false,
        }
    }

    pub fn set_layer_mask_enabled(&mut self, layer_id: LayerId, enabled: bool) -> bool {
        match self.layers.iter_mut().find(|l| l.id == layer_id).and_then(|l| l.mask.as_mut()) {
            Some(mask) => {
                mask.enabled = enabled;
                true
            }
            None => false,
        }
    }

    pub fn remove_layer_mask(&mut self, layer_id: LayerId) -> bool {
        match self.layers.iter_mut().find(|l| l.id == layer_id) {
            Some(layer) => layer.mask.take().is_some(),
            None => false,
        }
    }

    pub fn move_layer(&mut self, id: LayerId, delta: i32) {
        if let Some(idx) = self.layers.iter().position(|l| l.id == id) {
            let new_idx = (idx as i32 + delta).clamp(0, self.layers.len() as i32 - 1) as usize;
            if new_idx != idx {
                let layer = self.layers.remove(idx);
                self.layers.insert(new_idx, layer);
            }
        }
    }

    pub fn merge_layer_down(&mut self) -> bool {
        let active_idx = match self.layers.iter().position(|l| l.id == self.active_layer_id) {
            Some(i) if i > 0 => i,
            _ => return false,
        };

        let upper = self.layers.remove(active_idx);
        let lower = &mut self.layers[active_idx - 1];

        // Composite upper into lower, respecting offsets
        for y in 0..self.height {
            for x in 0..self.width {
                // Sample lower layer at its local coords
                let lx = x as i32 - lower.offset_x;
                let ly = y as i32 - lower.offset_y;
                let lower_px = if lx >= 0 && lx < self.width as i32 && ly >= 0 && ly < self.height as i32 {
                    lower.get_pixel(lx as u32, ly as u32).unwrap_or([0, 0, 0, 0])
                } else {
                    [0, 0, 0, 0]
                };

                // Pre-multiply lower layer's opacity into its pixels
                let lower_baked = BlendMode::Normal.composite_pixel([0, 0, 0, 0], lower_px, lower.opacity);

                // Sample upper layer at its local coords
                let ux = x as i32 - upper.offset_x;
                let uy = y as i32 - upper.offset_y;
                let upper_px = if ux >= 0 && ux < self.width as i32 && uy >= 0 && uy < self.height as i32 {
                    upper.get_pixel(ux as u32, uy as u32).unwrap_or([0, 0, 0, 0])
                } else {
                    [0, 0, 0, 0]
                };

                let blended = upper.blend_mode.composite_pixel(lower_baked, upper_px, upper.opacity);

                // Store result at canvas coordinate in lower layer
                // We need to write at the lower layer's local coordinate
                if lx >= 0 && lx < self.width as i32 && ly >= 0 && ly < self.height as i32 {
                    lower.set_pixel(lx as u32, ly as u32, blended);
                }
            }
        }

        // Reset lower layer's opacity since we baked it in
        lower.opacity = 1.0;
        lower.offset_x = 0;
        lower.offset_y = 0;
        self.active_layer_id = lower.id;
        true
    }

    pub fn merge_visible_layers(&mut self) {
        if self.layers.len() <= 1 {
            return;
        }

        let flat = self.composite_layers(true);
        let id = self.next_layer_id;
        self.next_layer_id += 1;

        let merged = Layer::from_pixels(id, "Merged", self.width, self.height, flat);
        self.layers.clear();
        self.layers.push(merged);
        self.active_layer_id = id;
    }

    pub fn resize(&mut self, new_w: u32, new_h: u32) {
        self.width = new_w;
        self.height = new_h;
        for layer in &mut self.layers {
            layer.resize(new_w, new_h);
        }
    }

    pub fn flip(&mut self, horizontal: bool) {
        let w = self.width;
        let h = self.height;

        for layer in &mut self.layers {
            let mut flipped = vec![0u8; layer.as_srgb8_exact().len()];
            for y in 0..h {
                for x in 0..w {
                    let src_idx = ((y * w + x) * 4) as usize;
                    let dst_x = if horizontal { w - 1 - x } else { x };
                    let dst_y = if !horizontal { h - 1 - y } else { y };
                    let dst_idx = ((dst_y * w + dst_x) * 4) as usize;

                    flipped[dst_idx..dst_idx + 4].copy_from_slice(&layer.as_srgb8_exact()[src_idx..src_idx + 4]);
                }
            }
            layer.set_pixels(flipped);
        }
    }

    pub fn rotate_90(&mut self, clockwise: bool) {
        let old_w = self.width;
        let old_h = self.height;
        let new_w = old_h;
        let new_h = old_w;

        for layer in &mut self.layers {
            let mut rotated = vec![0u8; (new_w * new_h * 4) as usize];
            for y in 0..old_h {
                for x in 0..old_w {
                    let src_idx = ((y * old_w + x) * 4) as usize;
                    let (dst_x, dst_y) = if clockwise {
                        (old_h - 1 - y, x)
                    } else {
                        (y, old_w - 1 - x)
                    };
                    let dst_idx = ((dst_y * new_w + dst_x) * 4) as usize;
                    rotated[dst_idx..dst_idx + 4].copy_from_slice(&layer.as_srgb8_exact()[src_idx..src_idx + 4]);
                }
            }
            layer.width = new_w;
            layer.height = new_h;
            layer.set_pixels(rotated);
        }

        self.width = new_w;
        self.height = new_h;
    }

    pub fn rotate_180(&mut self) {
        self.flip(true);
        self.flip(false);
    }

    /// Resize canvas dimensions with an anchor offset
    pub fn resize_canvas(&mut self, new_w: u32, new_h: u32, offset_x: i32, offset_y: i32) {
        if new_w == 0 || new_h == 0 {
            return;
        }
        let old_w = self.width as i32;
        let old_h = self.height as i32;

        for layer in &mut self.layers {
            let mut new_pixels = vec![0u8; (new_w * new_h * 4) as usize];
            for ny in 0..new_h as i32 {
                let oy = ny - offset_y;
                if oy < 0 || oy >= old_h {
                    continue;
                }
                for nx in 0..new_w as i32 {
                    let ox = nx - offset_x;
                    if ox < 0 || ox >= old_w {
                        continue;
                    }
                    let src_idx = ((oy * old_w + ox) * 4) as usize;
                    let dst_idx = ((ny * new_w as i32 + nx) * 4) as usize;
                    if src_idx + 4 <= layer.as_srgb8_exact().len() && dst_idx + 4 <= new_pixels.len() {
                        new_pixels[dst_idx..dst_idx + 4].copy_from_slice(&layer.as_srgb8_exact()[src_idx..src_idx + 4]);
                    }
                }
            }
            layer.width = new_w;
            layer.height = new_h;
            layer.set_pixels(new_pixels);
        }

        self.width = new_w;
        self.height = new_h;
    }

    /// Scale canvas and resample all layers using nearest neighbor or bilinear interpolation
    pub fn scale_canvas(&mut self, new_w: u32, new_h: u32, bilinear: bool) {
        if new_w == 0 || new_h == 0 {
            return;
        }
        let old_w = self.width;
        let old_h = self.height;

        let scale_x = old_w as f32 / new_w as f32;
        let scale_y = old_h as f32 / new_h as f32;

        for layer in &mut self.layers {
            let mut new_pixels = vec![0u8; (new_w * new_h * 4) as usize];
            for ny in 0..new_h {
                for nx in 0..new_w {
                    let dst_idx = ((ny * new_w + nx) * 4) as usize;
                    let px = if bilinear {
                        let gx = (nx as f32 + 0.5) * scale_x - 0.5;
                        let gy = (ny as f32 + 0.5) * scale_y - 0.5;
                        let x0 = (gx.floor() as i32).clamp(0, old_w as i32 - 1) as u32;
                        let y0 = (gy.floor() as i32).clamp(0, old_h as i32 - 1) as u32;
                        let x1 = (x0 + 1).min(old_w - 1);
                        let y1 = (y0 + 1).min(old_h - 1);

                        let fx = (gx - gx.floor()).clamp(0.0, 1.0);
                        let fy = (gy - gy.floor()).clamp(0.0, 1.0);

                        let p00 = layer.get_pixel(x0, y0).unwrap_or([0, 0, 0, 0]);
                        let p10 = layer.get_pixel(x1, y0).unwrap_or([0, 0, 0, 0]);
                        let p01 = layer.get_pixel(x0, y1).unwrap_or([0, 0, 0, 0]);
                        let p11 = layer.get_pixel(x1, y1).unwrap_or([0, 0, 0, 0]);

                        let mut res = [0u8; 4];
                        for c in 0..4 {
                            let top = (p00[c] as f32) * (1.0 - fx) + (p10[c] as f32) * fx;
                            let bot = (p01[c] as f32) * (1.0 - fx) + (p11[c] as f32) * fx;
                            res[c] = (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
                        }
                        res
                    } else {
                        let ox = ((nx as f32 * scale_x).floor() as u32).min(old_w - 1);
                        let oy = ((ny as f32 * scale_y).floor() as u32).min(old_h - 1);
                        layer.get_pixel(ox, oy).unwrap_or([0, 0, 0, 0])
                    };
                    new_pixels[dst_idx..dst_idx + 4].copy_from_slice(&px);
                }
            }
            layer.width = new_w;
            layer.height = new_h;
            layer.set_pixels(new_pixels);
        }

        self.width = new_w;
        self.height = new_h;
    }

    /// Composite visible layers into a flat RGBA8 pixel buffer
    pub fn composite_layers(&self, include_background: bool) -> Vec<u8> {
        let mut out = vec![0u8; (self.width * self.height * 4) as usize];
        self.composite_layers_into(&mut out, include_background);
        out
    }

    /// Composite visible layers directly into an existing buffer without allocations
    pub fn composite_layers_into(&self, out: &mut [u8], include_background: bool) {
        let total_bytes = (self.width * self.height * 4) as usize;
        if out.len() < total_bytes {
            return;
        }

        if include_background && !self.is_transparent {
            let bg = self.background_color().to_rgba8();
            for chunk in out[..total_bytes].chunks_exact_mut(4) {
                chunk.copy_from_slice(&bg);
            }
        } else {
            out[..total_bytes].fill(0);
        }

        // Format boundary. Until 16-bit documents exist (AUR v3), a 16-bit layer is
        // converted **once per composite**, not per row or per pixel, and the compositor
        // then runs its existing 8-bit path. This is the seam where per-format dispatch
        // will live; keeping the conversion here means no pixel loop ever sees a temporary.
        let staged: Vec<Option<Vec<u8>>> = self
            .layers
            .iter()
            .map(|layer| match layer.buffer() {
                LayerBuffer::Srgb8(_) => None,
                LayerBuffer::Srgb16(channels) => Some(
                    channels
                        .iter()
                        .map(|&channel| crate::pixel::srgb16_to_srgb8(channel))
                        .collect(),
                ),
            })
            .collect();

        for (layer_idx, layer) in self.layers.iter().enumerate() {
            if layer.is_group() || !self.is_layer_effective_visible(layer.id) {
                continue;
            }

            let prev_layer = if layer.clipping_mask && layer_idx > 0 {
                let base = &self.layers[layer_idx - 1];
                if self.is_layer_effective_visible(base.id) {
                    Some(base)
                } else {
                    // Base layer is hidden; skip this clipping layer entirely
                    continue;
                }
            } else {
                None
            };

            let eff_opacity = self.effective_opacity(layer.id);
            if eff_opacity <= 0.001 {
                continue;
            }

            // ── Non-Destructive Adjustment Layer Pass ──
            if layer.is_adjustment() {
                if let Some(adj_cfg) = &layer.adjustment {
                    let adj_type = &adj_cfg.adjustment_type;
                    for y in 0..self.height {
                        for x in 0..self.width {
                            let dst_idx = ((y * self.width + x) * 4) as usize;
                            let dr = out[dst_idx];
                            let dg = out[dst_idx + 1];
                            let db = out[dst_idx + 2];
                            let da = out[dst_idx + 3];

                            if da == 0 {
                                continue;
                            }

                            let mut opacity = eff_opacity;
                            if let Some(m) = layer.mask.as_ref().filter(|m| m.enabled) {
                                opacity *= m.alpha_at(x, y) as f32 / 255.0;
                            }
                            if let Some(base) = prev_layer {
                                let bx = x as i32 - base.offset_x;
                                let by = y as i32 - base.offset_y;
                                let base_a = if bx >= 0 && bx < self.width as i32 && by >= 0 && by < self.height as i32 {
                                    base.get_pixel(bx as u32, by as u32).map(|p| p[3] as f32 / 255.0).unwrap_or(0.0)
                                } else {
                                    0.0
                                };
                                opacity *= base_a * self.effective_opacity(base.id);
                            }

                            if opacity <= 0.001 {
                                continue;
                            }

                            let (ar, ag, ab) = adj_type.apply_to_rgb(dr, dg, db);
                            let orig_px = [dr, dg, db, da];
                            let adj_px = [ar, ag, ab, da];
                            let blended = layer.blend_mode.composite_pixel(orig_px, adj_px, opacity);
                            out[dst_idx] = blended[0];
                            out[dst_idx + 1] = blended[1];
                            out[dst_idx + 2] = blended[2];
                            out[dst_idx + 3] = blended[3];
                        }
                    }
                }
                continue;
            }

            // ── Fast path for Normal blend mode with 0 offset and no clipping ──
            if layer.blend_mode == BlendMode::Normal && layer.offset_x == 0 && layer.offset_y == 0 && prev_layer.is_none() && layer.as_srgb8_exact().len() >= total_bytes {
                let alpha_mul = (eff_opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
                // A mask scales the layer's alpha per pixel; a mismatched mask is ignored
                // rather than silently misaligned.
                let mask = layer
                    .mask
                    .as_ref()
                    .filter(|m| m.enabled && m.pixels.len() >= (self.width as usize) * (self.height as usize));
                for (i, (dst, src)) in out[..total_bytes]
                    .chunks_exact_mut(4)
                    .zip(layer.as_srgb8_exact()[..total_bytes].chunks_exact(4))
                    .enumerate()
                {
                    let mut sa = (src[3] as u32 * alpha_mul) / 255;
                    if let Some(m) = mask {
                        sa = sa * m.pixels[i] as u32 / 255;
                    }
                    if sa == 0 {
                        continue;
                    }
                    let da = dst[3] as u32;
                    if da == 0 {
                        dst[0] = src[0];
                        dst[1] = src[1];
                        dst[2] = src[2];
                        dst[3] = sa as u8;
                    } else if sa == 255 {
                        dst[0] = src[0];
                        dst[1] = src[1];
                        dst[2] = src[2];
                        dst[3] = 255;
                    } else {
                        let inv_sa = 255 - sa;
                        let out_a = sa + (da * inv_sa + 127) / 255;
                        if out_a > 0 {
                            let r = (src[0] as u32 * sa * 255 + dst[0] as u32 * da * inv_sa) / (out_a * 255);
                            let g = (src[1] as u32 * sa * 255 + dst[1] as u32 * da * inv_sa) / (out_a * 255);
                            let b = (src[2] as u32 * sa * 255 + dst[2] as u32 * da * inv_sa) / (out_a * 255);
                            dst[0] = r.min(255) as u8;
                            dst[1] = g.min(255) as u8;
                            dst[2] = b.min(255) as u8;
                            dst[3] = out_a.min(255) as u8;
                        }
                    }
                }
                continue;
            }

            // ?? General path: offsets, blending, clipping, masks ??
            //
            // Two things dominate here and both are avoidable work, not maths:
            //   * the loop used to visit every pixel of the canvas even when the
            //     layer was offset, or fully transparent there; and
            //   * `get_pixel` did four bounds checks and an Option per pixel.
            // The blend itself is untouched (still `composite_pixel`), so output is
            // bit-identical to the straightforward implementation - see the
            // equivalence tests in `tests.rs`.
            let mask = layer
                .mask
                .as_ref()
                .filter(|m| m.enabled)
                .map(|m| m.pixels.as_slice());
            let layer_pixels: &[u8] = match &staged[layer_idx] {
                Some(converted) => converted,
                None => layer.as_srgb8_exact(),
            };
            let base_index = layer_idx.checked_sub(1);
            let base = prev_layer.map(|b| ClipBase {
                pixels: match base_index.and_then(|index| staged.get(index)).and_then(|s| s.as_deref()) {
                    Some(converted) => converted,
                    None => b.as_srgb8_exact(),
                },
                offset_x: b.offset_x,
                offset_y: b.offset_y,
                width: b.width,
                height: b.height,
                indexable: b.is_raster() || b.is_text(),
                opacity: self.effective_opacity(b.id),
            });
            let (w, h) = (self.width, self.height);

            // Resolve the mode once, outside the pixel loop: every arm runs the same loop
            // monomorphised for a single mode, so the blend dispatch disappears from the
            // hot path while the maths stays single-sourced in `composite_pixel`.
            macro_rules! run {
                ($compose:path) => {
                    compose_layer(out, layer, w, h, eff_opacity, mask, base, layer_pixels, $compose)
                };
            }
            match layer.blend_mode {
                BlendMode::Normal => run!(crate::blend::compose_normal),
                BlendMode::Multiply => run!(crate::blend::compose_multiply),
                BlendMode::Screen => run!(crate::blend::compose_screen),
                BlendMode::Overlay => run!(crate::blend::compose_overlay),
                BlendMode::Darken => run!(crate::blend::compose_darken),
                BlendMode::Lighten => run!(crate::blend::compose_lighten),
                BlendMode::ColorDodge => run!(crate::blend::compose_color_dodge),
                BlendMode::ColorBurn => run!(crate::blend::compose_color_burn),
                BlendMode::HardLight => run!(crate::blend::compose_hard_light),
                BlendMode::SoftLight => run!(crate::blend::compose_soft_light),
                BlendMode::Difference => run!(crate::blend::compose_difference),
                BlendMode::Exclusion => run!(crate::blend::compose_exclusion),
                BlendMode::Luminosity => run!(crate::blend::compose_luminosity),
            }
        }
    }
}

/// Read-only view of the layer a clipping layer is clipped to.
#[derive(Clone, Copy)]
struct ClipBase<'a, C> {
    pixels: &'a [C],
    offset_x: i32,
    offset_y: i32,
    width: u32,
    height: u32,
    /// `Layer::get_pixel` only answers for raster and text layers; for anything else it
    /// reports "no pixel", which the reference implementation treats as alpha 0.
    indexable: bool,
    /// Cached `effective_opacity` of the base layer.
    opacity: f32,
}

/// Composites one layer, honouring its offset, mask, clip base and blend mode.
///
/// The loop body performs exactly the operations the reference implementation does, in
/// the same order - only the dead work is removed. See
/// `tests::general_compositing_is_bit_identical_to_the_reference`.
/// Rows below this many pixels are composed on the calling thread: a scoped thread costs
/// tens of microseconds, which is more than a small layer takes to composite.
const PARALLEL_MIN_PIXELS: u32 = 128 * 128;

/// Upper bound on the threads a single layer is split across. More threads than this stop
/// helping (the work is memory-bound) and only add scheduling noise.
const PARALLEL_MAX_THREADS: usize = 8;

/// How many threads a layer of `pixels` pixels should be split across.
pub(crate) fn parallel_degree(pixels: u32) -> usize {
    if pixels < PARALLEL_MIN_PIXELS {
        return 1;
    }
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(PARALLEL_MAX_THREADS)
        .max(1)
}

/// Composites one layer, splitting its rows across threads when it is worth doing.
///
/// Compositing order across layers is untouched: this is called once per layer, in order,
/// and joins before returning. Within the layer each thread owns a fixed, contiguous band
/// of rows and writes only to its own slice of `out`, so the result cannot depend on
/// scheduling - see `tests::parallel_compositing_matches_the_serial_reference`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compose_layer<C: Channel, F>(
    out: &mut [C],
    layer: &Layer,
    canvas_w: u32,
    canvas_h: u32,
    layer_opacity: f32,
    mask: Option<&[u8]>,
    base: Option<ClipBase<'_, C>>,
    src_pixels: &[C],
    compose: F,
) where
    F: Fn([f32; 4], [f32; 4], f32) -> [f32; 4] + Sync,
{
    let threads = parallel_degree(canvas_w.saturating_mul(canvas_h));
    if threads <= 1 {
        compose_rows(
            out,
            0,
            canvas_h,
            layer,
            canvas_w,
            canvas_h,
            layer_opacity,
            mask,
            base,
            src_pixels,
            &compose,
        );
        return;
    }

    let bytes_per_row = canvas_w as usize * 4;
    let rows_per_thread = canvas_h.div_ceil(threads as u32);
    let compose = &compose;
    std::thread::scope(|scope| {
        for (index, slice) in out
            .chunks_mut(rows_per_thread as usize * bytes_per_row)
            .enumerate()
        {
            let y_start = index as u32 * rows_per_thread;
            let y_end = y_start + (slice.len() / bytes_per_row) as u32;
            scope.spawn(move || {
                compose_rows(
                    slice,
                    y_start,
                    y_end,
                    layer,
                    canvas_w,
                    canvas_h,
                    layer_opacity,
                    mask,
                    base,
                    src_pixels,
                    compose,
                );
            });
        }
    });
}

/// Composites rows `y_start..y_end` of one layer into `out`.
///
/// `out` holds only those rows (`(y_end - y_start) * canvas_w * 4` bytes); every other
/// argument is shared and read-only. The loop body performs exactly the operations the
/// reference implementation does, in the same order - only dead work is removed. See
/// `tests::general_compositing_is_bit_identical_to_the_reference`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub(crate) fn compose_rows<C: Channel, F>(
    out: &mut [C],
    y_start: u32,
    y_end: u32,
    layer: &Layer,
    canvas_w: u32,
    canvas_h: u32,
    layer_opacity: f32,
    mask: Option<&[u8]>,
    base: Option<ClipBase<'_, C>>,
    src_pixels: &[C],
    compose: &F,
) where
    F: Fn([f32; 4], [f32; 4], f32) -> [f32; 4],
{
    // The reference reads its source through `Layer::get_pixel`, which answers only for
    // raster and text layers; for anything else every source is transparent and the layer
    // writes nothing at all.
    if !layer.is_raster() && !layer.is_text() {
        return;
    }

    let stride = canvas_w as usize;
    let src_stride = layer.width as usize;
    let x_start = layer.offset_x.max(0);
    let x_end = (canvas_w as i32 + layer.offset_x).min(canvas_w as i32);
    if x_start >= x_end {
        return;
    }
    // `get_pixel` clamps to the layer's own size as well as the canvas.
    let src_lim_x = (canvas_w as i32).min(layer.width as i32);
    let src_lim_y = (canvas_h as i32).min(layer.height as i32);

    for y in y_start..y_end {
        let src_y = y as i32 - layer.offset_y;
        if src_y < 0 || src_y >= src_lim_y {
            continue;
        }
        // Resolve the part of this row the layer can actually contribute: contiguous, so
        // the inner loop indexes the buffer directly instead of calling `get_pixel` and
        // checking bounds for every pixel. A pixel outside it is transparent, which is a
        // no-op, so skipping it is exact.
        let row_start = (src_y as usize) * src_stride * 4;
        let available = src_pixels.len().saturating_sub(row_start) / 4;
        let lim = src_lim_x.min(available as i32);
        let lo = x_start.max(layer.offset_x);
        let hi = x_end.min(layer.offset_x.saturating_add(lim));
        if lo >= hi {
            continue;
        }
        let dst_row = (y - y_start) as usize * stride;

        // The clip base contributes inside a contiguous span of this row too; outside it
        // its alpha is 0, which zeroes this layer's opacity there.
        let mut base_span: Option<(&[C], i32, i32, i32, f32)> = None;
        if let Some(b) = base {
            let b_lim_x = (canvas_w as i32).min(b.width as i32);
            let b_lim_y = (canvas_h as i32).min(b.height as i32);
            let by = y as i32 - b.offset_y;
            if !b.indexable || by < 0 || by >= b_lim_y {
                continue;
            }
            let b_row_start = (by as usize) * (b.width as usize) * 4;
            let b_available = b.pixels.len().saturating_sub(b_row_start) / 4;
            let b_lim = b_lim_x.min(b_available as i32);
            let b_lo = x_start.max(b.offset_x);
            let b_hi = x_end.min(b.offset_x.saturating_add(b_lim));
            if b_lo >= b_hi {
                continue;
            }
            base_span = Some((&b.pixels[b_row_start..], b_lo, b_hi, b.offset_x, b.opacity));
        }

        for x in lo..hi {
            let si = row_start + ((x - layer.offset_x) as usize) * 4;
            // Compositing a fully transparent source is a no-op by definition; skipping
            // it before the float work is exact and is the biggest win on sparse art.
            if src_pixels[si + 3] == C::ZERO {
                continue;
            }
            let dst_idx = (dst_row + x as usize) * 4;
            if dst_idx + 3 >= out.len() {
                break;
            }

            let mut opacity = layer_opacity;
            if let Some(m) = mask {
                if let Some(&mv) = m.get(y as usize * stride + x as usize) {
                    opacity *= mv as f32 / 255.0;
                }
            }
            if let Some((row, b_lo, b_hi, b_off, base_op)) = base_span {
                if x < b_lo || x >= b_hi {
                    continue;
                }
                let a = row[((x - b_off) as usize) * 4 + 3];
                opacity *= a.to_f32() * base_op;
            }
            if opacity <= 0.001 {
                continue;
            }

            // The two conversion boundaries: storage to working on the way in, working to
            // storage on the way out. Everything between is the shared maths.
            let dst_px = [
                out[dst_idx].to_f32(),
                out[dst_idx + 1].to_f32(),
                out[dst_idx + 2].to_f32(),
                out[dst_idx + 3].to_f32(),
            ];
            let src_px = [
                src_pixels[si].to_f32(),
                src_pixels[si + 1].to_f32(),
                src_pixels[si + 2].to_f32(),
                src_pixels[si + 3].to_f32(),
            ];
            let blended = compose(dst_px, src_px, opacity);
            out[dst_idx] = C::from_f32(blended[0]);
            out[dst_idx + 1] = C::from_f32(blended[1]);
            out[dst_idx + 2] = C::from_f32(blended[2]);
            out[dst_idx + 3] = C::from_f32(blended[3]);
        }
    }
}

/// Composites a document into `Srgb16` storage.
///
/// This is the gate-1 harness for the 16-bit plan (`docs/16bit-plan.md`): it exists so the
/// oracle can be run against promoted fixtures and compared, after downconversion, with
/// today's 8-bit output. It is not wired into the application, and no `LayerBuffer` exists
/// yet - that is gate 3.
///
/// There is **no 16-bit fast path** (a deliberate deferral), so every layer takes the general
/// path. A comparison is therefore only meaningful against 8-bit fixtures that also take the
/// general path - see the gate-1 tests.
/// A document's layers promoted to `Srgb16` storage, owned so a measurement can exclude the
/// promotion from the thing being measured.
///
/// A real 16-bit document holds 16-bit buffers; promoting on every composite would be
/// measuring the conversion, not the compositing.
pub(crate) struct PromotedLayers(Vec<Vec<u16>>);

#[allow(dead_code)]
pub(crate) fn promote_layers(doc: &Document) -> PromotedLayers {
    PromotedLayers(
        doc.layers
            .iter()
            .map(|layer| {
                layer
                    .as_srgb8_exact()
                    .iter()
                    .map(|&v| crate::pixel::srgb8_to_srgb16(v))
                    .collect()
            })
            .collect(),
    )
}

#[allow(dead_code)]
pub(crate) fn compose_u16(doc: &Document, include_background: bool) -> Vec<u16> {
    let promoted = promote_layers(doc);
    compose_u16_prepared(doc, &promoted, include_background)
}

#[allow(dead_code)]
pub(crate) fn compose_u16_prepared(
    doc: &Document,
    promoted: &PromotedLayers,
    include_background: bool,
) -> Vec<u16> {
    let (w, h) = (doc.width, doc.height);
    let mut out = vec![0u16; (w as usize) * (h as usize) * 4];

    if include_background && !doc.is_transparent {
        let bg = doc.background_color().to_rgba8();
        let promoted = [
            crate::pixel::srgb8_to_srgb16(bg[0]),
            crate::pixel::srgb8_to_srgb16(bg[1]),
            crate::pixel::srgb8_to_srgb16(bg[2]),
            crate::pixel::srgb8_to_srgb16(bg[3]),
        ];
        for chunk in out.chunks_exact_mut(4) {
            chunk.copy_from_slice(&promoted);
        }
    }

    for (index, layer) in doc.layers.iter().enumerate() {
        if layer.is_group()
            || layer.is_adjustment()
            || !doc.is_layer_effective_visible(layer.id)
        {
            continue;
        }
        let opacity = doc.effective_opacity(layer.id);
        if opacity <= 0.001 {
            continue;
        }
        let base = if layer.clipping_mask && index > 0 {
            let b = &doc.layers[index - 1];
            if !doc.is_layer_effective_visible(b.id) {
                continue;
            }
            Some(ClipBase {
                pixels: &promoted.0[index - 1],
                offset_x: b.offset_x,
                offset_y: b.offset_y,
                width: b.width,
                height: b.height,
                indexable: b.is_raster() || b.is_text(),
                opacity: doc.effective_opacity(b.id),
            })
        } else {
            None
        };
        let mask = layer
            .mask
            .as_ref()
            .filter(|m| m.enabled)
            .map(|m| m.pixels.as_slice());

        macro_rules! run {
            ($compose:path) => {
                compose_layer(
                    &mut out, layer, w, h, opacity, mask, base, &promoted.0[index], $compose,
                )
            };
        }
        match layer.blend_mode {
            BlendMode::Normal => run!(crate::blend::compose_normal),
            BlendMode::Multiply => run!(crate::blend::compose_multiply),
            BlendMode::Screen => run!(crate::blend::compose_screen),
            BlendMode::Overlay => run!(crate::blend::compose_overlay),
            BlendMode::Darken => run!(crate::blend::compose_darken),
            BlendMode::Lighten => run!(crate::blend::compose_lighten),
            BlendMode::ColorDodge => run!(crate::blend::compose_color_dodge),
            BlendMode::ColorBurn => run!(crate::blend::compose_color_burn),
            BlendMode::HardLight => run!(crate::blend::compose_hard_light),
            BlendMode::SoftLight => run!(crate::blend::compose_soft_light),
            BlendMode::Difference => run!(crate::blend::compose_difference),
            BlendMode::Exclusion => run!(crate::blend::compose_exclusion),
            BlendMode::Luminosity => run!(crate::blend::compose_luminosity),
        }
    }
    out
}
