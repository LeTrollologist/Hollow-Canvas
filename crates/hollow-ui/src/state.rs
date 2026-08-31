use glam::Vec2;
use hollow_core::brush::BrushSettings;
use hollow_core::color::{Color, DEFAULT_PALETTE};
use hollow_core::document::Document;
use hollow_core::history::HistoryStack;
use hollow_core::selection::SelectionMask;
use hollow_core::symmetry::SymmetryConfig;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasPreset {
    pub category: &'static str,
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
}

impl CanvasPreset {
    pub const ALL: &'static [Self] = &[
        // Digital Art
        Self { category: "Digital Art", name: "1:1 Square (1024 × 1024)", width: 1024, height: 1024 },
        Self { category: "Digital Art", name: "2K Square (2048 × 2048)", width: 2048, height: 2048 },
        Self { category: "Digital Art", name: "4K Master (4096 × 4096)", width: 4096, height: 4096 },
        // Screen & Display
        Self { category: "Screen & Video", name: "1080p FHD (1920 × 1080)", width: 1920, height: 1080 },
        Self { category: "Screen & Video", name: "1440p QHD (2560 × 1440)", width: 2560, height: 1440 },
        Self { category: "Screen & Video", name: "4K UHD (3840 × 2160)", width: 3840, height: 2160 },
        Self { category: "Screen & Video", name: "Ultrawide (3440 × 1440)", width: 3440, height: 1440 },
        // Social Media
        Self { category: "Social Media", name: "Twitter / X Banner (1500 × 500)", width: 1500, height: 500 },
        Self { category: "Social Media", name: "YouTube Thumb (1280 × 720)", width: 1280, height: 720 },
        Self { category: "Social Media", name: "Mobile Story (1080 × 1920)", width: 1080, height: 1920 },
        Self { category: "Social Media", name: "Instagram Portrait (1080 × 1350)", width: 1080, height: 1350 },
        // Print Documents
        Self { category: "Print", name: "A4 @ 300 DPI (2480 × 3508)", width: 2480, height: 3508 },
        Self { category: "Print", name: "A5 @ 300 DPI (1748 × 2480)", width: 1748, height: 2480 },
        Self { category: "Print", name: "Photo 4×6 (1200 × 1800)", width: 1200, height: 1800 },
        // Pixel Art
        Self { category: "Pixel Art", name: "Sprite (32 × 32)", width: 32, height: 32 },
        Self { category: "Pixel Art", name: "Icon (64 × 64)", width: 64, height: 64 },
        Self { category: "Pixel Art", name: "Scene (128 × 128)", width: 128, height: 128 },
        Self { category: "Pixel Art", name: "Retro (256 × 256)", width: 256, height: 256 },
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingFileAction {
    SaveProject,
    OpenProject,
    ExportPng,
    OpenReferenceImage,
    NewCanvas(u32, u32, u8), // w, h, bg_mode (0: dark, 1: white, 2: transparent)
}

pub struct AppState {
    pub document: Document,
    pub history: HistoryStack,
    pub brush: BrushSettings,
    pub symmetry: SymmetryConfig,
    pub selection: Option<SelectionMask>,
    pub color_slot_is_secondary: bool,
    pub color_history: Vec<Color>,
    pub pan: Vec2,
    pub zoom: f32,
    pub status_message: String,
    pub cursor_canvas_pos: Vec2,
    pub show_help: bool,
    pub show_gallery: bool,

    // Overlay Toggles
    pub show_grid: bool,
    pub grid_size: u32,
    pub grid_opacity: f32,
    pub show_rulers: bool,

    // New Canvas Modal
    pub show_new_canvas_dialog: bool,
    pub new_canvas_preset_idx: usize,
    pub new_canvas_w: u32,
    pub new_canvas_h: u32,
    pub new_canvas_lock_aspect: bool,
    pub new_canvas_aspect_ratio: f32,
    pub new_canvas_bg_mode: u8, // 0: Dark, 1: Pure White, 2: Transparent

    // Resize / Scale Modal
    pub show_resize_canvas_dialog: bool,
    pub resize_canvas_w: u32,
    pub resize_canvas_h: u32,
    pub resize_scale_mode: bool, // false: crop/extend, true: resample scale
    pub resize_bilinear: bool,
    pub resize_anchor_center: bool,

    // Wand Tool Settings
    pub wand_tolerance: u8,
    pub wand_contiguous: bool,
    pub wand_sample_all_layers: bool,

    // Reference Dock & Backlight
    pub reference_image: Option<(u32, u32, Vec<u8>)>,
    pub ref_texture: Option<egui::TextureHandle>,
    pub show_ref_window: bool,
    pub ref_zoom: f32,
    pub ref_pan: Vec2,
    pub ref_backlight: bool,
    pub ref_backlight_mode: u8, // 0: Dark, 1: Lightbox Pure White, 2: Checkerboard

