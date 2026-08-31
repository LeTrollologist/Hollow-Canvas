use crate::blend::BlendMode;
use serde::{Deserialize, Serialize};

pub type LayerId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub locked: bool,
    #[serde(default)]
    pub alpha_locked: bool,
    #[serde(default)]
    pub clipping_mask: bool,
    #[serde(default)]
    pub is_reference: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub offset_x: i32,
    pub offset_y: i32,
    #[serde(skip)]
    pub pixels: Vec<u8>, // RGBA8, len = width * height * 4
}

impl Layer {
    pub fn new(id: LayerId, name: impl Into<String>, width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize) * 4;
        Self {
            id,
            name: name.into(),
            width,
            height,
            visible: true,
            locked: false,
            alpha_locked: false,
            clipping_mask: false,
            is_reference: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            offset_x: 0,
            offset_y: 0,
            pixels: vec![0; size],
        }
    }

    pub fn from_pixels(
        id: LayerId,
        name: impl Into<String>,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            width,
            height,
            visible: true,
            locked: false,
            alpha_locked: false,
            clipping_mask: false,
            is_reference: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            offset_x: 0,
            offset_y: 0,
            pixels,
        }
    }

    #[inline]
    pub fn pixel_index(&self, x: u32, y: u32) -> Option<usize> {
        if x < self.width && y < self.height {
            Some(((y as usize) * (self.width as usize) + (x as usize)) * 4)
        } else {
            None
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let idx = self.pixel_index(x, y)?;
        Some([
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ])
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if let Some(idx) = self.pixel_index(x, y) {
            self.pixels[idx] = color[0];
            self.pixels[idx + 1] = color[1];
            self.pixels[idx + 2] = color[2];
            self.pixels[idx + 3] = color[3];
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    pub fn resize(&mut self, new_w: u32, new_h: u32) {
        if self.width == new_w && self.height == new_h {
            return;
        }
        let mut new_pixels = vec![0u8; (new_w as usize) * (new_h as usize) * 4];
        let copy_w = self.width.min(new_w);
        let copy_h = self.height.min(new_h);

        for y in 0..copy_h {
            let src_start = ((y as usize) * (self.width as usize)) * 4;
            let src_end = src_start + (copy_w as usize) * 4;
            let dst_start = ((y as usize) * (new_w as usize)) * 4;
            let dst_end = dst_start + (copy_w as usize) * 4;
            new_pixels[dst_start..dst_end].copy_from_slice(&self.pixels[src_start..src_end]);
        }

        self.width = new_w;
        self.height = new_h;
        self.pixels = new_pixels;
    }

    pub fn duplicate(&self, new_id: LayerId) -> Self {
        Self {
            id: new_id,
            name: format!("{} Copy", self.name),
            width: self.width,
            height: self.height,
            visible: self.visible,
            locked: false,
            alpha_locked: self.alpha_locked,
            clipping_mask: self.clipping_mask,
            is_reference: self.is_reference,
            opacity: self.opacity,
            blend_mode: self.blend_mode,
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            pixels: self.pixels.clone(),
        }
    }
}
