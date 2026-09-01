use crate::document::Document;
use crate::layer::{Layer, LayerId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationFrame {
    pub id: u64,
    pub name: String,
    pub layers: Vec<Layer>,
    pub active_layer_id: LayerId,
    pub next_layer_id: u64,
    pub duration_ms: u32,
}

impl AnimationFrame {
    pub fn new(id: u64, name: impl Into<String>, width: u32, height: u32) -> Self {
        let first_id = 1;
        let base_layer = Layer::new(first_id, "Layer 1", width, height);
        Self {
            id,
            name: name.into(),
            layers: vec![base_layer],
            active_layer_id: first_id,
            next_layer_id: 2,
            duration_ms: 0,
        }
    }

    pub fn from_layers(
        id: u64,
        name: impl Into<String>,
        layers: Vec<Layer>,
        active_layer_id: LayerId,
        next_layer_id: u64,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            layers,
            active_layer_id,
            next_layer_id,
            duration_ms: 0,
        }
    }

    pub fn duplicate(&self, new_id: u64) -> Self {
        let mut cloned_layers = Vec::with_capacity(self.layers.len());
        for (i, layer) in self.layers.iter().enumerate() {
            let mut dup = layer.clone();
            dup.id = (i + 1) as LayerId;
            cloned_layers.push(dup);
        }
        Self {
            id: new_id,
            name: format!("{} Copy", self.name),
            layers: cloned_layers,
            active_layer_id: self.active_layer_id,
            next_layer_id: self.next_layer_id,
            duration_ms: self.duration_ms,
        }
    }

    pub fn composite_layers(&self, width: u32, height: u32, include_background: bool, bg_val: u8) -> Vec<u8> {
        let mut out = vec![0u8; (width * height * 4) as usize];
        if include_background {
            let bg_rgb = [bg_val, bg_val, bg_val, 255];
            for px in out.chunks_exact_mut(4) {
                px.copy_from_slice(&bg_rgb);
            }
        }

        for layer in &self.layers {
            if !layer.visible || layer.opacity <= 0.001 {
                continue;
            }
            let layer_alpha = layer.opacity.clamp(0.0, 1.0);
            for y in 0..height {
                let dst_row = (y * width * 4) as usize;
                let src_y = y as i32 - layer.offset_y;
                if src_y < 0 || src_y >= layer.height as i32 {
                    continue;
                }
                let src_row = (src_y as u32 * layer.width * 4) as usize;

                for x in 0..width {
                    let dst_idx = dst_row + (x * 4) as usize;
                    let src_x = x as i32 - layer.offset_x;
                    if src_x < 0 || src_x >= layer.width as i32 {
                        continue;
                    }
                    let src_idx = src_row + (src_x as u32 * 4) as usize;
                    if src_idx + 3 >= layer.pixels.len() || dst_idx + 3 >= out.len() {
                        continue;
                    }

                    let src = [
                        layer.pixels[src_idx],
                        layer.pixels[src_idx + 1],
                        layer.pixels[src_idx + 2],
                        layer.pixels[src_idx + 3],
                    ];
                    let dst = [out[dst_idx], out[dst_idx + 1], out[dst_idx + 2], out[dst_idx + 3]];
                    let blended = layer.blend_mode.composite_pixel(dst, src, layer_alpha);
                    out[dst_idx..dst_idx + 4].copy_from_slice(&blended);
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationTimeline {
    pub is_enabled: bool,
    pub frames: Vec<AnimationFrame>,
    pub current_frame_idx: usize,
    pub fps: u32,
    pub is_playing: bool,
    pub loop_playback: bool,
    pub next_frame_id: u64,

    // Onion Skinning Configuration
    pub onion_skin_enabled: bool,
    pub onion_skin_prev_count: usize,
    pub onion_skin_next_count: usize,
    pub onion_skin_opacity: f32,
}

impl Default for AnimationTimeline {
    fn default() -> Self {
        Self {
            is_enabled: false,
            frames: Vec::new(),
            current_frame_idx: 0,
            fps: 12,
            is_playing: false,
            loop_playback: true,
            next_frame_id: 1,

            onion_skin_enabled: true,
            onion_skin_prev_count: 2,
            onion_skin_next_count: 1,
            onion_skin_opacity: 0.45,
        }
    }
}

impl AnimationTimeline {
    pub fn new(width: u32, height: u32) -> Self {
        let first_frame = AnimationFrame::new(1, "Frame 1", width, height);
        Self {
            is_enabled: false,
            frames: vec![first_frame],
            current_frame_idx: 0,
            fps: 12,
            is_playing: false,
            loop_playback: true,
            next_frame_id: 2,

            onion_skin_enabled: true,
            onion_skin_prev_count: 2,
            onion_skin_next_count: 1,
            onion_skin_opacity: 0.45,
        }
    }

    pub fn from_document(doc: &Document) -> Self {
        let first_frame = AnimationFrame::from_layers(
            1,
            "Frame 1",
            doc.layers.clone(),
            doc.active_layer_id,
            doc.next_layer_id,
        );
        Self {
            is_enabled: false,
            frames: vec![first_frame],
            current_frame_idx: 0,
            fps: 12,
            is_playing: false,
            loop_playback: true,
            next_frame_id: 2,

            onion_skin_enabled: true,
            onion_skin_prev_count: 2,
            onion_skin_next_count: 1,
            onion_skin_opacity: 0.45,
        }
    }

    pub fn add_frame(&mut self, width: u32, height: u32) -> usize {
        let id = self.next_frame_id;
        self.next_frame_id += 1;
        let name = format!("Frame {}", self.frames.len() + 1);
        let new_frame = AnimationFrame::new(id, name, width, height);
        let insert_idx = if self.frames.is_empty() {
            0
        } else {
            self.current_frame_idx + 1
        };
        self.frames.insert(insert_idx, new_frame);
        self.current_frame_idx = insert_idx;
        insert_idx
    }

    pub fn duplicate_current_frame(&mut self) -> usize {
        if self.frames.is_empty() {
            return 0;
        }
        let id = self.next_frame_id;
        self.next_frame_id += 1;
        let dup = self.frames[self.current_frame_idx].duplicate(id);
        let insert_idx = self.current_frame_idx + 1;
        self.frames.insert(insert_idx, dup);
        self.current_frame_idx = insert_idx;
        insert_idx
    }

    pub fn delete_current_frame(&mut self) -> bool {
        if self.frames.len() <= 1 {
            return false;
        }
        self.frames.remove(self.current_frame_idx);
        if self.current_frame_idx >= self.frames.len() {
            self.current_frame_idx = self.frames.len() - 1;
        }
        true
    }

    pub fn move_frame(&mut self, from: usize, to: usize) {
        if from < self.frames.len() && to < self.frames.len() && from != to {
            let item = self.frames.remove(from);
            self.frames.insert(to, item);
            self.current_frame_idx = to;
        }
    }

    pub fn current_frame(&self) -> Option<&AnimationFrame> {
        self.frames.get(self.current_frame_idx)
    }

    pub fn current_frame_mut(&mut self) -> Option<&mut AnimationFrame> {
        self.frames.get_mut(self.current_frame_idx)
    }

    pub fn step_next_frame(&mut self) {
        if self.frames.is_empty() {
            return;
        }
        if self.current_frame_idx + 1 < self.frames.len() {
            self.current_frame_idx += 1;
        } else if self.loop_playback {
            self.current_frame_idx = 0;
        }
    }

    pub fn step_prev_frame(&mut self) {
        if self.frames.is_empty() {
            return;
        }
        if self.current_frame_idx > 0 {
            self.current_frame_idx -= 1;
        } else if self.loop_playback {
            self.current_frame_idx = self.frames.len() - 1;
        }
    }

    pub fn sync_from_document(&mut self, doc: &Document) {
        if let Some(frame) = self.frames.get_mut(self.current_frame_idx) {
            frame.layers = doc.layers.clone();
            frame.active_layer_id = doc.active_layer_id;
            frame.next_layer_id = doc.next_layer_id;
        }
    }

    pub fn sync_to_document(&self, doc: &mut Document) {
        if let Some(frame) = self.frames.get(self.current_frame_idx) {
            doc.layers = frame.layers.clone();
            doc.active_layer_id = frame.active_layer_id;
            doc.next_layer_id = frame.next_layer_id;
        }
    }
}