    // Painting state
    pub is_painting: bool,
    pub last_paint_pos: Option<Vec2>,
    pub stroke_pixels_backup: Option<Vec<u8>>,
    pub is_space_panning: bool,
    pub pan_drag_start: Option<Vec2>,
    pub pending_file_action: Option<PendingFileAction>,
    pub drag_start_canvas_pos: Option<Vec2>,
    pub polygon_points: Vec<Vec2>,
    pub crop_box: Option<(Vec2, Vec2)>,
}

impl AppState {
    pub fn new(width: u32, height: u32) -> Self {
        let doc = Document::new(width, height);
        let mut color_history = Vec::new();
        for &hex in DEFAULT_PALETTE.iter().take(12) {
            if let Some(c) = Color::from_hex(hex) {
                color_history.push(c);
            }
        }

        Self {
            document: doc,
            history: HistoryStack::new(50),
            brush: BrushSettings::default(),
            symmetry: SymmetryConfig::default(),
            selection: None,
            color_slot_is_secondary: false,
            color_history,
            pan: Vec2::ZERO,
            zoom: 1.0,
            status_message: "Ready".to_string(),
            cursor_canvas_pos: Vec2::ZERO,
            show_help: false,
            show_gallery: false,

            show_grid: false,
            grid_size: 32,
            grid_opacity: 0.25,
            show_rulers: true,

            show_new_canvas_dialog: false,
            new_canvas_preset_idx: 0,
            new_canvas_w: 1920,
            new_canvas_h: 1080,
            new_canvas_lock_aspect: false,
            new_canvas_aspect_ratio: 1920.0 / 1080.0,
            new_canvas_bg_mode: 0,

            show_resize_canvas_dialog: false,
            resize_canvas_w: width,
            resize_canvas_h: height,
            resize_scale_mode: false,
            resize_bilinear: true,
            resize_anchor_center: true,

            wand_tolerance: 24,
            wand_contiguous: true,
            wand_sample_all_layers: false,

            reference_image: None,
            ref_texture: None,
            show_ref_window: false,
            ref_zoom: 1.0,
            ref_pan: Vec2::ZERO,
            ref_backlight: false,
            ref_backlight_mode: 0,

            is_painting: false,
            last_paint_pos: None,
            stroke_pixels_backup: None,
            is_space_panning: false,
            pan_drag_start: None,
            pending_file_action: None,
            drag_start_canvas_pos: None,
            polygon_points: Vec::new(),
            crop_box: None,
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
    }

    pub fn reset_view(&mut self) {
        self.pan = Vec2::ZERO;
        self.zoom = 1.0;
        for layer in &mut self.document.layers {
            layer.offset_x = 0;
            layer.offset_y = 0;
        }
        self.set_status("View reset to center (100%)");
    }

    pub fn reset_view_centered(&mut self, win_w: f32, win_h: f32) {
        let left_panel_w = 200.0_f32;
        let right_panel_w = 230.0_f32;
        let top_panel_h = 40.0_f32;
        let bottom_panel_h = 24.0_f32;

        let avail_center_x = (left_panel_w + (win_w - right_panel_w)) * 0.5;
        let avail_center_y = (top_panel_h + (win_h - bottom_panel_h)) * 0.5;

        self.pan = Vec2::new(avail_center_x - win_w * 0.5, avail_center_y - win_h * 0.5);

        let avail_w = (win_w - left_panel_w - right_panel_w - 40.0).max(100.0);
        let avail_h = (win_h - top_panel_h - bottom_panel_h - 40.0).max(100.0);
        let fit_zoom = (avail_w / self.document.width as f32).min(avail_h / self.document.height as f32).min(1.0).max(0.1);
        self.zoom = fit_zoom;
        for layer in &mut self.document.layers {
            layer.offset_x = 0;
            layer.offset_y = 0;
        }
        self.set_status(format!("Canvas centered · Zoom: {}%", (self.zoom * 100.0).round() as u32));
    }

    pub fn swap_colors(&mut self) {
        let temp = self.brush.primary_color;
        self.brush.primary_color = self.brush.secondary_color;
        self.brush.secondary_color = temp;
    }

    pub fn from_document(doc: Document) -> Self {
        let mut s = Self::new(doc.width, doc.height);
        s.document = doc;
        s
    }

    pub fn screen_to_canvas(&self, screen_pos: Vec2, win_w: f32, win_h: f32) -> Vec2 {
        let center_x = win_w * 0.5 + self.pan.x;
        let center_y = win_h * 0.5 + self.pan.y;
        let local_x = (screen_pos.x - center_x) / self.zoom;
        let local_y = (screen_pos.y - center_y) / self.zoom;
        Vec2::new(
            local_x + self.document.width as f32 * 0.5,
            local_y + self.document.height as f32 * 0.5,
        )
    }

    pub fn canvas_to_screen(&self, canvas_pos: Vec2, win_w: f32, win_h: f32) -> Vec2 {
        let center_x = win_w * 0.5 + self.pan.x;
        let center_y = win_h * 0.5 + self.pan.y;
        let local_x = (canvas_pos.x - self.document.width as f32 * 0.5) * self.zoom;
        let local_y = (canvas_pos.y - self.document.height as f32 * 0.5) * self.zoom;
        Vec2::new(center_x + local_x, center_y + local_y)
    }

    pub fn theme_accent_color(&self) -> Color {
        self.document.theme.accent_color()
    }

    pub fn push_color_history(&mut self, color: Color) {
        self.color_history.retain(|&c| c.to_hex() != color.to_hex());
        self.color_history.insert(0, color);
        if self.color_history.len() > 14 {
            self.color_history.truncate(14);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(1280, 720)
    }
}
