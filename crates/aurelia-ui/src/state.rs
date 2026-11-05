use glam::Vec2;
use std::path::PathBuf;
use std::time::Instant;
use aurelia_core::blend::BlendMode;
use aurelia_core::brush::BrushSettings;
use aurelia_core::color::{Color, DEFAULT_PALETTE};
use aurelia_core::document::Document;
use aurelia_core::history::HistoryStack;
use aurelia_core::layer::PixelFormat;
use aurelia_core::selection::{SelectionMask, StrokePosition};
use aurelia_core::symmetry::SymmetryConfig;
use aurelia_core::transform::{
    render_mesh_warped_patch, render_quad_transformed_patch, render_tps_warped_patch,
    render_transformed_patch, AffineTransform2D, InterpolationMode, MeshWarpGrid,
    PerspectiveQuadTransform, ThinPlateSpline2D, TransformMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformHandle {
    // Affine Box Handles
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

    // Perspective Quad Handles
    QuadCorner(usize), // 0: TopLeft, 1: TopRight, 2: BottomRight, 3: BottomLeft

    // Mesh Grid Handles
    MeshVertex(usize), // Vertex index in mesh_grid.vertices

    // Thin Plate Spline Landmark Handles
    TpsPin(usize), // Pin index in tps_target_pins
}

#[derive(Debug, Clone)]
pub struct TransformSession {
    pub is_active: bool,
    pub mode: TransformMode,
    pub original_layer_pixels: Vec<u8>,
    pub extracted_patch: Vec<u8>,
    pub patch_w: u32,
    pub patch_h: u32,
    pub patch_origin: Vec2,
    pub transform: AffineTransform2D,
    pub quad: PerspectiveQuadTransform,
    pub mesh_grid: MeshWarpGrid,
    pub tps: ThinPlateSpline2D,
    pub tps_source_pins: Vec<Vec2>,
    pub tps_target_pins: Vec<Vec2>,
    pub interpolation: InterpolationMode,
    pub active_handle: Option<TransformHandle>,
    pub drag_start_canvas_pos: Vec2,
    pub initial_transform: AffineTransform2D,
    pub lock_aspect: bool,
    pub grid_rows: usize,
    pub grid_cols: usize,
}

impl Default for TransformSession {
    fn default() -> Self {
        Self {
            is_active: false,
            mode: TransformMode::Affine,
            original_layer_pixels: Vec::new(),
            extracted_patch: Vec::new(),
            patch_w: 0,
            patch_h: 0,
            patch_origin: Vec2::ZERO,
            transform: AffineTransform2D::default(),
            quad: PerspectiveQuadTransform::default(),
            mesh_grid: MeshWarpGrid::new(4, 4, Vec2::ZERO, 100.0, 100.0),
            tps: ThinPlateSpline2D::default(),
            tps_source_pins: Vec::new(),
            tps_target_pins: Vec::new(),
            interpolation: InterpolationMode::Bicubic,
            active_handle: None,
            drag_start_canvas_pos: Vec2::ZERO,
            initial_transform: AffineTransform2D::default(),
            lock_aspect: false,
            grid_rows: 4,
            grid_cols: 4,
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
    SaveProjectAs,
    OpenProject,
    ExportPng,
    OpenReferenceImage,
    NewCanvas(NewCanvasSpec),
}

/// Everything the New Canvas dialog collects, carried as one value so the variant does not
/// grow another positional argument every time a control is added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewCanvasSpec {
    pub width: u32,
    pub height: u32,
    /// 0: dark, 1: white, 2: transparent
    pub bg_mode: u8,
    pub format: PixelFormat,
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
    Levels,
    Colorize,
    Pixelate,
    MotionBlur,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceMode {
    FloatingWindow, // Detached separate lightbox dock
    CanvasTracing,  // Pinned directly to canvas for tracing paper overlay/underlay
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowAction {
    Minimize,
    ToggleMaximize,
    Close,
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
    /// Chrome metrics written by the UI each frame, so canvas math follows the docks.
    pub left_panel_w: f32,
    pub right_panel_w: f32,
    pub header_h: f32,
    pub footer_h: f32,
    /// egui pixels-per-point for the current frame. Panel sizes above and all
    /// view geometry (`pan`, `zoom`, `viewport_*`) are in physical pixels; egui
    /// layout and painting are in points. Convert with this at the boundary.
    pub ui_ppp: f32,
    pub perspective: aurelia_core::perspective::PerspectiveConfig,
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
    /// Storage format the next created document will use.
    pub new_canvas_format: PixelFormat,

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

    // ── Document lifecycle & save safety ──
    /// Path of the project on disk, once it has been saved or opened.
    pub document_path: Option<PathBuf>,
    /// `HistoryStack::generation()` captured at the last successful save.
    pub saved_generation: u64,
    pub recent_files: Vec<PathBuf>,
    pub autosave_enabled: bool,
    pub autosave_interval_secs: u64,
    pub last_autosave: Instant,
    /// Set when an autosave snapshot has been written and not yet cleared by a real save.
    pub autosave_pending: bool,
    // ── Discord Rich Presence ──
    /// Opt-in. Off by default; nothing is sent to Discord until enabled.
    pub discord_presence: bool,
    /// True when the binary was built with a Discord application id.
    pub discord_presence_configured: bool,
    /// Separate opt-in: include the project display name in the activity.
    pub discord_show_project_name: bool,
    pub show_discord_presence_dialog: bool,
    // ── Custom window chrome ──
    pub window_action: Option<WindowAction>,
    pub is_maximized: bool,
    // ── Export settings ──
    pub jpeg_quality: u8,
    pub sprite_columns: u32,
    /// Flat export format: 0 = PNG, 1 = JPEG, 2 = WebP.
    pub export_format: u8,
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

    // ── Levels / Colorize / Pixelate / Motion Blur ──
    pub filter_levels_black: f32,
    pub filter_levels_white: f32,
    pub filter_levels_gamma: f32,
    pub filter_colorize_hue: f32,
    pub filter_colorize_saturation: f32,
    pub filter_pixelate_block: u32,
    pub filter_motion_distance: u32,
    pub filter_motion_angle: f32,

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
    pub presets: Vec<aurelia_core::brush::BrushPreset>,
    pub active_preset_idx: Option<usize>,
    pub show_save_preset_dialog: bool,
    pub new_preset_name: String,

    // ── Frame Animation & Timeline Strip ──
    pub timeline: aurelia_core::animation::AnimationTimeline,
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

    // ── Text Tool & Custom Fonts ──
    pub text: aurelia_core::brush::TextSettings,
    pub text_font: Option<ab_glyph::FontArc>,
    pub text_placement_pos: Option<Vec2>,
    pub text_live_preview: bool,
    pub active_text_layer_id: Option<aurelia_core::layer::LayerId>,
    pub text_box_dragging: bool,
    pub text_box_resizing_handle: Option<usize>, // 0: TopLeft, 1: TopRight, 2: BottomRight, 3: BottomLeft
    pub text_drag_start: Option<Vec2>,
    pub text_box_drag_initial_rect: Option<(f32, f32, f32, f32)>, // pos_x, pos_y, box_w, box_h
    pub show_text_hud: bool,
    pub show_shell_status_modal: Option<String>,

    // ── Navigator ──
    /// The navigator thumbnail, and the only owner of it.
    pub navigator_texture: Option<egui::TextureHandle>,
    /// Bumped by every content change. The navigator rebuilds only when this moves, so an idle
    /// frame costs no recomposite, no downsample and no texture upload.
    pub document_revision: u64,
    /// The revision `navigator_texture` currently reflects.
    pub navigator_revision: u64,
    /// The document size `navigator_texture` was built for.
    pub navigator_size: (u32, u32),
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
            left_panel_w: 268.0,
            right_panel_w: 324.0,
            header_h: 38.0,
            footer_h: 30.0,
            ui_ppp: 1.0,
            perspective: {
                let mut p = aurelia_core::perspective::PerspectiveConfig::default();
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
            new_canvas_format: PixelFormat::Srgb8,

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
            document_path: None,
            saved_generation: 0,
            recent_files: Vec::new(),
            autosave_enabled: true,
            autosave_interval_secs: 120,
            last_autosave: Instant::now(),
            autosave_pending: false,
            discord_presence: false,
            discord_presence_configured: false,
            discord_show_project_name: false,
            show_discord_presence_dialog: false,
            window_action: None,
            is_maximized: false,
            jpeg_quality: 90,
            sprite_columns: 4,
            export_format: 0,
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

            filter_levels_black: 0.0,
            filter_levels_white: 1.0,
            filter_levels_gamma: 1.0,
            filter_colorize_hue: 200.0,
            filter_colorize_saturation: 0.55,
            filter_pixelate_block: 8,
            filter_motion_distance: 12,
            filter_motion_angle: 0.0,

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

            presets: aurelia_core::brush::BrushPreset::default_library(),
            active_preset_idx: Some(0),
            show_save_preset_dialog: false,
            new_preset_name: String::new(),

            timeline: aurelia_core::animation::AnimationTimeline::new(width, height),
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

            text: aurelia_core::brush::TextSettings::default(),
            text_font: aurelia_core::rasterizer::StrokeRasterizer::load_font(None, None),
            text_placement_pos: Some(Vec2::new(50.0, 50.0)),
            text_live_preview: true,
            active_text_layer_id: None,
            text_box_dragging: false,
            text_box_resizing_handle: None,
            text_drag_start: None,
            text_box_drag_initial_rect: None,
            show_text_hud: true,
            show_shell_status_modal: None,

            navigator_texture: None,
            document_revision: 0,
            navigator_revision: u64::MAX,
            navigator_size: (0, 0),
        }
    }

    pub fn load_custom_font(&mut self, path: &str) -> bool {
        if let Some(font) = aurelia_core::rasterizer::StrokeRasterizer::load_font(None, Some(path)) {
            let filename = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());
            self.text.font_name = filename;
            self.text.font_path = Some(path.to_string());
            self.text_font = Some(font);
            self.sync_active_text_layer();
            self.set_status(format!("Loaded custom font: {}", self.text.font_name));
            true
        } else {
            self.set_status("Failed to load font: invalid TTF/OTF file");
            false
        }
    }

    pub fn select_text_layer(&mut self, id: aurelia_core::layer::LayerId) {
        self.active_text_layer_id = Some(id);
        self.document.active_layer_id = id;
        if let Some(layer) = self.document.get_layer(id) {
            if let Some(ref config) = layer.text {
                self.text.content = config.content.clone();
                self.text.font_name = config.font_family.clone();
                self.text.font_size = config.font_size;
                self.text.align = config.align;
                self.text.line_spacing = config.line_spacing;
                self.text.letter_spacing = config.letter_spacing;
                self.text_placement_pos = Some(Vec2::new(config.pos_x, config.pos_y));
                self.brush.primary_color = Color::from_rgba8(
                    config.color[0],
                    config.color[1],
                    config.color[2],
                    config.color[3],
                );
            }
        }
    }

    pub fn create_text_layer_at(&mut self, pos: Vec2, box_size: Option<Vec2>) -> aurelia_core::layer::LayerId {
        let (box_w, box_h) = match box_size {
            Some(sz) => (sz.x.max(0.0), sz.y.max(0.0)),
            None => (0.0, 0.0),
        };
        let mut config = aurelia_core::layer::TextLayerConfig {
            content: if self.text.content.trim().is_empty() {
                "Double-click to edit text".to_string()
            } else {
                self.text.content.clone()
            },
            font_family: self.text.font_name.clone(),
            font_size: self.text.font_size,
            color: self.brush.primary_color.to_rgba8(),
            align: self.text.align,
            line_spacing: self.text.line_spacing,
            letter_spacing: self.text.letter_spacing,
            box_w,
            box_h,
            pos_x: pos.x,
            pos_y: pos.y,
        };
        if config.content == "Aurelia" {
            config.content = "Text Layer".to_string();
        }
        let id = self.document.add_text_layer(None, config.clone(), None);
        if let Some(font) = &self.text_font {
            self.document.update_text_layer_config(id, config, font);
        }
        self.select_text_layer(id);
        self.mark_canvas_dirty();
        self.set_status("Created new interactive Text Layer");
        id
    }

    pub fn sync_active_text_layer(&mut self) {
        let active_id = self.active_text_layer_id.or_else(|| {
            self.document.active_layer().and_then(|l| if l.is_text() { Some(l.id) } else { None })
        });
        if let Some(id) = active_id {
            self.active_text_layer_id = Some(id);
            if let Some(layer) = self.document.get_layer_mut(id) {
                if let Some(mut config) = layer.text.clone() {
                    config.content = self.text.content.clone();
                    config.font_family = self.text.font_name.clone();
                    config.font_size = self.text.font_size;
                    config.color = self.brush.primary_color.to_rgba8();
                    config.align = self.text.align;
                    config.line_spacing = self.text.line_spacing;
                    config.letter_spacing = self.text.letter_spacing;

                    layer.text = Some(config.clone());
                    if let Some(font) = &self.text_font {
                        aurelia_core::rasterizer::StrokeRasterizer::rasterize_text_layer_config(layer, &config, font);
                    }
                    self.mark_canvas_dirty();
                }
            }
        }
    }

    pub fn rasterize_active_text_layer(&mut self) -> bool {
        let active_id = self.document.active_layer_id;
        if self.document.rasterize_text_layer(active_id) {
            self.active_text_layer_id = None;
            self.mark_canvas_dirty();
            self.set_status("Rasterized text layer to standard pixel bitmap");
            true
        } else {
            false
        }
    }

    pub fn place_text_on_canvas(&mut self) -> bool {
        let pos = self.text_placement_pos.unwrap_or_else(|| {
            Vec2::new(
                (self.document.width as f32 * 0.1).max(20.0),
                (self.document.height as f32 * 0.1).max(20.0),
            )
        });

        self.create_text_layer_at(pos, None);
        true
    }

    /// Records that the document's pixels changed, so the navigator rebuilds next frame.
    ///
    /// This is the only signal the thumbnail has: pan and zoom deliberately do not bump it,
    /// because the thumbnail shows the document, not the viewport (the viewport is drawn as a
    /// framing box on top).
    pub fn mark_canvas_dirty(&mut self) {
        self.document_revision = self.document_revision.wrapping_add(1);
    }

    /// Whether the navigator thumbnail must be rebuilt this frame.
    ///
    /// `navigator_revision` starts at `u64::MAX`, so the first frame always refreshes; after
    /// that only a content change or a size change does.
    pub fn navigator_needs_refresh(&self) -> bool {
        self.navigator_revision != self.document_revision
            || self.navigator_size != (self.document.width, self.document.height)
    }

    /// Records that the thumbnail now reflects the current document.
    pub fn note_navigator_refreshed(&mut self) {
        self.navigator_revision = self.document_revision;
        self.navigator_size = (self.document.width, self.document.height);
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
            aurelia_core::brush::ToolType::Pencil => "✎",
            aurelia_core::brush::ToolType::Watercolor => "≋",
            aurelia_core::brush::ToolType::Chalk => "░",
            aurelia_core::brush::ToolType::Spray => "⁕",
            aurelia_core::brush::ToolType::Smudge => "≈",
            _ => "✦",
        };
        let new_preset = aurelia_core::brush::BrushPreset {
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
        self.presets = aurelia_core::brush::BrushPreset::default_library();
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
            let bottom = if self.timeline.is_enabled { 88.0 } else { self.footer_h.max(24.0) };
            let ruler = if self.show_rulers { crate::design::RULER_PX } else { 0.0 };
            Vec2::new(
                (self.left_panel_w - self.right_panel_w + ruler) * 0.5,
                (self.header_h - bottom + ruler) * 0.5,
            )
        } else {
            Vec2::ZERO
        }
    }

    /// Visible canvas area in window pixels: `[x0, y0, x1, y1]`.
    ///
    /// Reads the dock widths the UI actually painted this frame, so resizing a
    /// side panel keeps the canvas clip, hit-testing and rulers aligned.
    pub fn viewport_rect(&self, win_w: usize, win_h: usize) -> [usize; 4] {
        if self.show_ui_panels {
            let bottom = if self.timeline.is_enabled { 88.0 } else { self.footer_h.max(24.0) };
            let ruler = if self.show_rulers { crate::design::RULER_PX } else { 0.0 };
            // The ruler strips are drawn *inside* this rect, along its top and left
            // edges, so the rect itself starts at the panel edge. Adding the ruler
            // here as well left a dead ruler-wide gap beside the left panel and header.
            let _ = ruler;
            let vx0 = (self.left_panel_w.max(0.0) as usize).min(win_w);
            let vy0 = (self.header_h.max(0.0) as usize).min(win_h);
            let vx1 = win_w.saturating_sub(self.right_panel_w.max(0.0) as usize).max(vx0);
            let vy1 = win_h.saturating_sub(bottom as usize).max(vy0);
            [vx0, vy0, vx1, vy1]
        } else {
            [0, 0, win_w, win_h]
        }
    }

    pub fn viewport_center(&self, win_w: f32, win_h: f32) -> Vec2 {
        Vec2::new(win_w * 0.5, win_h * 0.5) + self.pan + self.viewport_center_offset()
    }

    /// The visible slice of the canvas in **normalised canvas coordinates** (0..1 per axis).
    ///
    /// Derived from the same `viewport_rect` and `viewport_center` the renderer clips and
    /// maps with, so the navigator frames exactly what is on screen. Values are returned
    /// unclamped: a viewport larger than the canvas legitimately reports outside 0..1, and
    /// the caller clamps only when drawing.
    pub fn viewport_canvas_fraction(&self, win_w: usize, win_h: usize) -> (f32, f32, f32, f32) {
        let [x0, y0, x1, y1] = self.viewport_rect(win_w, win_h);
        let center = self.viewport_center(win_w as f32, win_h as f32);
        let zoom = self.zoom.max(0.0001);
        let doc_w = (self.document.width.max(1)) as f32;
        let doc_h = (self.document.height.max(1)) as f32;

        // Renderer mapping: canvas = (screen - center) / zoom + doc / 2.
        let axis = |screen: f32, center: f32, extent: f32| {
            ((screen - center) / zoom + extent * 0.5) / extent
        };
        (
            axis(x0 as f32, center.x, doc_w),
            axis(y0 as f32, center.y, doc_h),
            axis(x1 as f32, center.x, doc_w),
            axis(y1 as f32, center.y, doc_h),
        )
    }

    /// The zoom at which the document exactly fits the visible viewport.
    ///
    /// Uses `viewport_rect`, not the window: "Fit" used to subtract a magic margin from the
    /// whole window and then scale by 0.92, which left the canvas smaller than the space it
    /// had and off-centre whenever the timeline was open.
    pub fn fit_zoom(&self, win_w: f32, win_h: f32) -> f32 {
        let [x0, y0, x1, y1] = self.viewport_rect(win_w as usize, win_h as usize);
        let available_w = (x1.saturating_sub(x0)).max(1) as f32;
        let available_h = (y1.saturating_sub(y0)).max(1) as f32;
        let doc_w = (self.document.width.max(1)) as f32;
        let doc_h = (self.document.height.max(1)) as f32;
        (available_w / doc_w).min(available_h / doc_h).clamp(0.05, 8.0)
    }

    pub fn reset_view_centered(&mut self, win_w: f32, win_h: f32) {
        self.pan = Vec2::ZERO;
        self.zoom = self.fit_zoom(win_w, win_h);
        self.set_status(format!("Fit to screen ({:.0}%)", self.zoom * 100.0));
    }

    /// Centres the view on a canvas point.
    ///
    /// `viewport_center()` already carries the panel offset, so shifting `pan` by the target's
    /// distance from the document centre puts that point at the viewport's midpoint.
    pub fn pan_to_canvas_center(&mut self, target_canvas_pos: Vec2) {
        let canvas_size = Vec2::new(self.document.width as f32, self.document.height as f32);
        self.pan = -(target_canvas_pos - canvas_size * 0.5) * self.zoom;
    }

    /// Pans by a screen-space delta, in physical pixels.
    ///
    /// Used by the navigator's drag: dragging the viewport box right moves the view right,
    /// which moves the document left on screen.
    pub fn pan_by_screen_delta(&mut self, delta: Vec2) {
        self.pan -= delta;
    }

    /// Changes zoom while keeping the canvas point under `anchor` (physical pixels) fixed.
    ///
    /// Without this a wheel zoom drifts away from the cursor: the anchor has to be resolved
    /// against the *old* zoom before the scale changes.
    pub fn zoom_at(&mut self, new_zoom: f32, anchor: Vec2, win_w: f32, win_h: f32) {
        let new_zoom = new_zoom.clamp(0.05, 8.0);
        let offset = self.viewport_center_offset();
        let origin = Vec2::new(win_w * 0.5, win_h * 0.5) + offset;
        let center = origin + self.pan;
        let ratio = new_zoom / self.zoom.max(0.0001);
        let center_after = anchor - (anchor - center) * ratio;
        self.pan = center_after - origin;
        self.zoom = new_zoom;
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

    /// Canvas position -> egui point-space position, for painting overlays with egui.
    /// `win_w`/`win_h` are the egui screen size in points.
    pub fn canvas_to_ui(&self, canvas_pos: Vec2, win_w: f32, win_h: f32) -> Vec2 {
        let ppp = self.ui_ppp.max(0.01);
        self.canvas_to_screen(canvas_pos, win_w * ppp, win_h * ppp) / ppp
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

    /// The active layer's storage format, or the document's when nothing is active.
    pub fn active_layer_format(&self) -> PixelFormat {
        self.document
            .active_layer()
            .map(|layer| layer.format())
            .unwrap_or(self.document.pixel_format)
    }

    /// True when the active layer can be written through an 8-bit-only operation.
    ///
    /// On refusal it sets a status naming the operation and the document's format, and the
    /// caller must not touch the layer. This is the guard that keeps #13 from exposing the
    /// 8-bit-only paths as panics.
    pub fn require_8bit(&mut self, operation: &str) -> bool {
        if self.active_layer_format() == PixelFormat::Srgb8 {
            return true;
        }
        self.set_status(format!(
            "{operation} is 8-bit only; this document is 16-bit sRGB"
        ));
        false
    }

    /// A short label for the document's storage format, for the footer and the layer panel.
    pub fn document_format_label(&self) -> &'static str {
        match self.document.pixel_format {
            PixelFormat::Srgb8 => "RGBA8",
            PixelFormat::Srgb16 => "RGBA16",
        }
    }

    /// Backs up the active layer's pixels before opening a filter dialog
    pub fn begin_filter_modal(&mut self, modal: ActiveFilterModal) {
        if let Some(layer) = self.document.active_layer() {
            self.filter_original_pixels = Some(layer.storage_bytes());
            self.active_filter_modal = modal;
            self.filter_preview_active = true;
        }
    }

    /// Cancels filter modal and restores original layer pixels
    pub fn cancel_filter_modal(&mut self) {
        if let Some(orig) = self.filter_original_pixels.take() {
            if let Some(layer) = self.document.active_layer_mut() {
                layer.write_storage_bytes(0, &orig);
            }
        }
        self.active_filter_modal = ActiveFilterModal::None;
    }

    /// Commits the filter modal changes and registers to undo history
    pub fn apply_filter_modal(&mut self, filter_name: &'static str) {
        if let Some(before) = self.filter_original_pixels.take() {
            if let Some(layer) = self.document.active_layer() {
                let after = layer.storage_bytes();
                if before != after {
                    let cmd = Box::new(aurelia_core::history::LayerPixelsDeltaCommand::new(layer.id, filter_name, before, after));
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
                            if layer.as_srgb8_exact()[idx + 3] > 0 {
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
                        if layer.as_srgb8_exact()[idx + 3] > 0 {
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
                        patch[dst_idx..dst_idx + 4].copy_from_slice(&layer.as_srgb8_exact()[src_idx..src_idx + 4]);
                    }
                }
            }

            let patch_origin = Vec2::new(min_x as f32, min_y as f32);
            let center = patch_origin + Vec2::new(patch_w as f32 * 0.5, patch_h as f32 * 0.5);

            let mut transform = AffineTransform2D::new(center);
            transform.pivot = center;

            let quad_src = [
                patch_origin,
                patch_origin + Vec2::new(patch_w as f32, 0.0),
                patch_origin + Vec2::new(patch_w as f32, patch_h as f32),
                patch_origin + Vec2::new(0.0, patch_h as f32),
            ];
            let quad = PerspectiveQuadTransform::new(quad_src, quad_src);

            let rows = 4;
            let cols = 4;
            let mesh_grid = MeshWarpGrid::new(rows, cols, patch_origin, patch_w as f32, patch_h as f32);

            let tps_source_pins = vec![
                patch_origin,
                patch_origin + Vec2::new(patch_w as f32, 0.0),
                patch_origin + Vec2::new(patch_w as f32, patch_h as f32),
                patch_origin + Vec2::new(0.0, patch_h as f32),
                center,
            ];
            let tps_target_pins = tps_source_pins.clone();
            let tps = ThinPlateSpline2D::solve(&tps_target_pins, &tps_source_pins);

            self.transform_session = TransformSession {
                is_active: true,
                mode: TransformMode::Affine,
                original_layer_pixels: layer.as_srgb8_exact().to_vec(),
                extracted_patch: patch,
                patch_w,
                patch_h,
                patch_origin,
                transform,
                quad,
                mesh_grid,
                tps,
                tps_source_pins,
                tps_target_pins,
                interpolation: InterpolationMode::Bicubic,
                active_handle: None,
                drag_start_canvas_pos: Vec2::ZERO,
                initial_transform: transform,
                lock_aspect: false,
                grid_rows: rows,
                grid_cols: cols,
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
                            layer_mut.as_srgb8_exact_mut()[idx] = 0;
                            layer_mut.as_srgb8_exact_mut()[idx + 1] = 0;
                            layer_mut.as_srgb8_exact_mut()[idx + 2] = 0;
                            layer_mut.as_srgb8_exact_mut()[idx + 3] = 0;
                        }
                    }
                }
            }

            self.update_transform_preview();
            self.set_status("Free Transform & Warp: Drag handles / mesh pins | Enter to Apply, Esc to Cancel");
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
        let mode = session.mode;
        let transform = session.transform;
        let quad = session.quad;
        let mesh_grid = &session.mesh_grid;
        let tps = &session.tps;
        let interp = session.interpolation;
        let sel_opt = self.selection.as_ref();

        if let Some(layer) = self.document.active_layer_mut() {
            // Start from original pixels with source patch cleared
            layer.as_srgb8_exact_mut().copy_from_slice(orig);
            for py in 0..ph {
                let sy = origin.y as u32 + py;
                for px in 0..pw {
                    let sx = origin.x as u32 + px;
                    if sx < doc_w && sy < doc_h {
                        let should_clear = sel_opt.map_or(true, |s| s.is_selected(sx, sy));
                        if should_clear {
                            let idx = (sy * doc_w + sx) as usize * 4;
                            layer.as_srgb8_exact_mut()[idx] = 0;
                            layer.as_srgb8_exact_mut()[idx + 1] = 0;
                            layer.as_srgb8_exact_mut()[idx + 2] = 0;
                            layer.as_srgb8_exact_mut()[idx + 3] = 0;
                        }
                    }
                }
            }

            match mode {
                TransformMode::Affine => {
                    render_transformed_patch(
                        patch,
                        pw,
                        ph,
                        origin,
                        &transform,
                        interp,
                        layer.as_srgb8_exact_mut(),
                        doc_w,
                        doc_h,
                    );
                }
                TransformMode::PerspectiveQuad => {
                    render_quad_transformed_patch(
                        patch,
                        pw,
                        ph,
                        &quad,
                        interp,
                        layer.as_srgb8_exact_mut(),
                        doc_w,
                        doc_h,
                    );
                }
                TransformMode::MeshGrid => {
                    render_mesh_warped_patch(
                        patch,
                        pw,
                        ph,
                        mesh_grid,
                        interp,
                        layer.as_srgb8_exact_mut(),
                        doc_w,
                        doc_h,
                    );
                }
                TransformMode::ThinPlateSpline => {
                    render_tps_warped_patch(
                        patch,
                        pw,
                        ph,
                        origin,
                        tps,
                        interp,
                        layer.as_srgb8_exact_mut(),
                        doc_w,
                        doc_h,
                    );
                }
            }
        }
    }

    /// Sets transform deformation mode and synchronizes coordinates
    pub fn set_transform_mode(&mut self, mode: TransformMode) {
        if !self.transform_session.is_active {
            return;
        }
        self.transform_session.mode = mode;
        self.transform_session.active_handle = None;
        self.update_transform_preview();
        let mode_label = match mode {
            TransformMode::Affine => "Affine Free Transform (Box)",
            TransformMode::PerspectiveQuad => "Perspective Quad Distortion",
            TransformMode::MeshGrid => "Bicubic Mesh Grid Warp",
            TransformMode::ThinPlateSpline => "Thin Plate Spline (TPS) Landmark Warping",
        };
        self.set_status(format!("Active Transform Mode: {}", mode_label));
    }

    /// Resizes Mesh Grid density (e.g. 3x3, 4x4, 5x5, 8x8)
    pub fn set_transform_grid_density(&mut self, rows: usize, cols: usize) {
        if !self.transform_session.is_active {
            return;
        }
        self.transform_session.grid_rows = rows;
        self.transform_session.grid_cols = cols;
        self.transform_session.mesh_grid.resize_density(rows, cols);
        self.update_transform_preview();
        self.set_status(format!("Set Mesh Grid Density to {}×{}", cols, rows));
    }

    /// Resets current mode's deformation back to original unwarped state
    pub fn reset_transform_deformation(&mut self) {
        if !self.transform_session.is_active {
            return;
        }
        let origin = self.transform_session.patch_origin;
        let pw = self.transform_session.patch_w as f32;
        let ph = self.transform_session.patch_h as f32;
        let center = origin + Vec2::new(pw * 0.5, ph * 0.5);

        match self.transform_session.mode {
            TransformMode::Affine => {
                self.transform_session.transform = AffineTransform2D::new(center);
            }
            TransformMode::PerspectiveQuad => {
                let quad_src = [
                    origin,
                    origin + Vec2::new(pw, 0.0),
                    origin + Vec2::new(pw, ph),
                    origin + Vec2::new(0.0, ph),
                ];
                self.transform_session.quad = PerspectiveQuadTransform::new(quad_src, quad_src);
            }
            TransformMode::MeshGrid => {
                self.transform_session.mesh_grid.reset_grid();
            }
            TransformMode::ThinPlateSpline => {
                self.transform_session.tps_target_pins = self.transform_session.tps_source_pins.clone();
                self.transform_session.tps = ThinPlateSpline2D::solve(
                    &self.transform_session.tps_target_pins,
                    &self.transform_session.tps_source_pins,
                );
            }
        }
        self.update_transform_preview();
        self.set_status("Reset active transform deformation");
    }

    /// Adds a new TPS landmark pin at canvas position
    pub fn add_tps_pin(&mut self, canvas_pos: Vec2) {
        if !self.transform_session.is_active {
            return;
        }
        self.transform_session.tps_source_pins.push(canvas_pos);
        self.transform_session.tps_target_pins.push(canvas_pos);
        self.transform_session.tps = ThinPlateSpline2D::solve(
            &self.transform_session.tps_target_pins,
            &self.transform_session.tps_source_pins,
        );
        self.update_transform_preview();
        self.set_status(format!("Added TPS pin ({:.0}, {:.0})", canvas_pos.x, canvas_pos.y));
    }

    /// Commits the transformation to layer and registers in history
    pub fn commit_transform_session(&mut self) {
        if !self.transform_session.is_active {
            return;
        }
        self.update_transform_preview();
        if let Some(layer) = self.document.active_layer() {
            let before = self.transform_session.original_layer_pixels.clone();
            let after = layer.as_srgb8_exact().to_vec();
            if before != after {
                let mode_name = match self.transform_session.mode {
                    TransformMode::Affine => "Free Transform",
                    TransformMode::PerspectiveQuad => "Perspective Quad Warp",
                    TransformMode::MeshGrid => "Mesh Grid Warp",
                    TransformMode::ThinPlateSpline => "TPS Landmark Warp",
                };
                let cmd = Box::new(aurelia_core::history::LayerPixelsDeltaCommand::new(layer.id, mode_name, before, after));
                self.history.push(cmd);
                self.set_status(format!("Committed {}", mode_name));
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
            layer.set_pixels(std::mem::take(&mut self.transform_session.original_layer_pixels));
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
                let before = layer.as_srgb8_exact().to_vec();
                mask.fill_selection(layer.as_srgb8_exact_mut(), doc_w, doc_h, color);
                let after = layer.as_srgb8_exact().to_vec();
                if before != after {
                    let cmd = Box::new(aurelia_core::history::LayerPixelsDeltaCommand::new(layer.id, "Fill Selection", before, after));
                    self.history.push(cmd);
                    self.set_status("Filled selection with primary color");
                }
            }
        }
    }

    pub fn clear_selection_active_layer(&mut self) {
        if let Some(mask) = &self.selection {
            let doc_w = self.document.width;
            let doc_h = self.document.height;
            let color = [0, 0, 0, 0];
            if let Some(layer) = self.document.active_layer_mut() {
                let before = layer.as_srgb8_exact().to_vec();
                mask.fill_selection(layer.as_srgb8_exact_mut(), doc_w, doc_h, color);
                let after = layer.as_srgb8_exact().to_vec();
                if before != after {
                    let cmd = Box::new(aurelia_core::history::LayerPixelsDeltaCommand::new(layer.id, "Clear Selection", before, after));
                    self.history.push(cmd);
                    self.set_status("Cleared selected pixels");
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
                let before = layer.as_srgb8_exact().to_vec();
                mask.stroke_selection(layer.as_srgb8_exact_mut(), doc_w, doc_h, color, width, pos);
                let after = layer.as_srgb8_exact().to_vec();
                if before != after {
                    let cmd = Box::new(aurelia_core::history::LayerPixelsDeltaCommand::new(layer.id, "Stroke Selection", before, after));
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
                                let src = aurelia_core::rasterizer::sample_bilinear_rgba(&self.scratchpad_pixels, w, h, sx, sy);
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
    // ─────────────────────────────────────────────────────────────────────
    // Document lifecycle & save safety
    // ─────────────────────────────────────────────────────────────────────

    /// True when the document has edits that are not on disk yet.
    ///
    /// Derived from the history generation counter, so it covers strokes,
    /// filters, layer operations and undo/redo without touching every call site.
    pub fn is_dirty(&self) -> bool {
        self.history.generation() != self.saved_generation
    }

    /// Records the current history generation as "written to disk".
    pub fn mark_saved(&mut self) {
        self.saved_generation = self.history.generation();
        self.autosave_pending = false;
    }

    /// File name (or `Untitled.aur`) with a trailing `*` when there are unsaved edits.
    pub fn document_title(&self) -> String {
        let name = self
            .document_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled.aur")
            .to_string();
        if self.is_dirty() {
            format!("{} *", name)
        } else {
            name
        }
    }

    pub fn note_recent_file(&mut self, path: &std::path::Path) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_path_buf());
        self.recent_files.truncate(8);
    }

    /// Saves to an explicit path and updates path/recent/preferences bookkeeping.
    pub fn save_project_to(&mut self, path: &std::path::Path) -> Result<(), String> {
        // Snapshot the recovery path before the document gains its real name, so an
        // untitled document's snapshot is also cleaned up after "Save As".
        let previous_autosave = self.autosave_path();
        aurelia_io::save_project_file(&self.document, path).map_err(|e| e.to_string())?;
        self.document_path = Some(path.to_path_buf());
        self.mark_saved();
        let _ = std::fs::remove_file(&previous_autosave);
        self.note_recent_file(path);
        self.save_preferences();
        self.clear_autosave();
        self.set_status(format!("Saved to {}", path.display()));
        Ok(())
    }

    /// Forces the dirty state (used when restoring a crash-recovery snapshot).
    pub fn mark_dirty(&mut self) {
        self.saved_generation = u64::MAX;
    }

    /// Saves to the already-known path without prompting. False when there is no path yet.
    pub fn quick_save(&mut self) -> bool {
        match self.document_path.clone() {
            Some(path) => self.save_project_to(&path).is_ok(),
            None => false,
        }
    }

    /// `%LOCALAPPDATA%\Aurelia\autosave` (temp dir fallback).
    pub fn autosave_dir() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join("Aurelia").join("autosave")
    }

    /// `%LOCALAPPDATA%\Aurelia\autosave\<stem>-autosave.aur`.
    pub fn autosave_path(&self) -> PathBuf {
        let stem = self
            .document_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        Self::autosave_dir().join(format!("{}-autosave.aur", stem))
    }

    /// Writes a recovery snapshot if the interval elapsed and there are unsaved edits.
    pub fn maybe_autosave(&mut self) -> bool {
        if !self.autosave_enabled || !self.is_dirty() {
            return false;
        }
        if self.last_autosave.elapsed().as_secs() < self.autosave_interval_secs {
            return false;
        }
        self.last_autosave = Instant::now();
        let path = self.autosave_path();
        if let Some(dir) = path.parent() {
            if std::fs::create_dir_all(dir).is_err() {
                return false;
            }
        }
        if aurelia_io::save_project_file(&self.document, &path).is_ok() {
            self.autosave_pending = true;
            self.set_status(format!("Auto-saved recovery snapshot: {}", path.display()));
            true
        } else {
            false
        }
    }

    /// Deletes the recovery snapshot after a successful real save.
    pub fn clear_autosave(&mut self) {
        let path = self.autosave_path();
        let _ = std::fs::remove_file(&path);
        self.autosave_pending = false;
    }
    // ─────────────────────────────────────────────────────────────────────
    // Preferences (theme / JPEG quality / autosave / recent files)
    // ─────────────────────────────────────────────────────────────────────

    pub fn preferences_path() -> PathBuf {
        Self::autosave_dir()
            .parent()
            .map(|d| d.join("preferences.txt"))
            .unwrap_or_else(|| std::env::temp_dir().join("aurelia-preferences.txt"))
    }

    pub fn load_preferences(&mut self) {
        let Ok(text) = std::fs::read_to_string(Self::preferences_path()) else {
            return;
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "theme" => {
                    self.document.theme = match value {
                        "1" => aurelia_core::color::ThemeMode::Dusk,
                        "2" => aurelia_core::color::ThemeMode::Ember,
                        _ => aurelia_core::color::ThemeMode::Dawn,
                    };
                }
                "jpeg_quality" => {
                    if let Ok(q) = value.parse::<u8>() {
                        self.jpeg_quality = q.clamp(1, 100);
                    }
                }
                "autosave" => self.autosave_enabled = value != "0",
                "discord_presence" => self.discord_presence = value == "1",
                "discord_show_project" => self.discord_show_project_name = value == "1",
                "autosave_interval" => {
                    if let Ok(secs) = value.parse::<u64>() {
                        self.autosave_interval_secs = secs.clamp(15, 3600);
                    }
                }
                "recent" => {
                    if !value.is_empty() {
                        self.recent_files.push(PathBuf::from(value));
                    }
                }
                _ => {}
            }
        }
    }

    pub fn save_preferences(&self) {
        let path = Self::preferences_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let theme = match self.document.theme {
            aurelia_core::color::ThemeMode::Dusk => 1,
            aurelia_core::color::ThemeMode::Ember => 2,
            _ => 0,
        };
        let mut out = String::new();
        out.push_str(&format!("theme={}\n", theme));
        out.push_str(&format!("jpeg_quality={}\n", self.jpeg_quality));
        out.push_str(&format!("autosave={}\n", if self.autosave_enabled { 1 } else { 0 }));
        out.push_str(&format!(
            "discord_presence={}\n",
            if self.discord_presence { 1 } else { 0 }
        ));
        out.push_str(&format!(
            "discord_show_project={}\n",
            if self.discord_show_project_name { 1 } else { 0 }
        ));
        out.push_str(&format!("autosave_interval={}\n", self.autosave_interval_secs));
        for recent in &self.recent_files {
            out.push_str(&format!("recent={}\n", recent.display()));
        }
        let _ = std::fs::write(&path, out);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(1920, 1080)
    }
}

#[cfg(test)]
mod viewport_geometry {
    use super::AppState;

    /// Panel-free window so the viewport is the whole window, which makes the numbers
    /// checkable by hand.
    fn full_window(doc_w: u32, doc_h: u32) -> AppState {
        let mut state = AppState::new(doc_w, doc_h);
        state.show_ui_panels = false;
        state.show_rulers = false;
        state
    }

    /// The renderer's inverse mapping, for asserting what is under a screen point.
    fn canvas_under(state: &AppState, screen: glam::Vec2, win_w: f32, win_h: f32) -> glam::Vec2 {
        let center = state.viewport_center(win_w, win_h);
        let doc = glam::Vec2::new(state.document.width as f32, state.document.height as f32);
        (screen - center) / state.zoom + doc * 0.5
    }

    /// The framing box must be derived from the same geometry `render_canvas` clips with.
    #[test]
    fn navigator_framing_box_matches_the_renderer_viewport() {
        let mut state = full_window(100, 100);

        // 2x zoom in a window twice the document: exactly the whole canvas is visible.
        state.zoom = 2.0;
        state.pan = glam::Vec2::ZERO;
        let (x0, y0, x1, y1) = state.viewport_canvas_fraction(200, 200);
        assert!((x0 - 0.0).abs() < 1e-4 && (y0 - 0.0).abs() < 1e-4, "top-left: {x0},{y0}");
        assert!((x1 - 1.0).abs() < 1e-4 && (y1 - 1.0).abs() < 1e-4, "bottom-right: {x1},{y1}");

        // 1x zoom: the window shows twice the canvas, centred, so it extends half a canvas
        // beyond each edge. The function reports this unclamped; the caller clamps to draw.
        state.zoom = 1.0;
        let (x0, y0, x1, y1) = state.viewport_canvas_fraction(200, 200);
        assert!((x0 + 0.5).abs() < 1e-4, "left {x0}");
        assert!((y0 + 0.5).abs() < 1e-4, "top {y0}");
        assert!((x1 - 1.5).abs() < 1e-4, "right {x1}");
        assert!((y1 - 1.5).abs() < 1e-4, "bottom {y1}");

        // Panning right moves the visible slice right.
        state.pan = glam::Vec2::new(10.0, 0.0);
        let (x0, _, x1, _) = state.viewport_canvas_fraction(200, 200);
        assert!((x1 - x0 - 2.0).abs() < 1e-4, "the box keeps its width");
        assert!(x0 < -0.5, "panning right must move the box left: {x0}");
    }

    /// "Fit" must target the viewport rect, not the window minus a fudge factor.
    #[test]
    fn fit_to_viewport_is_exact() {
        let mut state = full_window(100, 50);
        // Window 200x200, document 100x50: the limiting axis is width, 200/100 = 2.
        assert!((state.fit_zoom(200.0, 200.0) - 2.0).abs() < 1e-5);

        // A tall document is limited by height.
        let tall = full_window(50, 400);
        assert!((tall.fit_zoom(200.0, 200.0) - 0.5).abs() < 1e-5);

        // With panels the available rect shrinks, and the fit follows it exactly.
        state.show_ui_panels = true;
        state.left_panel_w = 40.0;
        state.right_panel_w = 60.0;
        state.header_h = 20.0;
        state.footer_h = 30.0;
        let [x0, y0, x1, y1] = state.viewport_rect(400, 300);
        let expected = ((x1 - x0) as f32 / 100.0).min((y1 - y0) as f32 / 50.0);
        assert!((state.fit_zoom(400.0, 300.0) - expected).abs() < 1e-5);

        // And reset_view_centered applies it and re-centres.
        state.pan = glam::Vec2::new(123.0, 45.0);
        state.reset_view_centered(400.0, 300.0);
        assert!(state.pan == glam::Vec2::ZERO, "fit must re-centre");
        assert!((state.zoom - expected).abs() < 1e-5);
    }

    /// A wheel zoom must keep the canvas point under the cursor where it is.
    #[test]
    fn zoom_at_keeps_the_anchor_pixel_fixed() {
        let mut state = full_window(400, 300);
        state.zoom = 1.0;
        state.pan = glam::Vec2::new(17.0, -9.0);
        let anchor = glam::Vec2::new(137.0, 88.0);
        let before = canvas_under(&state, anchor, 800.0, 600.0);

        state.zoom_at(2.5, anchor, 800.0, 600.0);

        assert!((state.zoom - 2.5).abs() < 1e-6);
        let after = canvas_under(&state, anchor, 800.0, 600.0);
        assert!(
            (after - before).length() < 1e-3,
            "the point under the cursor moved: {before:?} -> {after:?}"
        );
    }

    /// Dragging the navigator box by a screen delta moves the view by that delta.
    #[test]
    fn drag_carries_the_viewport_box() {
        let mut state = full_window(100, 100);
        state.zoom = 2.0;
        let before = state.viewport_canvas_fraction(200, 200);

        state.pan_by_screen_delta(glam::Vec2::new(20.0, 0.0));

        let after = state.viewport_canvas_fraction(200, 200);
        // 20 screen px at 2x is 0.1 of a 100 px canvas, and dragging the box right moves the
        // visible slice right.
        assert!((after.0 - before.0 - 0.1).abs() < 1e-4, "{} -> {}", before.0, after.0);
        assert!((after.2 - before.2 - 0.1).abs() < 1e-4);
        assert!((state.pan.x + 20.0).abs() < 1e-4, "pan must move opposite the drag");
    }

    /// The thumbnail is rebuilt on content changes only - never on an idle frame, and never
    /// merely because the user panned or zoomed.
    #[test]
    fn idle_frame_rebuilds_nothing() {
        let mut state = full_window(64, 64);
        assert!(state.navigator_needs_refresh(), "the first frame needs a thumbnail");

        state.note_navigator_refreshed();
        assert!(!state.navigator_needs_refresh(), "an idle frame must not rebuild");

        state.pan = glam::Vec2::new(40.0, 12.0);
        state.zoom = 3.0;
        assert!(!state.navigator_needs_refresh(), "pan and zoom do not change the document");

        state.mark_canvas_dirty();
        assert!(state.navigator_needs_refresh(), "an edit must rebuild");
        state.note_navigator_refreshed();

        state.document.width = 128;
        assert!(
            state.navigator_needs_refresh(),
            "a size change must rebuild even without an edit"
        );
    }

    /// Clicking the navigator puts that canvas point at the viewport's midpoint. The midpoint
    /// is `viewport_center`'s origin - it already accounts for asymmetric panels - so the test
    /// resolves the canvas point under it rather than assuming the window centre.
    #[test]
    fn pan_to_canvas_center_accounts_for_the_viewport_offset() {
        let mut state = AppState::new(100, 100);
        state.show_ui_panels = true;
        state.left_panel_w = 40.0;
        state.right_panel_w = 60.0;
        state.header_h = 20.0;
        state.footer_h = 30.0;
        state.zoom = 2.0;

        let target = glam::Vec2::new(25.0, 75.0);
        state.pan_to_canvas_center(target);

        let (win_w, win_h) = (400.0_f32, 300.0_f32);
        let viewport_midpoint = glam::Vec2::new(win_w * 0.5, win_h * 0.5) + state.viewport_center_offset();
        let doc = glam::Vec2::new(100.0, 100.0);
        let canvas_at_midpoint =
            (viewport_midpoint - state.viewport_center(win_w, win_h)) / state.zoom + doc * 0.5;

        assert!(
            (canvas_at_midpoint - target).length() < 1e-3,
            "expected {target:?} at the viewport midpoint, got {canvas_at_midpoint:?}"
        );
    }
}

#[cfg(test)]
mod discord_presence_defaults {
    use super::AppState;

    #[test]
    fn presence_is_off_and_project_name_is_private_on_first_run() {
        let state = AppState::default();
        assert!(!state.discord_presence);
        assert!(!state.discord_show_project_name);
        assert!(!state.show_discord_presence_dialog);
    }
}


#[cfg(test)]
mod document_format_ui {
    use super::*;

    fn sixteen_bit_state() -> AppState {
        let mut state = AppState::new(4, 4);
        state.document = Document::new_with_format(4, 4, PixelFormat::Srgb16);
        state
    }

    #[test]
    fn the_footer_label_reports_the_document_format() {
        let eight = AppState::new(2, 2);
        assert_eq!(eight.document_format_label(), "RGBA8");

        let wide = sixteen_bit_state();
        assert_eq!(wide.document_format_label(), "RGBA16");
    }

    /// The guard still reports rather than silently doing nothing, for any future 8-bit-only
    /// operation, and filters now open on a 16-bit document.
    #[test]
    fn the_eight_bit_guard_still_reports_and_filters_open_on_sixteen_bit() {
        let mut wide = sixteen_bit_state();
        assert!(!wide.require_8bit("Something New"), "the guard must refuse");
        let message = wide.status_message.clone();
        assert!(message.contains("8-bit only"), "status should explain the refusal: {message}");
        assert!(message.contains("16-bit"), "status should name the document format: {message}");

        assert!(AppState::new(4, 4).require_8bit("Something New"));

        // Filters are no longer 8-bit only: the modal opens and captures storage bytes, so the
        // preview can restore a 16-bit layer exactly.
        wide.begin_filter_modal(ActiveFilterModal::BrightnessContrast);
        assert!(wide.active_filter_modal == ActiveFilterModal::BrightnessContrast);
        assert_eq!(
            wide.filter_original_pixels.as_ref().map(|bytes| bytes.len()),
            Some((4 * 4 * 4 * 2) as usize),
            "the preview buffer must be storage bytes, not an 8-bit view"
        );
    }

    /// Saving and reopening a 16-bit document keeps its native data and its format, through the
    /// same `AppState` entry point the editor uses.
    #[test]
    fn save_and_reopen_preserves_native_sixteen_bit_data() {
        use aurelia_core::layer::LayerBuffer;

        let mut state = sixteen_bit_state();
        let id = state.document.active_layer_id;
        let low = aurelia_core::pixel::srgb8_to_srgb16(90);
        let high = low + 128; // off-lattice
        if let Some(layer) = state.document.get_layer_mut(id) {
            // 4x4 pixels, four channels each.
            let mut channels = vec![0u16; 4 * 4 * 4];
            for pixel in channels.chunks_exact_mut(4) {
                pixel[0] = high;
                pixel[3] = 65535;
            }
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let dir = std::env::temp_dir().join(format!("aurelia-format-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("wide.aur");

        state.save_project_to(&path).expect("save");
        let loaded = aurelia_io::load_project_file(&path).expect("load");

        assert_eq!(loaded.pixel_format, PixelFormat::Srgb16, "format must survive");
        let layer = loaded.get_layer(id).expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "storage must stay 16-bit");
        assert_eq!(
            layer.as_srgb16().expect("16-bit")[0],
            high,
            "an off-lattice channel must come back exactly"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
