use glam::Vec2;
use hollow_core::brush::BrushSettings;
use hollow_core::color::{Color, DEFAULT_PALETTE};
use hollow_core::document::Document;
use hollow_core::history::HistoryStack;
use hollow_core::selection::SelectionMask;
use hollow_core::symmetry::SymmetryConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionPreset {
    WindowSize,
    Fhd1080p,
    Square1080,
    Portrait1080x1350,
    Qhd1440p,
    Uhd4k,
}

impl ResolutionPreset {
    pub const ALL: &'static [Self] = &[
        Self::WindowSize,
        Self::Fhd1080p,
        Self::Square1080,
        Self::Portrait1080x1350,
        Self::Qhd1440p,
        Self::Uhd4k,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::WindowSize => "Window Size",
            Self::Fhd1080p => "1080p (1920×1080)",
            Self::Square1080 => "Square (1080×1080)",
            Self::Portrait1080x1350 => "Portrait (1080×1350)",
            Self::Qhd1440p => "1440p (2560×1440)",
            Self::Uhd4k => "4K (3840×2160)",
        }
    }

    pub fn dimensions(&self, window_w: u32, window_h: u32) -> (u32, u32) {
        match self {
            Self::WindowSize => (window_w.max(200), window_h.max(200)),
            Self::Fhd1080p => (1920, 1080),
            Self::Square1080 => (1080, 1080),
            Self::Portrait1080x1350 => (1080, 1350),
            Self::Qhd1440p => (2560, 1440),
            Self::Uhd4k => (3840, 2160),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingFileAction {
    SaveProject,
    OpenProject,
    ExportPng,
    OpenReferenceImage,
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
    pub resolution_preset: ResolutionPreset,
    pub is_painting: bool,
    pub last_paint_pos: Option<Vec2>,
    pub stroke_pixels_backup: Option<Vec<u8>>,
    pub is_space_panning: bool,
    pub pan_drag_start: Option<Vec2>,
    pub pending_file_action: Option<PendingFileAction>,
    pub drag_start_canvas_pos: Option<Vec2>,
    pub polygon_points: Vec<Vec2>,
    pub crop_box: Option<(Vec2, Vec2)>,
    pub reference_image: Option<(u32, u32, Vec<u8>)>,
    pub show_ref_window: bool,
    pub ref_zoom: f32,
    pub ref_pan: Vec2,
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
            resolution_preset: ResolutionPreset::WindowSize,
            is_painting: false,
            last_paint_pos: None,
            stroke_pixels_backup: None,
            is_space_panning: false,
            pan_drag_start: None,
            pending_file_action: None,
            drag_start_canvas_pos: None,
            polygon_points: Vec::new(),
            crop_box: None,
            reference_image: None,
            show_ref_window: false,
            ref_zoom: 1.0,
            ref_pan: Vec2::ZERO,
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
        match self.document.theme {
            hollow_core::color::ThemeMode::DeepMist => Color::new(0.66, 0.62, 0.85, 1.0),
            hollow_core::color::ThemeMode::Moonlit => Color::new(0.48, 0.69, 0.94, 1.0),
            hollow_core::color::ThemeMode::EmberGlow => Color::new(0.96, 0.64, 0.38, 1.0),
        }
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

