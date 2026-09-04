use glam::Vec2;
use hollow_core::blend::BlendMode;
use hollow_core::brush::BrushSettings;
use hollow_core::color::{Color, DEFAULT_PALETTE};
use hollow_core::document::Document;
use hollow_core::history::HistoryStack;
use hollow_core::selection::{SelectionMask, StrokePosition};
use hollow_core::symmetry::SymmetryConfig;
use hollow_core::transform::{AffineTransform2D, render_transformed_patch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformHandle {
    TopLeft,
    TopCenter,
    TopRight,
    MidRight,
    BottomRight,
    BottomCenter,
    BottomLeft,
    MidLeft,
    RotateStem,
    Pivot,
    BodyTranslate,
}

#[derive(Debug, Clone)]
pub struct TransformSession {
    pub is_active: bool,
    pub original_layer_pixels: Vec<u8>,
    pub extracted_patch: Vec<u8>,
    pub patch_w: u32,
    pub patch_h: u32,
    pub patch_origin: Vec2,
    pub transform: AffineTransform2D,
    pub is_bilinear: bool,
    pub active_handle: Option<TransformHandle>,
    pub drag_start_canvas_pos: Vec2,
    pub initial_transform: AffineTransform2D,
    pub lock_aspect: bool,
}

impl Default for TransformSession {
    fn default() -> Self {
        Self {
            is_active: false,
            original_layer_pixels: Vec::new(),
            extracted_patch: Vec::new(),
            patch_w: 0,
            patch_h: 0,
            patch_origin: Vec2::ZERO,
            transform: AffineTransform2D::default(),
            is_bilinear: true,
            active_handle: None,
            drag_start_canvas_pos: Vec2::ZERO,
            initial_transform: AffineTransform2D::default(),
            lock_aspect: false,
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveFilterModal {
    None,
    Hsl,
    BrightnessContrast,
    ColorBalance,
    PosterizeThreshold,
    GaussianBlur,
    SharpenUnsharp,
    FilmGrain,
    VignetteChromatic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceMode {
    FloatingWindow, // Detached separate lightbox dock
    CanvasTracing,  // Pinned directly to canvas for tracing paper overlay/underlay
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
    pub show_about_dialog: bool,
    pub show_ui_panels: bool, // Zen Mode / Full Canvas toggle
    pub show_gallery: bool,

    // Overlay Toggles & Viewport
    pub show_grid: bool,
    pub grid_size: u32,
    pub grid_opacity: f32,
    pub show_rulers: bool,
    pub show_navigator: bool,
    pub flip_view_horizontal: bool,
    pub perspective: hollow_core::perspective::PerspectiveConfig,
    pub show_perspective_dock: bool,
    pub active_adjustment_modal: Option<u64>,

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

    // Reference & Tracing System
    pub reference_mode: ReferenceMode,
    pub reference_image: Option<(u32, u32, Vec<u8>)>,
    pub ref_texture: Option<egui::TextureHandle>,
    pub show_ref_window: bool,
    pub ref_zoom: f32,
    pub ref_pan: Vec2,
    pub ref_backlight: bool,
    pub ref_backlight_mode: u8, // 0: Dark, 1: Lightbox Pure White, 2: Checkerboard

    // On-Canvas Tracing Paper Controls
    pub tracing_enabled: bool,
    pub tracing_opacity: f32, // 0.05..=1.0
    pub tracing_pos: Vec2,    // Canvas coordinate offset (X, Y)
    pub tracing_scale: f32,   // Canvas scaling
    pub tracing_as_underlay: bool, // true: underlay (light table), false: ghost overlay (tracing sheet)
    pub tracing_locked: bool,

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

    // ── Filters & Adjustments State ──
    pub active_filter_modal: ActiveFilterModal,
    pub filter_original_pixels: Option<Vec<u8>>,
    pub filter_preview_active: bool,

    pub filter_hue_shift: f32,
    pub filter_saturation_scale: f32,
    pub filter_lightness_shift: f32,

    pub filter_brightness: f32,
    pub filter_contrast: f32,

    pub filter_cyan_red: f32,
    pub filter_magenta_green: f32,
    pub filter_yellow_blue: f32,

    pub filter_posterize_levels: u32,
    pub filter_threshold_val: u8,

    pub filter_blur_radius: f32,
    pub filter_sharpen_amount: f32,
    pub filter_unsharp_radius: f32,
    pub filter_unsharp_amount: f32,
    pub filter_unsharp_threshold: f32,

    pub filter_noise_intensity: f32,
    pub filter_noise_colored: bool,

    pub filter_vignette_radius: f32,
    pub filter_vignette_softness: f32,
    pub filter_vignette_darkness: f32,

    pub filter_chromatic_shift: f32,
    pub filter_chromatic_angle: f32,

    // ── Free Transform Session ──
    pub transform_session: TransformSession,

    // ── Lasso & Selection Modifiers ──
    pub lasso_points: Vec<Vec2>,
    pub show_feather_dialog: bool,
    pub feather_radius: u32,
    pub show_expand_dialog: bool,
    pub expand_radius: u32,
    pub show_contract_dialog: bool,
    pub contract_radius: u32,
    pub show_stroke_dialog: bool,
    pub stroke_width: u32,
    pub stroke_position: u8, // 0: Center, 1: Inside, 2: Outside

    // ── Brush Preset Shelf ──
    pub presets: Vec<hollow_core::brush::BrushPreset>,
    pub active_preset_idx: Option<usize>,
    pub show_save_preset_dialog: bool,
    pub new_preset_name: String,

    // ── Frame Animation & Timeline Strip ──
    pub timeline: hollow_core::animation::AnimationTimeline,
    pub show_export_animation_dialog: bool,
    pub export_anim_fps: u32,
    pub export_anim_format: u8, // 0: GIF, 1: WebP, 2: PNG Sequence
    pub export_anim_loop: bool,

    // ── Floating Mixing Scratchpad ──
    pub show_scratchpad: bool,
    pub scratchpad_w: u32,
    pub scratchpad_h: u32,
    pub scratchpad_pixels: Vec<u8>,
    pub scratchpad_texture: Option<egui::TextureHandle>,
    pub scratchpad_texture_dirty: bool,
    pub scratchpad_brush_size: f32,
    pub scratchpad_mode: u8, // 0: Paint, 1: Smudge / Water Mix, 2: Eyedropper, 3: Eraser
    pub scratchpad_bg_mode: u8, // 0: Dark Studio, 1: Pure White, 2: Transparent
    pub scratchpad_last_pos: Option<Vec2>,
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
            show_about_dialog: false,
            show_ui_panels: true,
            show_gallery: false,

            show_grid: false,
            grid_size: 32,
            grid_opacity: 0.25,
            show_rulers: true,
            show_navigator: true,
            flip_view_horizontal: false,
            perspective: {
                let mut p = hollow_core::perspective::PerspectiveConfig::default();
                p.init_for_canvas(width, height);
                p
            },
            show_perspective_dock: false,
            active_adjustment_modal: None,

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

            reference_mode: ReferenceMode::CanvasTracing,
            reference_image: None,
            ref_texture: None,
            show_ref_window: false,
            ref_zoom: 1.0,
            ref_pan: Vec2::ZERO,
            ref_backlight: false,
            ref_backlight_mode: 0,

            tracing_enabled: false,
            tracing_opacity: 0.5,
            tracing_pos: Vec2::ZERO,
            tracing_scale: 1.0,
            tracing_as_underlay: false,
            tracing_locked: false,

            is_painting: false,
            last_paint_pos: None,
            stroke_pixels_backup: None,
            is_space_panning: false,
            pan_drag_start: None,
            pending_file_action: None,
            drag_start_canvas_pos: None,
            polygon_points: Vec::new(),
            crop_box: None,

            // Filter defaults
            active_filter_modal: ActiveFilterModal::None,
            filter_original_pixels: None,
            filter_preview_active: true,

            filter_hue_shift: 0.0,
            filter_saturation_scale: 1.0,
            filter_lightness_shift: 0.0,

            filter_brightness: 0.0,
            filter_contrast: 0.0,

            filter_cyan_red: 0.0,
            filter_magenta_green: 0.0,
            filter_yellow_blue: 0.0,

            filter_posterize_levels: 6,
            filter_threshold_val: 128,

            filter_blur_radius: 4.0,
            filter_sharpen_amount: 1.0,
            filter_unsharp_radius: 2.5,
            filter_unsharp_amount: 1.2,
            filter_unsharp_threshold: 3.0,

            filter_noise_intensity: 0.25,
            filter_noise_colored: false,

            filter_vignette_radius: 0.8,
            filter_vignette_softness: 0.6,
            filter_vignette_darkness: 0.75,

            filter_chromatic_shift: 4.0,
            filter_chromatic_angle: 0.0,

            transform_session: TransformSession::default(),

            lasso_points: Vec::new(),
            show_feather_dialog: false,
            feather_radius: 5,
            show_expand_dialog: false,
            expand_radius: 5,
            show_contract_dialog: false,
            contract_radius: 5,
            show_stroke_dialog: false,
            stroke_width: 3,
            stroke_position: 0, // Center

            presets: hollow_core::brush::BrushPreset::default_library(),
            active_preset_idx: Some(0),
            show_save_preset_dialog: false,
            new_preset_name: String::new(),

            timeline: hollow_core::animation::AnimationTimeline::new(width, height),
            show_export_animation_dialog: false,
            export_anim_fps: 12,
            export_anim_format: 0,
            export_anim_loop: true,

            show_scratchpad: false,
            scratchpad_w: 360,
            scratchpad_h: 360,
            scratchpad_pixels: vec![24, 28, 42, 255].repeat(360 * 360),
            scratchpad_texture: None,
            scratchpad_texture_dirty: true,
            scratchpad_brush_size: 14.0,
            scratchpad_mode: 0,
            scratchpad_bg_mode: 0,
            scratchpad_last_pos: None,
        }
    }

    // ── Animation Timeline Management ──
    pub fn select_animation_frame(&mut self, idx: usize) {
        if idx < self.timeline.frames.len() {
            self.timeline.sync_from_document(&self.document);
            self.timeline.current_frame_idx = idx;
            self.timeline.sync_to_document(&mut self.document);
            self.set_status(format!("Frame {} / {}", idx + 1, self.timeline.frames.len()));
        }
    }

    pub fn add_animation_frame(&mut self) {
        self.timeline.sync_from_document(&self.document);
        let new_idx = self.timeline.add_frame(self.document.width, self.document.height);
        self.timeline.sync_to_document(&mut self.document);
        self.set_status(format!("Added frame {} / {}", new_idx + 1, self.timeline.frames.len()));
    }

    pub fn duplicate_animation_frame(&mut self) {
        self.timeline.sync_from_document(&self.document);
        let new_idx = self.timeline.duplicate_current_frame();
        self.timeline.sync_to_document(&mut self.document);
        self.set_status(format!("Duplicated frame {} / {}", new_idx + 1, self.timeline.frames.len()));
    }

    pub fn delete_animation_frame(&mut self) {
        if self.timeline.delete_current_frame() {
            self.timeline.sync_to_document(&mut self.document);
            self.set_status(format!("Frame deleted. ({} frames left)", self.timeline.frames.len()));
        }
    }

    pub fn reorder_animation_frame(&mut self, from: usize, to: usize) {
        self.timeline.sync_from_document(&self.document);
        self.timeline.move_frame(from, to);
        self.timeline.sync_to_document(&mut self.document);
        self.set_status(format!("Reordered frame {} to {}", from + 1, to + 1));
    }

    pub fn step_next_frame(&mut self) {
        self.timeline.sync_from_document(&self.document);
        self.timeline.step_next_frame();
        self.timeline.sync_to_document(&mut self.document);
    }

    pub fn step_prev_frame(&mut self) {
        self.timeline.sync_from_document(&self.document);
        self.timeline.step_prev_frame();
        self.timeline.sync_to_document(&mut self.document);
    }

    pub fn toggle_animation_playback(&mut self) {
        self.timeline.is_playing = !self.timeline.is_playing;
        let state_str = if self.timeline.is_playing { "Playing" } else { "Paused" };
        self.set_status(format!("Animation {}", state_str));
    }

    pub fn toggle_onion_skin(&mut self) {
        self.timeline.onion_skin_enabled = !self.timeline.onion_skin_enabled;
        let state_str = if self.timeline.onion_skin_enabled { "ON" } else { "OFF" };
        self.set_status(format!("Onion Skinning: {}", state_str));
    }

    pub fn select_preset(&mut self, idx: usize) {
        if idx < self.presets.len() {
            let p = &self.presets[idx];
            let prim = self.brush.primary_color;
            let sec = self.brush.secondary_color;
            self.brush = p.settings.clone();
            self.brush.primary_color = prim;
            self.brush.secondary_color = sec;
            self.active_preset_idx = Some(idx);
            self.set_status(format!("Loaded brush preset: {}", p.name));
        }
    }

    pub fn save_current_as_preset(&mut self, name: &str) {
        let name_trimmed = name.trim();
        if name_trimmed.is_empty() {
            return;
        }
        let icon = match self.brush.tool {
            hollow_core::brush::ToolType::Pencil => "✎",
            hollow_core::brush::ToolType::Watercolor => "≋",
            hollow_core::brush::ToolType::Chalk => "░",
            hollow_core::brush::ToolType::Spray => "⁕",
            hollow_core::brush::ToolType::Smudge => "≈",
            _ => "✦",
        };
        let new_preset = hollow_core::brush::BrushPreset {
            name: name_trimmed.to_string(),
            icon: icon.to_string(),
            category: "Custom".to_string(),
            description: format!("Custom user preset (Size: {:.1}px)", self.brush.size),
            settings: self.brush.clone(),
        };
        self.presets.push(new_preset);
        self.active_preset_idx = Some(self.presets.len() - 1);
        self.set_status(format!("Saved custom preset: {}", name_trimmed));
    }

    pub fn reset_presets_to_default(&mut self) {
        self.presets = hollow_core::brush::BrushPreset::default_library();
        self.active_preset_idx = Some(0);
        self.select_preset(0);
        self.set_status("Reset preset shelf to factory defaults");
    }

    pub fn from_document(doc: Document) -> Self {
        let (w, h) = (doc.width, doc.height);
        let mut s = Self::new(w, h);
        s.document = doc;
        s
    }

    pub fn reset_view(&mut self) {
        self.pan = Vec2::ZERO;
        self.zoom = 1.0;
        self.set_status("View reset to 100%");
    }

    pub fn viewport_center_offset(&self) -> Vec2 {
        if self.show_ui_panels {
            let bottom = if self.timeline.is_enabled { 88.0 } else { 32.0 };
            Vec2::new((220.0 - 240.0) * 0.5, (38.0 - bottom) * 0.5)
        } else {
            Vec2::ZERO
        }
    }

    pub fn viewport_rect(&self, win_w: usize, win_h: usize) -> [usize; 4] {
        if self.show_ui_panels {
            let bottom = if self.timeline.is_enabled { 88 } else { 32 };
            let vx0 = 220.min(win_w);
            let vy0 = 38.min(win_h);
            let vx1 = win_w.saturating_sub(240).max(vx0);
            let vy1 = win_h.saturating_sub(bottom).max(vy0);
            [vx0, vy0, vx1, vy1]
        } else {
            [0, 0, win_w, win_h]
        }
    }

    pub fn viewport_center(&self, win_w: f32, win_h: f32) -> Vec2 {
        Vec2::new(win_w * 0.5, win_h * 0.5) + self.pan + self.viewport_center_offset()
    }

    pub fn reset_view_centered(&mut self, win_w: f32, win_h: f32) {
        self.pan = Vec2::ZERO;
        let avail_w = if self.show_ui_panels { (win_w - 510.0).max(200.0) } else { (win_w - 60.0).max(200.0) };
        let bottom = if self.timeline.is_enabled { 88.0 } else { 32.0 };
        let avail_h = if self.show_ui_panels { (win_h - (38.0 + bottom + 30.0)).max(200.0) } else { (win_h - 60.0).max(200.0) };
        let scale_x = avail_w / self.document.width as f32;
        let scale_y = avail_h / self.document.height as f32;
        self.zoom = (scale_x.min(scale_y) * 0.92).clamp(0.05, 5.0);
        self.set_status(format!("Fit to screen ({:.0}%)", self.zoom * 100.0));
    }

    pub fn pan_to_canvas_center(&mut self, target_canvas_pos: Vec2) {
        let canvas_size = Vec2::new(self.document.width as f32, self.document.height as f32);
        self.pan = -(target_canvas_pos - canvas_size * 0.5) * self.zoom;
    }

    pub fn theme_accent_color(&self) -> Color {
        self.document.theme.accent_color()
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
    }

    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.brush.primary_color, &mut self.brush.secondary_color);
        self.set_status("Swapped primary and secondary colors");
    }

    pub fn push_color_history(&mut self, color: Color) {
        if self.color_history.first() == Some(&color) {
            return;
        }
        self.color_history.retain(|&c| c != color);
        self.color_history.insert(0, color);
        if self.color_history.len() > 18 {
            self.color_history.pop();
        }
    }

    pub fn screen_to_canvas(&self, screen_pos: Vec2, win_w: f32, win_h: f32) -> Vec2 {
        let center = self.viewport_center(win_w, win_h);
        let offset = screen_pos - center;
        let canvas_size = Vec2::new(self.document.width as f32, self.document.height as f32);
        (offset / self.zoom) + (canvas_size * 0.5)
    }

    pub fn canvas_to_screen(&self, canvas_pos: Vec2, win_w: f32, win_h: f32) -> Vec2 {
        let center = self.viewport_center(win_w, win_h);
        let canvas_size = Vec2::new(self.document.width as f32, self.document.height as f32);
        let offset = (canvas_pos - canvas_size * 0.5) * self.zoom;
        center + offset
    }

    pub fn fit_tracing_to_canvas(&mut self) {
        if let Some((rw, rh, _)) = self.reference_image {
            let scale_x = self.document.width as f32 / rw as f32;
            let scale_y = self.document.height as f32 / rh as f32;
            self.tracing_scale = scale_x.min(scale_y);
            let scaled_w = rw as f32 * self.tracing_scale;
            let scaled_h = rh as f32 * self.tracing_scale;
            self.tracing_pos = Vec2::new(
                (self.document.width as f32 - scaled_w) * 0.5,
                (self.document.height as f32 - scaled_h) * 0.5,
            );
            self.set_status("Tracing reference fit to canvas");
        }
    }

    pub fn center_tracing_on_canvas(&mut self) {
        if let Some((rw, rh, _)) = self.reference_image {
            let scaled_w = rw as f32 * self.tracing_scale;
            let scaled_h = rh as f32 * self.tracing_scale;
            self.tracing_pos = Vec2::new(
                (self.document.width as f32 - scaled_w) * 0.5,
                (self.document.height as f32 - scaled_h) * 0.5,
            );
            self.set_status("Tracing reference centered");
        }
    }

    /// Backs up the active layer's pixels before opening a filter dialog
    pub fn begin_filter_modal(&mut self, modal: ActiveFilterModal) {
        if let Some(layer) = self.document.active_layer() {
            self.filter_original_pixels = Some(layer.pixels.clone());
            self.active_filter_modal = modal;
            self.filter_preview_active = true;
        }
    }

    /// Cancels filter modal and restores original layer pixels
    pub fn cancel_filter_modal(&mut self) {
        if let Some(orig) = self.filter_original_pixels.take() {
            if let Some(layer) = self.document.active_layer_mut() {
                layer.pixels = orig;
            }
        }
        self.active_filter_modal = ActiveFilterModal::None;
    }

    /// Commits the filter modal changes and registers to undo history
    pub fn apply_filter_modal(&mut self, filter_name: &'static str) {
        if let Some(before) = self.filter_original_pixels.take() {
            if let Some(layer) = self.document.active_layer() {
                if before != layer.pixels {
                    let cmd = Box::new(hollow_core::history::LayerPixelsSnapshotCommand {
                        layer_id: layer.id,
                        description: filter_name,
                        before_pixels: before,
                        after_pixels: layer.pixels.clone(),
                    });
                    self.history.push(cmd);
                    self.set_status(format!("Applied {}", filter_name));
                }
            }
        }
        self.active_filter_modal = ActiveFilterModal::None;
    }

    /// Starts a Free Transform session on the active layer (or active selection mask)
    pub fn begin_transform_session(&mut self) {
        if let Some(layer) = self.document.active_layer() {
            let doc_w = self.document.width;
            let doc_h = self.document.height;

            // Determine content bounding box
            let mut min_x = doc_w;
            let mut max_x = 0;
            let mut min_y = doc_h;
            let mut max_y = 0;

            if let Some(sel) = &self.selection {
                for y in 0..doc_h {
                    for x in 0..doc_w {
                        if sel.is_selected(x, y) {
                            let idx = (y * doc_w + x) as usize * 4;
                            if layer.pixels[idx + 3] > 0 {
                                min_x = min_x.min(x);
                                max_x = max_x.max(x);
                                min_y = min_y.min(y);
                                max_y = max_y.max(y);
                            }
                        }
                    }
                }
            } else {
                for y in 0..doc_h {
                    for x in 0..doc_w {
                        let idx = (y * doc_w + x) as usize * 4;
                        if layer.pixels[idx + 3] > 0 {
                            min_x = min_x.min(x);
                            max_x = max_x.max(x);
                            min_y = min_y.min(y);
                            max_y = max_y.max(y);
                        }
                    }
                }
            }

            if min_x > max_x || min_y > max_y {
                // If entire layer is empty, default to canvas center 200x200
                min_x = (doc_w.saturating_sub(200)) / 2;
                max_x = (min_x + 200).min(doc_w.saturating_sub(1));
                min_y = (doc_h.saturating_sub(200)) / 2;
                max_y = (min_y + 200).min(doc_h.saturating_sub(1));
            }

            let patch_w = (max_x - min_x + 1).max(1);
            let patch_h = (max_y - min_y + 1).max(1);
            let mut patch = vec![0u8; (patch_w * patch_h * 4) as usize];

            let sel_opt = self.selection.as_ref();
            for py in 0..patch_h {
                let sy = min_y + py;
                for px in 0..patch_w {
                    let sx = min_x + px;
                    let should_extract = sel_opt.map_or(true, |s| s.is_selected(sx, sy));
                    if should_extract {
                        let src_idx = (sy * doc_w + sx) as usize * 4;
                        let dst_idx = (py * patch_w + px) as usize * 4;
                        patch[dst_idx..dst_idx + 4].copy_from_slice(&layer.pixels[src_idx..src_idx + 4]);
                    }
                }
            }

            let patch_origin = Vec2::new(min_x as f32, min_y as f32);
            let center = patch_origin + Vec2::new(patch_w as f32 * 0.5, patch_h as f32 * 0.5);

            let mut transform = AffineTransform2D::new(center);
            transform.pivot = center;

            self.transform_session = TransformSession {
                is_active: true,
                original_layer_pixels: layer.pixels.clone(),
                extracted_patch: patch,
                patch_w,
                patch_h,
                patch_origin,
                transform,
                is_bilinear: true,
                active_handle: None,
                drag_start_canvas_pos: Vec2::ZERO,
                initial_transform: transform,
                lock_aspect: false,
            };

            // Clear extracted pixels from active layer so transformed patch replaces them cleanly during preview
            if let Some(layer_mut) = self.document.active_layer_mut() {
                for py in 0..patch_h {
                    let sy = min_y + py;
                    for px in 0..patch_w {
                        let sx = min_x + px;
                        let should_clear = sel_opt.map_or(true, |s| s.is_selected(sx, sy));
                        if should_clear {
                            let idx = (sy * doc_w + sx) as usize * 4;
                            layer_mut.pixels[idx] = 0;
                            layer_mut.pixels[idx + 1] = 0;
                            layer_mut.pixels[idx + 2] = 0;
                            layer_mut.pixels[idx + 3] = 0;
                        }
                    }
                }
            }

            self.update_transform_preview();
            self.set_status("Free Transform: Drag handles to Scale/Rotate/Translate | Enter to Apply, Esc to Cancel");
        }
    }

    /// Re-renders the transformed patch onto active layer for live canvas preview
    pub fn update_transform_preview(&mut self) {
        if !self.transform_session.is_active {
            return;
        }
        let doc_w = self.document.width;
        let doc_h = self.document.height;
        let session = &self.transform_session;
        let orig = &session.original_layer_pixels;
        let patch = &session.extracted_patch;
        let pw = session.patch_w;
        let ph = session.patch_h;
        let origin = session.patch_origin;
        let transform = session.transform;
        let bilinear = session.is_bilinear;
        let sel_opt = self.selection.as_ref();

        if let Some(layer) = self.document.active_layer_mut() {
            // Start from original pixels with source patch cleared
            layer.pixels.copy_from_slice(orig);
            for py in 0..ph {
                let sy = origin.y as u32 + py;
                for px in 0..pw {
                    let sx = origin.x as u32 + px;
                    if sx < doc_w && sy < doc_h {
                        let should_clear = sel_opt.map_or(true, |s| s.is_selected(sx, sy));
                        if should_clear {
                            let idx = (sy * doc_w + sx) as usize * 4;
                            layer.pixels[idx] = 0;
                            layer.pixels[idx + 1] = 0;
                            layer.pixels[idx + 2] = 0;
                            layer.pixels[idx + 3] = 0;
                        }
                    }
                }
            }

            render_transformed_patch(
                patch,
                pw,
                ph,
                origin,
                &transform,
                bilinear,
                &mut layer.pixels,
                doc_w,
                doc_h,
            );
        }
    }

    /// Commits the transformation to layer and registers in history
    pub fn commit_transform_session(&mut self) {
        if !self.transform_session.is_active {
            return;
        }
        self.update_transform_preview();
        if let Some(layer) = self.document.active_layer() {
            let before = self.transform_session.original_layer_pixels.clone();
            let after = layer.pixels.clone();
            if before != after {
                let cmd = Box::new(hollow_core::history::LayerPixelsSnapshotCommand {
                    layer_id: layer.id,
                    description: "Free Transform",
                    before_pixels: before,
                    after_pixels: after,
                });
                self.history.push(cmd);
                self.set_status("Committed Free Transform");
            }
        }
        self.transform_session.is_active = false;
        self.transform_session.original_layer_pixels.clear();
        self.transform_session.extracted_patch.clear();
    }

    /// Cancels transformation and restores original layer pixels
    pub fn cancel_transform_session(&mut self) {
        if !self.transform_session.is_active {
            return;
        }
        if let Some(layer) = self.document.active_layer_mut() {
            layer.pixels = std::mem::take(&mut self.transform_session.original_layer_pixels);
        }
        self.transform_session.is_active = false;
        self.transform_session.extracted_patch.clear();
        self.set_status("Canceled Transform");
    }

    pub fn feather_selection(&mut self, radius: u32) {
        if let Some(mask) = &mut self.selection {
            mask.feather(radius);
            self.set_status(format!("Feathered selection ({}px)", radius));
        }
    }

    pub fn expand_selection(&mut self, radius: u32) {
        if let Some(mask) = &mut self.selection {
            mask.expand(radius);
            self.set_status(format!("Expanded selection ({}px)", radius));
        }
    }

    pub fn contract_selection(&mut self, radius: u32) {
        if let Some(mask) = &mut self.selection {
            mask.contract(radius);
            self.set_status(format!("Contracted selection ({}px)", radius));
        }
    }

    pub fn fill_selection_active_layer(&mut self) {
        if let Some(mask) = &self.selection {
            let doc_w = self.document.width;
            let doc_h = self.document.height;
            let color = self.brush.primary_color.to_rgba8();
            if let Some(layer) = self.document.active_layer_mut() {
                let before = layer.pixels.clone();
                mask.fill_selection(&mut layer.pixels, doc_w, doc_h, color);
                let after = layer.pixels.clone();
                if before != after {
                    let cmd = Box::new(hollow_core::history::LayerPixelsSnapshotCommand {
                        layer_id: layer.id,
                        description: "Fill Selection",
                        before_pixels: before,
                        after_pixels: after,
                    });
                    self.history.push(cmd);
                    self.set_status("Filled selection with primary color");
                }
            }
        }
    }

    pub fn stroke_selection_active_layer(&mut self, width: u32, position: u8) {
        if let Some(mask) = &self.selection {
            let doc_w = self.document.width;
            let doc_h = self.document.height;
            let color = self.brush.primary_color.to_rgba8();
            let pos = match position {
                1 => StrokePosition::Inside,
                2 => StrokePosition::Outside,
                _ => StrokePosition::Center,
            };
            if let Some(layer) = self.document.active_layer_mut() {
                let before = layer.pixels.clone();
                mask.stroke_selection(&mut layer.pixels, doc_w, doc_h, color, width, pos);
                let after = layer.pixels.clone();
                if before != after {
                    let cmd = Box::new(hollow_core::history::LayerPixelsSnapshotCommand {
                        layer_id: layer.id,
                        description: "Stroke Selection",
                        before_pixels: before,
                        after_pixels: after,
                    });
                    self.history.push(cmd);
                    self.set_status(format!("Stroked selection ({}px)", width));
                }
            }
        }
    }

    // ── Floating Mixing Scratchpad Operations ──
    pub fn clear_scratchpad(&mut self, bg_mode: u8) {
        self.scratchpad_bg_mode = bg_mode;
        let fill_color = match bg_mode {
            1 => [255, 255, 255, 255], // Pure White
            2 => [0, 0, 0, 0],         // Transparent
            _ => [24, 28, 42, 255],    // Dark Studio
        };
        for chunk in self.scratchpad_pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&fill_color);
        }
        self.scratchpad_texture_dirty = true;
        self.set_status("Scratchpad cleared");
    }

    pub fn sample_scratchpad_pixel(&mut self, x: u32, y: u32) {
        if x < self.scratchpad_w && y < self.scratchpad_h {
            let idx = ((y * self.scratchpad_w + x) * 4) as usize;
            if idx + 3 < self.scratchpad_pixels.len() {
                let r = self.scratchpad_pixels[idx];
                let g = self.scratchpad_pixels[idx + 1];
                let b = self.scratchpad_pixels[idx + 2];
                let a = self.scratchpad_pixels[idx + 3];
                if a > 10 {
                    let color = Color::from_rgba8(r, g, b, 255);
                    self.brush.primary_color = color;
                    self.push_color_history(color);
                    self.set_status(format!("Sampled from Scratchpad: {}", color.to_hex()));
                }
            }
        }
    }

    pub fn paint_scratchpad_point(&mut self, pt: Vec2, prev_pt: Option<Vec2>, pressure: f32) {
        let w = self.scratchpad_w;
        let h = self.scratchpad_h;
        let radius = (self.scratchpad_brush_size * pressure * 0.5).max(1.0);
        let radius_sq = radius * radius;
        let hardness = self.brush.hardness.clamp(0.05, 0.95);
        let aa_fringe = 1.0_f32.min(radius * 0.5);
        let inner_r = (radius * hardness).min(radius - aa_fringe).max(0.0);
        let inner_radius_sq = inner_r * inner_r;
        let opacity = self.brush.opacity;
        let mode = self.scratchpad_mode;

        let min_x = ((pt.x - radius - 0.5).floor() as i32).max(0);
        let max_x = ((pt.x + radius + 0.5).ceil() as i32).min(w as i32 - 1);
        let min_y = ((pt.y - radius - 0.5).floor() as i32).max(0);
        let max_y = ((pt.y + radius + 0.5).ceil() as i32).min(h as i32 - 1);

        let color_rgba = self.brush.primary_color.to_rgba8();

        for py_i in min_y..=max_y {
            let y = py_i as u32;
            let dy_f = (y as f32 + 0.5) - pt.y;
            let dy_sq = dy_f * dy_f;
            if dy_sq > radius_sq {
                continue;
            }

            for px_i in min_x..=max_x {
                let x = px_i as u32;
                let dx_f = (x as f32 + 0.5) - pt.x;
                let d_sq = dx_f * dx_f + dy_sq;
                if d_sq <= radius_sq {
                    let d = d_sq.sqrt();
                    let alpha = if d_sq <= inner_radius_sq {
                        1.0
                    } else {
                        (1.0 - (d - inner_r) / (radius - inner_r).max(0.001)).clamp(0.0, 1.0)
                    } * opacity;

                    let idx = ((y * w + x) * 4) as usize;
                    if idx + 3 >= self.scratchpad_pixels.len() {
                        continue;
                    }

                    let dst = [
                        self.scratchpad_pixels[idx],
                        self.scratchpad_pixels[idx + 1],
                        self.scratchpad_pixels[idx + 2],
                        self.scratchpad_pixels[idx + 3],
                    ];

                    match mode {
                        1 => {
                            // Smudge / Water Color Blend
                            if let Some(prev) = prev_pt {
                                let drag_vec = pt - prev;
                                let drag_dist = drag_vec.length();
                                let shift = (drag_dist * 1.5).min(radius * 0.9);
                                let dir = if drag_dist > 0.001 { drag_vec / drag_dist } else { Vec2::ZERO };
                                let sx = (x as f32 - dir.x * shift).clamp(0.0, w as f32 - 1.0);
                                let sy = (y as f32 - dir.y * shift).clamp(0.0, h as f32 - 1.0);
                                let src = hollow_core::rasterizer::sample_bilinear_rgba(&self.scratchpad_pixels, w, h, sx, sy);
                                let blended = BlendMode::Normal.composite_pixel(dst, src, alpha * 0.65);
                                self.scratchpad_pixels[idx..idx + 4].copy_from_slice(&blended);
                            }
                        }
                        3 => {
                            // Eraser
                            let current_a = dst[3] as f32 / 255.0;
                            let new_a = (current_a * (1.0 - alpha)).clamp(0.0, 1.0);
                            self.scratchpad_pixels[idx + 3] = (new_a * 255.0).round() as u8;
                        }
                        _ => {
                            // Normal Paint
                            let blended = BlendMode::Normal.composite_pixel(dst, color_rgba, alpha);
                            self.scratchpad_pixels[idx..idx + 4].copy_from_slice(&blended);
                        }
                    }
                }
            }
        }
        self.scratchpad_texture_dirty = true;
    }

    pub fn paint_scratchpad_stroke(&mut self, from: Vec2, to: Vec2, pressure: f32) {
        let dist = from.distance(to);
        let radius = (self.scratchpad_brush_size * pressure * 0.5).max(1.0);
        let step = (radius * 0.35).max(0.5);
        let steps = ((dist / step).ceil() as usize).max(1);
        let mut prev = Some(from);
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let pos = from.lerp(to, t);
            self.paint_scratchpad_point(pos, prev, pressure);
            prev = Some(pos);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(1920, 1080)
    }
}
