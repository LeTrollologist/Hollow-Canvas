use glam::Vec2;
use std::path::PathBuf;
use std::time::Instant;
use aurelia_core::blend::BlendMode;
use aurelia_core::brush::BrushSettings;
use aurelia_core::color::{Color, DEFAULT_PALETTE};
use aurelia_core::document::{
    Document, EditorState, GuideLine, StraightRulerState,
};
use aurelia_core::history::HistoryStack;
use aurelia_core::layer::{Layer, LayerBuffer, PixelFormat};
use aurelia_core::selection::{SelectionCombineMode, SelectionMask, StrokePosition};
use aurelia_core::symmetry::SymmetryConfig;
use aurelia_core::transform::{
    AffineTransform2D, InterpolationMode, MeshWarpGrid, PerspectiveQuadTransform, RenderMode,
    ThinPlateSpline2D, TransformMode, TransformPatch,
};

/// One committed selection change, retained so Ctrl+Z can restore the previous mask (#36).
#[derive(Debug, Clone)]
pub struct SelectionEdit {
    pub before: Option<SelectionMask>,
    pub after: Option<SelectionMask>,
}

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
    pub extracted_patch: TransformPatch,
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
            extracted_patch: TransformPatch::Srgb8(Vec::new()),
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

/// Whether a layer has a non-zero alpha channel at a canvas coordinate.
///
/// The transform session bounds its source rectangle by content alpha. Reading the native
/// buffer keeps that scan working at both storage widths; the patch extraction and rendering
/// that follow are what carry the pixels themselves.
fn layer_has_alpha(layer: &Layer, x: u32, y: u32) -> bool {
    let Some(index) = layer.pixel_index(x, y) else {
        return false;
    };
    match layer.buffer() {
        LayerBuffer::Srgb8(bytes) => bytes.get(index + 3).is_some_and(|&alpha| alpha > 0),
        LayerBuffer::Srgb16(channels) => channels.get(index + 3).is_some_and(|&alpha| alpha > 0),
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

/// Inclusive bounds for the values the Preferences dialog accepts.
pub const AUTOSAVE_INTERVAL_RANGE: (u64, u64) = (15, 3600);
pub const UNDO_BUDGET_MB_RANGE: (u32, u32) = (16, 4096);

/// A working copy of the persisted editor preferences.
///
/// The dialog edits this, validates it, and only then writes it back to [`AppState`], so a
/// cancelled or invalid edit cannot leave half-applied settings behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferencesDraft {
    pub autosave_enabled: bool,
    pub autosave_interval_secs: u64,
    pub undo_budget_mb: u32,
    pub jpeg_quality: u8,
    pub theme: aurelia_core::color::ThemeMode,
}

impl Default for PreferencesDraft {
    fn default() -> Self {
        Self {
            autosave_enabled: true,
            autosave_interval_secs: 120,
            undo_budget_mb: (HistoryStack::DEFAULT_MAX_BYTES / (1024 * 1024)) as u32,
            jpeg_quality: 90,
            theme: aurelia_core::color::ThemeMode::Dawn,
        }
    }
}

impl PreferencesDraft {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            autosave_enabled: state.autosave_enabled,
            autosave_interval_secs: state.autosave_interval_secs,
            undo_budget_mb: state.undo_budget_mb,
            jpeg_quality: state.jpeg_quality,
            theme: state.document.theme,
        }
    }

    /// Rejects out-of-range values with a message naming the field and its accepted range.
    /// Every control already clamps, so this guards text entry, future controls and tests.
    pub fn validate(&self) -> Result<(), String> {
        let (min_interval, max_interval) = AUTOSAVE_INTERVAL_RANGE;
        if !(min_interval..=max_interval).contains(&self.autosave_interval_secs) {
            return Err(format!(
                "Autosave interval must be between {min_interval} and {max_interval} seconds."
            ));
        }
        let (min_budget, max_budget) = UNDO_BUDGET_MB_RANGE;
        if !(min_budget..=max_budget).contains(&self.undo_budget_mb) {
            return Err(format!(
                "Undo budget must be between {min_budget} and {max_budget} MiB."
            ));
        }
        if !(1..=100).contains(&self.jpeg_quality) {
            return Err("JPEG quality must be between 1 and 100.".to_string());
        }
        Ok(())
    }
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

/// A persisted measurement guide in canvas coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideOrientation {
    Horizontal,
    Vertical,
}

/// What the Move tool acts on when the user starts a drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMode {
    /// Move only the active layer, even if another layer is under the cursor.
    SelectedLayer,
    /// Select and move the topmost visible layer under the cursor.
    FullMove,
}

/// A guide line currently being used as a snap target, for the overlay indicator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapMarker {
    pub vertical: bool,
    pub position: f32,
}

/// A movable, rotatable straight-edge measurement ruler in canvas coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StraightRuler {
    pub origin: Vec2,
    pub angle_rad: f32,
    pub length: f32,
}

impl StraightRuler {
    /// Projects a canvas point onto the ruler's centerline, clamped to its endpoints.
    pub fn constrain(&self, point: Vec2) -> Vec2 {
        let dir = Vec2::new(self.angle_rad.cos(), self.angle_rad.sin());
        let along = (point - self.origin).dot(dir).clamp(0.0, self.length);
        self.origin + dir * along
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guide {
    pub orientation: GuideOrientation,
    /// Canvas-space coordinate: y for horizontal, x for vertical.
    pub position: f32,
}

pub struct AppState {
    pub document: Document,
    pub history: HistoryStack,
    pub brush: BrushSettings,
    pub symmetry: SymmetryConfig,
    pub selection: Option<SelectionMask>,
    /// How the next selection region combines with the existing one (#36).
    pub selection_mode: SelectionCombineMode,
    /// Committed selection edits, newest last, so Ctrl+Z can restore the previous mask.
    pub selection_undo: Vec<SelectionEdit>,
    pub selection_redo: Vec<SelectionEdit>,
    /// True while the newest un-done action is a selection change; Ctrl+Z then targets it.
    pub selection_pending: bool,
    /// True while the newest undone action is a selection change; Ctrl+Y then replays it.
    pub selection_redo_pending: bool,
    /// Document history depth when the newest selection edit was committed. A document edit
    /// pushes past it; undoing back to it makes the selection edit the newest action again.
    pub selection_doc_depth: usize,
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

    // ── Mask editing (#35) ──
    /// When true, freehand strokes edit the active layer's mask instead of its pixels.
    pub mask_editing: bool,
    /// Ruby overlay showing what the mask hides.
    pub mask_overlay: bool,
    /// Overlay texture, rebuilt only when the mask changes.
    pub mask_overlay_texture: Option<egui::TextureHandle>,
    pub mask_overlay_dirty: bool,

    // Overlay Toggles & Viewport
    pub show_grid: bool,
    pub grid_size: u32,
    pub grid_opacity: f32,
    /// Canvas-space origin of the grid. Dragging the on-canvas handle moves it.
    pub grid_origin: Vec2,
    /// Session-only horizontal/vertical measurement guides.
    pub guides: Vec<Guide>,
    pub guides_locked: bool,
    pub move_mode: MoveMode,
    pub snap_enabled: bool,
    pub snap_to_guides: bool,
    pub snap_to_layers: bool,
    pub snap_markers: Vec<SnapMarker>,
    pub straight_ruler: Option<StraightRuler>,
    /// Freehand strokes are projected onto the straight-edge ruler while enabled.
    pub ruler_draw: bool,
    pub show_rulers: bool,
    pub show_navigator: bool,
    /// Read-only undo/redo history section in the right dock (#38).
    pub show_history_panel: bool,
    pub flip_view_horizontal: bool,
    /// Chrome metrics written by the UI each frame, so canvas math follows the docks.
    pub left_panel_w: f32,
    pub right_panel_w: f32,
    /// Persisted side-panel width locks. Locked panels cannot be resized by dragging.
    pub left_panel_locked: bool,
    pub right_panel_locked: bool,
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
    /// Retained-memory ceiling for undo history, in MiB. Persisted with the other preferences.
    pub undo_budget_mb: u32,
    // ── Preferences dialog (#37) ──
    pub show_preferences_dialog: bool,
    pub preferences_draft: PreferencesDraft,
    pub preferences_error: Option<String>,
    /// Guide/ruler/grid changes are persisted, so they must participate in dirty tracking
    /// even though they do not create history entries.
    pub editor_dirty: bool,
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
    /// The history generation `navigator_texture` was built at. Painting, filters, undo and
    /// redo all move this, so the thumbnail stays live without a dirty call at every call site.
    pub navigator_history: u64,
    /// The paper state `navigator_texture` was built for: `(background_value, is_transparent)`.
    pub navigator_paper: (u8, bool),
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
            selection_mode: SelectionCombineMode::Replace,
            selection_undo: Vec::new(),
            selection_redo: Vec::new(),
            selection_pending: false,
            selection_redo_pending: false,
            selection_doc_depth: 0,
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
            mask_editing: false,
            mask_overlay: true,
            mask_overlay_texture: None,
            mask_overlay_dirty: true,

            show_grid: false,
            grid_size: 32,
            grid_opacity: 0.25,
            grid_origin: Vec2::ZERO,
            guides: Vec::new(),
            guides_locked: false,
            move_mode: MoveMode::SelectedLayer,
            snap_enabled: true,
            snap_to_guides: true,
            snap_to_layers: true,
            snap_markers: Vec::new(),
            straight_ruler: None,
            ruler_draw: false,
            show_rulers: true,
            show_navigator: true,
            show_history_panel: true,
            flip_view_horizontal: false,
            left_panel_w: 268.0,
            right_panel_w: 324.0,
            left_panel_locked: false,
            right_panel_locked: false,
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
            undo_budget_mb: (HistoryStack::DEFAULT_MAX_BYTES / (1024 * 1024)) as u32,
            show_preferences_dialog: false,
            preferences_draft: PreferencesDraft::default(),
            preferences_error: None,
            editor_dirty: false,
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
            navigator_history: u64::MAX,
            navigator_paper: (0, false),
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
            || self.navigator_history != self.history.generation()
            || self.navigator_paper != (self.document.background_value, self.document.is_transparent)
    }

    /// Records that the thumbnail now reflects the current document.
    pub fn note_navigator_refreshed(&mut self) {
        self.navigator_revision = self.document_revision;
        self.navigator_size = (self.document.width, self.document.height);
        self.navigator_history = self.history.generation();
        self.navigator_paper = (self.document.background_value, self.document.is_transparent);
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
        s.sync_editor_from_document();
        s
    }

    /// Copies the UI-only guide/ruler layout into the document immediately before saving.
    pub fn sync_editor_to_document(&mut self) {
        self.document.editor = EditorState {
            guides: self
                .guides
                .iter()
                .map(|guide| GuideLine {
                    vertical: guide.orientation == GuideOrientation::Vertical,
                    position: guide.position,
                })
                .collect(),
            grid_origin: [self.grid_origin.x, self.grid_origin.y],
            straight_ruler: self.straight_ruler.map(|ruler| StraightRulerState {
                origin: [ruler.origin.x, ruler.origin.y],
                angle_rad: ruler.angle_rad,
                length: ruler.length,
            }),
        };
    }

    /// Restores the persisted guide/ruler layout after a document load.
    pub fn sync_editor_from_document(&mut self) {
        let editor = self.document.editor.clone();
        self.guides = editor
            .guides
            .iter()
            .map(|guide| Guide {
                orientation: if guide.vertical {
                    GuideOrientation::Vertical
                } else {
                    GuideOrientation::Horizontal
                },
                position: guide.position,
            })
            .collect();
        self.grid_origin = Vec2::new(editor.grid_origin[0], editor.grid_origin[1]);
        self.straight_ruler = editor.straight_ruler.map(|ruler| StraightRuler {
            origin: Vec2::new(ruler.origin[0], ruler.origin[1]),
            angle_rad: ruler.angle_rad,
            length: ruler.length,
        });
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

    /// Restores the shared brush shape and dynamics to their defaults (#42).
    pub fn reset_brush_settings(&mut self) {
        self.brush.reset_brush();
        self.set_status("Brush settings reset to defaults");
    }

    /// Restores the active tool's settings to their defaults (#42).
    ///
    /// Wand and fill keep their tolerance in `AppState`, so they are reset here rather than in
    /// `BrushSettings`. Tools with nothing to reset report that instead of a false success.
    pub fn reset_current_tool_settings(&mut self) {
        let tool = self.brush.tool;
        let resettable = self.brush.reset_tool();
        match tool {
            aurelia_core::brush::ToolType::Wand | aurelia_core::brush::ToolType::Marquee
            | aurelia_core::brush::ToolType::Lasso => {
                self.wand_tolerance = 24;
                self.wand_contiguous = true;
                self.wand_sample_all_layers = false;
                self.set_status(format!("{} settings reset to defaults", tool.label()));
            }
            aurelia_core::brush::ToolType::Fill => {
                self.wand_tolerance = 24;
                self.set_status("Fill settings reset to defaults");
            }
            _ if resettable => {
                self.set_status(format!("{} settings reset to defaults", tool.label()));
            }
            _ => {
                self.set_status(format!("{} has no adjustable settings to reset", tool.label()));
            }
        }
    }

    /// True when the active layer has a mask that freehand strokes can edit.
    pub fn active_mask_available(&self) -> bool {
        self.document
            .active_layer()
            .and_then(|layer| layer.mask.as_ref())
            .is_some()
    }

    /// Toggles mask editing, refusing when there is no mask to edit.
    pub fn toggle_mask_editing(&mut self) {
        if self.mask_editing {
            self.mask_editing = false;
            self.set_status("Editing layer pixels");
        } else if self.active_mask_available() {
            self.mask_editing = true;
            self.set_status("Editing layer mask — brush strokes write mask alpha");
        } else {
            self.set_status("Add a layer mask before painting on it");
        }
        self.mask_overlay_dirty = true;
    }

    /// The grey level a mask stroke paints: white reveals, black conceals, and the eraser
    /// always conceals regardless of the paint colour.
    fn mask_paint_value(&self) -> u8 {
        if self.brush.tool == aurelia_core::brush::ToolType::Eraser {
            return 0;
        }
        let [r, g, b, _] = self.brush.primary_color.to_rgba8();
        (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as u8
    }

    fn paint_mask_with(
        &mut self,
        paint: impl FnOnce(
            &mut aurelia_core::layer::LayerMask,
            f32,
            f32,
            f32,
            u8,
            Option<&SelectionMask>,
        ),
    ) {
        let target = self.mask_paint_value();
        let radius = (self.brush.effective_size(1.0) * 0.5).max(0.5);
        let hardness = self.brush.hardness;
        let opacity = self.brush.opacity;
        let Self {
            document,
            selection,
            mask_overlay_dirty,
            ..
        } = self;
        let Some(mask) = document
            .active_layer_mut()
            .and_then(|layer| layer.mask.as_mut())
            .filter(|mask| mask.enabled)
        else {
            return;
        };
        paint(mask, radius, hardness, opacity, target, selection.as_ref());
        *mask_overlay_dirty = true;
    }

    /// Paints one mask dab at `pos` (#35).
    pub fn paint_mask_dab(&mut self, pos: Vec2) {
        self.paint_mask_with(move |mask, radius, hardness, opacity, target, selection| {
            mask.paint_dab(pos, radius, hardness, opacity, target, selection);
        });
    }

    /// Paints the mask along a stroke segment (#35), using the same dab cadence as pixels.
    pub fn paint_mask_segment(&mut self, from: Vec2, to: Vec2) {
        self.paint_mask_with(move |mask, radius, hardness, opacity, target, selection| {
            mask.paint_segment(from, to, radius, hardness, opacity, target, selection);
        });
    }

    pub fn push_color_history(&mut self, color: Color) {        if self.color_history.first() == Some(&color) {
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

    /// Adds a guide at the canvas centre. Used by the panel buttons and by a click in a ruler.
    pub fn add_guide(&mut self, orientation: GuideOrientation) {
        let position = match orientation {
            GuideOrientation::Horizontal => self.document.height as f32 * 0.5,
            GuideOrientation::Vertical => self.document.width as f32 * 0.5,
        };
        self.guides.push(Guide { orientation, position });
        self.mark_editor_dirty();
    }

    pub fn clear_guides(&mut self) {
        self.guides.clear();
        self.mark_editor_dirty();
    }

    pub fn add_straight_ruler(&mut self) {
        let length = self.document.width.min(self.document.height) as f32 * 0.65;
        let origin = Vec2::new(
            (self.document.width as f32 - length) * 0.5,
            self.document.height as f32 * 0.5,
        );
        self.straight_ruler = Some(StraightRuler {
            origin,
            angle_rad: 0.0,
            length: length.max(50.0),
        });
        self.ruler_draw = false;
        self.mark_editor_dirty();
    }

    pub fn remove_straight_ruler(&mut self) {
        self.straight_ruler = None;
        self.ruler_draw = false;
        self.mark_editor_dirty();
    }

    pub fn reset_grid_origin(&mut self) {
        self.grid_origin = Vec2::ZERO;
        self.mark_editor_dirty();
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
                        if sel.is_selected(x, y) && layer_has_alpha(layer, x, y) {
                            min_x = min_x.min(x);
                            max_x = max_x.max(x);
                            min_y = min_y.min(y);
                            max_y = max_y.max(y);
                        }
                    }
                }
            } else {
                for y in 0..doc_h {
                    for x in 0..doc_w {
                        if layer_has_alpha(layer, x, y) {
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

            // One predicate for extraction and clearing, so a selection-shaped patch and the
            // hole it leaves behind always agree.
            let sel_opt = self.selection.as_ref();
            let keep = |x: u32, y: u32| sel_opt.is_none_or(|s| s.is_selected(x, y));
            let patch = TransformPatch::extract_from_layer(
                layer, min_x, min_y, patch_w, patch_h, &keep,
            );
            let original_layer_pixels = layer.storage_bytes();

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
                original_layer_pixels,
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
                TransformPatch::clear_region(
                    layer_mut, min_x, min_y, patch_w, patch_h, &keep,
                );
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
        let keep = |x: u32, y: u32| sel_opt.is_none_or(|s| s.is_selected(x, y));

        if let Some(layer) = self.document.active_layer_mut() {
            // Restore the exact pre-session storage, then clear the source region and draw one
            // transformed patch over it. Restoring first is what makes repeated previews
            // idempotent rather than compositing each preview over the previous one.
            if orig.len() != layer.storage_len() {
                return;
            }
            layer.write_storage_bytes(0, orig);
            TransformPatch::clear_region(
                layer,
                origin.x as u32,
                origin.y as u32,
                pw,
                ph,
                &keep,
            );

            let render_mode = match mode {
                TransformMode::Affine => RenderMode::Affine(&transform),
                TransformMode::PerspectiveQuad => RenderMode::Quad(&quad),
                TransformMode::MeshGrid => RenderMode::Mesh(mesh_grid),
                TransformMode::ThinPlateSpline => RenderMode::Tps(tps),
            };
            patch.render_into(layer, pw, ph, origin, interp, render_mode);
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
            let after = layer.storage_bytes();
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
        self.transform_session.extracted_patch = TransformPatch::Srgb8(Vec::new());
    }

    /// Cancels transformation and restores original layer pixels
    pub fn cancel_transform_session(&mut self) {
        if !self.transform_session.is_active {
            return;
        }
        if let Some(layer) = self.document.active_layer_mut() {
            // Storage bytes, not an 8-bit reconstruction: this restores a 16-bit layer exactly
            // and never changes its format or allocation.
            let original = &self.transform_session.original_layer_pixels;
            if original.len() == layer.storage_len() {
                layer.write_storage_bytes(0, original);
            }
        }
        self.transform_session.is_active = false;
        self.transform_session.extracted_patch = TransformPatch::Srgb8(Vec::new());
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

    /// The mode a pointer gesture uses: an explicit Shift/Alt modifier overrides the menu
    /// mode for that gesture only, and the menu mode is the sticky default.
    pub fn effective_selection_mode(
        &self,
        shift: bool,
        alt: bool,
    ) -> SelectionCombineMode {
        match (shift, alt) {
            (true, true) => SelectionCombineMode::Intersect,
            (true, false) => SelectionCombineMode::Union,
            (false, true) => SelectionCombineMode::Subtract,
            (false, false) => self.selection_mode,
        }
    }

    /// Applies a newly drawn selection region under `mode` (#36).
    ///
    /// The resulting mask is exactly what [`SelectionMask::combined`] produces, which is also
    /// what any preview uses. An empty result clears the selection rather than leaving a
    /// zero-coverage mask behind, and a no-op change records no undo entry.
    pub fn commit_selection(&mut self, new: SelectionMask, mode: SelectionCombineMode) {
        let before = self.selection.clone();
        let after = match (&before, mode) {
            (_, SelectionCombineMode::Replace) => Some(new),
            (None, _) => Some(new),
            (Some(existing), _) => {
                if existing.width != new.width || existing.height != new.height {
                    self.set_status("Selection size does not match the canvas");
                    return;
                }
                Some(existing.combined(&new, mode))
            }
        };
        let after = after.filter(|mask| mask.has_selection());

        let unchanged = match (&before, &after) {
            (None, None) => true,
            (Some(old), Some(new)) => old.same_coverage(new),
            _ => false,
        };
        if unchanged {
            self.set_status(format!(
                "Selection unchanged ({})",
                mode.short_label()
            ));
            return;
        }

        if let Some(edit_source) = &after {
            if let Some(previous) = &before {
                // Keep the coverage change visible in the status: "3 px selected" is the only
                // confirmation that a Subtract did not remove everything.
                let before_px = previous.mask.iter().filter(|&&v| v > 0).count();
                let after_px = edit_source.mask.iter().filter(|&&v| v > 0).count();
                self.set_status(format!(
                    "Selection {} ({} → {} px selected)",
                    mode.short_label(),
                    before_px,
                    after_px
                ));
            } else {
                let count = edit_source.mask.iter().filter(|&&v| v > 0).count();
                self.set_status(format!("Selection {} ({} px selected)", mode.short_label(), count));
            }
        } else if mode == SelectionCombineMode::Subtract {
            self.set_status("Selection removed (Subtract)");
        }

        self.selection = after.clone();
        self.selection_undo.push(SelectionEdit { before, after });
        self.selection_redo.clear();
        self.selection_redo_pending = false;
        self.selection_pending = true;
        self.selection_doc_depth = self.history.len();
    }

    /// Ctrl+Z target when the newest action was a selection edit.
    fn undo_selection_edit(&mut self) -> Option<&'static str> {
        let edit = self.selection_undo.pop()?;
        self.selection = edit.before.clone();
        self.selection_redo.push(edit);
        self.selection_pending = false;
        self.selection_redo_pending = true;
        Some("Selection")
    }

    fn redo_selection_edit(&mut self) -> Option<&'static str> {
        let edit = self.selection_redo.pop()?;
        self.selection = edit.after.clone();
        self.selection_undo.push(edit);
        self.selection_redo_pending = false;
        self.selection_pending = true;
        Some("Selection")
    }

    /// Unified undo: a selection change whose document-history depth is still current is
    /// undone first, then document history takes over.
    ///
    /// Depth, not generation, is the ordering key: a document edit pushes past the recorded
    /// depth, and undoing back to it makes the selection edit the newest action again. Undo
    /// does not discard the selection stack, so the ordering survives a document undo.
    pub fn undo_action(&mut self) -> Option<&'static str> {
        if self.selection_pending
            && !self.selection_undo.is_empty()
            && self.history.len() == self.selection_doc_depth
        {
            return self.undo_selection_edit();
        }
        self.history.undo(&mut self.document)
    }

    pub fn redo_action(&mut self) -> Option<&'static str> {
        if self.selection_redo_pending
            && !self.selection_redo.is_empty()
            && self.history.len() == self.selection_doc_depth
        {
            return self.redo_selection_edit();
        }
        self.history.redo(&mut self.document)
    }

    pub fn fill_selection_active_layer(&mut self) {
        if let Some(mask) = &self.selection {
            let color = self.brush.primary_color.to_rgba8();
            if let Some(layer) = self.document.active_layer_mut() {
                let before = layer.storage_bytes();
                mask.fill_selection_on_layer(layer, color);
                let after = layer.storage_bytes();
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
            let color = [0, 0, 0, 0];
            if let Some(layer) = self.document.active_layer_mut() {
                let before = layer.storage_bytes();
                mask.fill_selection_on_layer(layer, color);
                let after = layer.storage_bytes();
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
            let color = self.brush.primary_color.to_rgba8();
            let pos = match position {
                1 => StrokePosition::Inside,
                2 => StrokePosition::Outside,
                _ => StrokePosition::Center,
            };
            if let Some(layer) = self.document.active_layer_mut() {
                let before = layer.storage_bytes();
                mask.stroke_selection_on_layer(layer, color, width, pos);
                let after = layer.storage_bytes();
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
        self.history.generation() != self.saved_generation || self.editor_dirty
    }

    /// Marks guide/ruler/grid changes as unsaved without touching pixel history.
    pub fn mark_editor_dirty(&mut self) {
        self.editor_dirty = true;
    }

    /// Records the current history generation as "written to disk".
    pub fn mark_saved(&mut self) {
        self.saved_generation = self.history.generation();
        self.editor_dirty = false;
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
        self.sync_editor_to_document();
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
        self.editor_dirty = true;
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
        self.sync_editor_to_document();
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

    /// Opens the Preferences dialog with a fresh copy of the effective settings.
    pub fn open_preferences(&mut self) {
        self.preferences_draft = PreferencesDraft::from_state(self);
        self.preferences_error = None;
        self.show_preferences_dialog = true;
    }

    /// Validates the draft, applies it and persists it.
    ///
    /// Returns an error and changes nothing when the draft is invalid. If the values applied
    /// but the file could not be written, the error says so: the settings stay live for this
    /// session and the user is not told they were saved when they were not.
    pub fn apply_preferences(&mut self) -> Result<(), String> {
        let draft = self.preferences_draft;
        draft.validate()?;
        self.apply_preferences_values();

        self.try_save_preferences()
            .map_err(|error| format!("Settings applied for this session, but saving failed: {error}"))
    }

    /// Writes the validated draft into the live state. Split from [`apply_preferences`] so the
    /// setting mapping is testable without touching the user's preferences file.
    fn apply_preferences_values(&mut self) {
        let draft = self.preferences_draft;
        self.autosave_enabled = draft.autosave_enabled;
        self.autosave_interval_secs = draft.autosave_interval_secs;
        self.undo_budget_mb = draft.undo_budget_mb;
        self.history
            .set_max_bytes(draft.undo_budget_mb as usize * 1024 * 1024);
        self.jpeg_quality = draft.jpeg_quality;
        self.document.theme = draft.theme;
    }

    pub fn preferences_path() -> PathBuf {
        Self::autosave_dir()
            .parent()
            .map(|d| d.join("preferences.txt"))
            .unwrap_or_else(|| std::env::temp_dir().join("aurelia-preferences.txt"))
    }

    pub fn load_preferences(&mut self) {
        if let Ok(text) = std::fs::read_to_string(Self::preferences_path()) {
            self.apply_preferences_text(&text);
        }
    }

    /// Applies a preferences file body.
    ///
    /// Unknown keys and unparseable values are ignored so a newer or damaged file cannot stop
    /// startup, and every numeric field is clamped to the range the dialog exposes. Missing
    /// keys keep the defaults the state was constructed with.
    pub fn apply_preferences_text(&mut self, text: &str) {
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
                "left_panel_locked" => self.left_panel_locked = value == "1",
                "right_panel_locked" => self.right_panel_locked = value == "1",
                "discord_presence" => self.discord_presence = value == "1",
                "discord_show_project" => self.discord_show_project_name = value == "1",
                "autosave_interval" => {
                    if let Ok(secs) = value.parse::<u64>() {
                        let (min, max) = AUTOSAVE_INTERVAL_RANGE;
                        self.autosave_interval_secs = secs.clamp(min, max);
                    }
                }
                "undo_budget_mb" => {
                    if let Ok(mb) = value.parse::<u32>() {
                        let (min, max) = UNDO_BUDGET_MB_RANGE;
                        self.undo_budget_mb = mb.clamp(min, max);
                        self.history
                            .set_max_bytes(self.undo_budget_mb as usize * 1024 * 1024);
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

    /// The full preferences file body. Pure, so tests can round-trip it without touching the
    /// user's real preferences.
    pub fn preferences_text(&self) -> String {
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
            "left_panel_locked={}\n",
            if self.left_panel_locked { 1 } else { 0 }
        ));
        out.push_str(&format!(
            "right_panel_locked={}\n",
            if self.right_panel_locked { 1 } else { 0 }
        ));
        out.push_str(&format!(
            "discord_presence={}\n",
            if self.discord_presence { 1 } else { 0 }
        ));
        out.push_str(&format!(
            "discord_show_project={}\n",
            if self.discord_show_project_name { 1 } else { 0 }
        ));
        out.push_str(&format!("autosave_interval={}\n", self.autosave_interval_secs));
        out.push_str(&format!("undo_budget_mb={}\n", self.undo_budget_mb));
        for recent in &self.recent_files {
            out.push_str(&format!("recent={}\n", recent.display()));
        }
        out
    }

    /// Persists preferences, surfacing the I/O error for the dialog to report.
    pub fn try_save_preferences(&self) -> std::io::Result<()> {
        let path = Self::preferences_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, self.preferences_text())
    }

    /// Fire-and-forget save for the many call sites where a failed write is not actionable.
    pub fn save_preferences(&self) {
        let _ = self.try_save_preferences();
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(1920, 1080)
    }
}

#[cfg(test)]
mod viewport_geometry {
    use super::{AppState, GuideOrientation, MoveMode, StraightRuler};

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

    /// The normalized viewport slice and the renderer's canvas-to-screen mapping must agree at
    /// all four edges. This is the invariant the Navigator framing box and hit-testing share.
    #[test]
    fn viewport_fraction_edges_match_canvas_mapping() {
        let mut state = AppState::new(640, 480);
        state.show_ui_panels = true;
        state.show_rulers = true;
        state.left_panel_w = 240.0;
        state.right_panel_w = 300.0;
        state.header_h = 40.0;
        state.footer_h = 30.0;
        state.zoom = 1.7;
        state.pan = glam::Vec2::new(13.0, -7.0);

        let (win_w, win_h) = (1200.0_f32, 800.0_f32);
        let [vx0, vy0, vx1, vy1] = state.viewport_rect(win_w as usize, win_h as usize);
        let (fx0, fy0, fx1, fy1) = state.viewport_canvas_fraction(win_w as usize, win_h as usize);
        let doc_w = state.document.width as f32;
        let doc_h = state.document.height as f32;

        let p0 = state.canvas_to_screen(glam::Vec2::new(fx0 * doc_w, fy0 * doc_h), win_w, win_h);
        let p1 = state.canvas_to_screen(glam::Vec2::new(fx1 * doc_w, fy1 * doc_h), win_w, win_h);

        assert!((p0.x - vx0 as f32).abs() < 0.01, "left edge: {} vs {vx0}", p0.x);
        assert!((p0.y - vy0 as f32).abs() < 0.01, "top edge: {} vs {vy0}", p0.y);
        assert!((p1.x - vx1 as f32).abs() < 0.01, "right edge: {} vs {vx1}", p1.x);
        assert!((p1.y - vy1 as f32).abs() < 0.01, "bottom edge: {} vs {vy1}", p1.y);
    }

    /// Overlay coordinates are in egui points; the renderer and hit-testing are in physical
    /// pixels. Converting one way and back must land on the same canvas point at every DPI.
    #[test]
    fn canvas_to_ui_and_screen_agree_at_non_integer_dpi() {
        let mut state = AppState::new(320, 200);
        state.zoom = 1.3;
        state.pan = glam::Vec2::new(-9.0, 14.0);
        state.show_ui_panels = true;
        state.left_panel_w = 91.0;
        state.right_panel_w = 117.0;
        state.header_h = 33.0;
        state.footer_h = 27.0;

        let ppp = 1.5_f32;
        state.ui_ppp = ppp;
        let (win_w, win_h) = (900.0_f32, 600.0_f32);
        let canvas = glam::Vec2::new(123.0, 77.0);
        let screen = state.canvas_to_screen(canvas, win_w, win_h);
        let ui = state.canvas_to_ui(canvas, win_w / ppp, win_h / ppp);

        assert!((ui.x * ppp - screen.x).abs() < 0.01, "x: {} vs {}", ui.x * ppp, screen.x);
        assert!((ui.y * ppp - screen.y).abs() < 0.01, "y: {} vs {}", ui.y * ppp, screen.y);
    }

    /// Guides and the grid origin are session state, not document state.
    #[test]
    fn guides_and_grid_origin_are_session_only() {
        let mut state = AppState::new(200, 100);
        state.add_guide(GuideOrientation::Horizontal);
        state.add_guide(GuideOrientation::Vertical);
        assert_eq!(state.guides.len(), 2);
        assert_eq!(state.guides[0].position, 50.0);
        assert_eq!(state.guides[1].position, 100.0);

        state.grid_origin = glam::Vec2::new(17.0, 9.0);
        state.reset_grid_origin();
        assert_eq!(state.grid_origin, glam::Vec2::ZERO);

        assert_eq!(state.move_mode, MoveMode::SelectedLayer);
        assert!(!state.guides_locked);
        assert!(state.snap_enabled && state.snap_to_guides && state.snap_to_layers);

        state.clear_guides();
        assert!(state.guides.is_empty());
    }

    #[test]
    fn editor_layout_survives_document_sync() {
        let mut state = AppState::new(64, 64);
        state.add_guide(GuideOrientation::Vertical);
        state.add_guide(GuideOrientation::Horizontal);
        state.guides[0].position = 21.0;
        state.grid_origin = glam::Vec2::new(8.0, 13.0);
        state.add_straight_ruler();

        state.sync_editor_to_document();
        let restored = AppState::from_document(state.document.clone());

        assert_eq!(restored.guides, state.guides);
        assert_eq!(restored.grid_origin, state.grid_origin);
        assert_eq!(restored.straight_ruler, state.straight_ruler);
    }

    #[test]
    fn straight_ruler_constrains_points_to_its_segment() {
        let ruler = StraightRuler {
            origin: glam::Vec2::new(10.0, 20.0),
            angle_rad: 0.0,
            length: 100.0,
        };
        assert_eq!(ruler.constrain(glam::Vec2::new(60.0, 80.0)), glam::Vec2::new(60.0, 20.0));
        assert_eq!(ruler.constrain(glam::Vec2::new(500.0, 20.0)), glam::Vec2::new(110.0, 20.0));
        assert_eq!(ruler.constrain(glam::Vec2::new(-50.0, 20.0)), glam::Vec2::new(10.0, 20.0));
    }

    /// Docks larger than the window must collapse to an empty rect, not invert it.
    #[test]
    fn tiny_window_clamps_viewport_rect() {
        let mut state = AppState::new(100, 100);
        state.show_ui_panels = true;
        state.left_panel_w = 500.0;
        state.right_panel_w = 500.0;
        state.header_h = 500.0;
        state.footer_h = 500.0;
        assert_eq!(state.viewport_rect(100, 100), [100, 100, 100, 100]);
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

    /// Painting and paper changes must invalidate the thumbnail even though neither path calls
    /// `mark_canvas_dirty` explicitly: history generation and the paper state are part of the
    /// freshness key.
    #[test]
    fn navigator_refreshes_on_paint_and_paper_changes() {
        use aurelia_core::history::LayerPixelsDeltaCommand;

        let mut state = AppState::new(32, 32);
        state.note_navigator_refreshed();
        assert!(!state.navigator_needs_refresh());

        state.document.background_value = 13;
        assert!(state.navigator_needs_refresh(), "a paper change must rebuild the thumbnail");
        state.note_navigator_refreshed();
        assert!(!state.navigator_needs_refresh());

        let id = state.document.active_layer_id;
        let bytes = state.document.active_layer().expect("layer").storage_bytes();
        state.history.push(Box::new(LayerPixelsDeltaCommand::new(id, "paint", bytes.clone(), bytes)));
        assert!(state.navigator_needs_refresh(), "a committed edit must rebuild the thumbnail");
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
    /// An 8x8 16-bit layer with an opaque, off-lattice 4x4 block inside a transparent border.
    /// The border makes the session's content bounds deterministic.
    fn sixteen_bit_transform_state() -> AppState {
        use aurelia_core::layer::LayerBuffer;
        use aurelia_core::pixel::srgb8_to_srgb16;

        let mut state = AppState::new(8, 8);
        state.document = Document::new_with_format(8, 8, PixelFormat::Srgb16);
        let id = state.document.active_layer_id;
        let layer = state.document.get_layer_mut(id).expect("layer");
        let mut channels = vec![0u16; 8 * 8 * 4];
        for (index, pixel) in channels.chunks_exact_mut(4).enumerate() {
            let x = index as u32 % 8;
            let y = index as u32 / 8;
            if (2..6).contains(&x) && (2..6).contains(&y) {
                pixel[0] = srgb8_to_srgb16(40) + 128; // off-lattice
                pixel[1] = srgb8_to_srgb16(90) + 64;
                pixel[2] = srgb8_to_srgb16(200) + 255;
                pixel[3] = 65535;
            }
        }
        *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        state
    }

    /// The session now carries the layer's own storage width: begin, preview and commit must
    /// never go through an 8-bit patch or reconstruct the layer.
    #[test]
    fn free_transform_is_native_on_a_sixteen_bit_document() {
        use aurelia_core::transform::TransformPatch;

        let mut wide = sixteen_bit_transform_state();
        wide.begin_transform_session();
        assert!(wide.transform_session.is_active, "{}", wide.status_message);
        assert!(
            matches!(wide.transform_session.extracted_patch, TransformPatch::Srgb16(_)),
            "a 16-bit layer must produce a 16-bit patch"
        );
        assert_eq!(wide.transform_session.extracted_patch.pixels(), 16);
        assert_eq!(wide.active_layer_format(), PixelFormat::Srgb16, "begin must not convert");
        assert!(!wide.status_message.contains("8-bit only"), "{}", wide.status_message);

        wide.transform_session.interpolation = InterpolationMode::Nearest;
        wide.update_transform_preview();
        assert_eq!(wide.active_layer_format(), PixelFormat::Srgb16, "preview must not convert");

        wide.commit_transform_session();
        assert_eq!(wide.active_layer_format(), PixelFormat::Srgb16, "commit must not convert");
        assert!(!wide.transform_session.is_active);
        assert!(
            wide.document
                .active_layer()
                .and_then(|layer| layer.as_srgb16())
                .is_some_and(|channels| channels.iter().any(|&c| c % 257 != 0)),
            "off-lattice detail must survive the session"
        );
    }

    /// Cancel restores the exact bytes without changing the format, the active layer, either
    /// history stack, or the dirty flag - the five observational invariants.
    #[test]
    fn cancelling_a_sixteen_bit_transform_leaves_observationally_identical_state() {
        use aurelia_core::history::LayerPixelsDeltaCommand;

        let mut wide = sixteen_bit_transform_state();
        let id = wide.document.active_layer_id;

        // Two edits, then undo one, so both stack directions are non-empty and have
        // something to lose if cancel pushes or clears anything.
        let base = wide.document.active_layer().expect("layer").storage_bytes();
        let mut first = base.clone();
        first[0] ^= 0xFF;
        let mut second = first.clone();
        second[1] ^= 0xFF;
        if let Some(layer) = wide.document.get_layer_mut(id) {
            layer.write_storage_bytes(0, &first);
        }
        wide.history.push(Box::new(LayerPixelsDeltaCommand::new(id, "seed one", base, first.clone())));
        if let Some(layer) = wide.document.get_layer_mut(id) {
            layer.write_storage_bytes(0, &second);
        }
        wide.history.push(Box::new(LayerPixelsDeltaCommand::new(id, "seed two", first.clone(), second)));
        wide.history.undo(&mut wide.document);

        let bytes = wide.document.get_layer(id).expect("layer").storage_bytes();
        let format = wide.document.pixel_format;
        let active = wide.document.active_layer_id;
        let history_len = wide.history.len();
        let can_undo = wide.history.can_undo();
        let can_redo = wide.history.can_redo();
        let generation = wide.history.generation();
        let dirty = wide.is_dirty();
        assert!(can_undo && can_redo, "the fixture must populate both stacks");

        wide.begin_transform_session();
        assert!(wide.transform_session.is_active);
        wide.transform_session.interpolation = InterpolationMode::Nearest;
        wide.transform_session.transform.translation = Vec2::new(1.5, 0.5);
        wide.update_transform_preview();
        assert_ne!(
            wide.document.get_layer(id).expect("layer").storage_bytes(),
            bytes,
            "the preview must actually alter the layer, or this test proves nothing"
        );

        wide.cancel_transform_session();

        assert_eq!(
            wide.document.get_layer(id).expect("layer").storage_bytes(),
            bytes,
            "cancel must restore the layer's storage bytes exactly"
        );
        assert_eq!(wide.document.pixel_format, format, "cancel must preserve the document format");
        assert_eq!(wide.document.active_layer_id, active, "cancel must preserve the active layer");
        assert_eq!(wide.history.len(), history_len, "cancel must not push or drop history");
        assert_eq!(wide.history.can_undo(), can_undo, "the undo stack must be untouched");
        assert_eq!(wide.history.can_redo(), can_redo, "the redo stack must be untouched");
        assert_eq!(wide.history.generation(), generation, "the history generation must not move");
        assert_eq!(wide.is_dirty(), dirty, "a cancelled transform must not imply unsaved work");
        assert!(!wide.transform_session.is_active);
    }

    /// Restoring from the session snapshot, rather than compositing over the previous preview,
    /// makes N previews identical to one. Fractional translation exercises the interpolator.
    #[test]
    fn repeated_sixteen_bit_previews_are_idempotent() {
        let mut wide = sixteen_bit_transform_state();
        wide.begin_transform_session();
        assert!(wide.transform_session.is_active);

        wide.transform_session.interpolation = InterpolationMode::Bilinear;
        wide.transform_session.transform.translation = Vec2::new(1.5, 0.5);
        wide.update_transform_preview();
        let once = wide.document.active_layer().expect("layer").storage_bytes();

        for _ in 0..4 {
            wide.update_transform_preview();
        }
        let many = wide.document.active_layer().expect("layer").storage_bytes();

        assert_eq!(once, many, "preview must be idempotent, not cumulative");
    }

    /// A 16-bit patch whose border is fully transparent must leave the opaque destination
    /// beneath that border untouched. The border deliberately carries red under zero alpha:
    /// a renderer that reads colour without checking coverage would blacken the edge.
    #[test]
    fn sixteen_bit_transparent_patch_edges_do_not_darken_the_destination() {
        use aurelia_core::layer::LayerBuffer;
        use aurelia_core::transform::{AffineTransform2D, RenderMode, TransformPatch};

        let mut doc = Document::new_with_format(6, 6, PixelFormat::Srgb16);
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            let mut blue = vec![0u16; 6 * 6 * 4];
            for pixel in blue.chunks_exact_mut(4) {
                pixel[2] = 65535;
                pixel[3] = 65535;
            }
            *layer.buffer_mut() = LayerBuffer::Srgb16(blue);
        }

        // Opaque red centre; the border is red with zero alpha, so only coverage decides.
        let mut patch = vec![0u16; 4 * 4 * 4];
        for (index, pixel) in patch.chunks_exact_mut(4).enumerate() {
            let x = index % 4;
            let y = index / 4;
            pixel[0] = 65535;
            if (1..3).contains(&x) && (1..3).contains(&y) {
                pixel[3] = 65535;
            }
        }

        let transform = AffineTransform2D::default();
        if let Some(layer) = doc.get_layer_mut(id) {
            TransformPatch::Srgb16(patch).render_into(
                layer,
                4,
                4,
                Vec2::new(1.0, 1.0),
                InterpolationMode::Nearest,
                RenderMode::Affine(&transform),
            );
        }

        let layer = doc.get_layer(id).expect("layer");
        for (index, pixel) in layer.as_srgb16().expect("16-bit").chunks_exact(4).enumerate() {
            let x = index % 6;
            let y = index / 6;
            if (1..5).contains(&x) && (1..5).contains(&y) {
                // The patch covers this pixel exactly; its opaque centre is red.
                let centre = (2..4).contains(&x) && (2..4).contains(&y);
                if centre {
                    assert_eq!(pixel[0], 65535, "opaque centre must stay red");
                    assert_eq!(pixel[2], 0, "opaque centre must not keep blue");
                } else {
                    assert_eq!(
                        (pixel[0], pixel[2]),
                        (0, 65535),
                        "the zero-alpha border must not paint pixel ({x}, {y})"
                    );
                }
            } else {
                assert_eq!((pixel[0], pixel[2]), (0, 65535), "outside the patch stays blue");
            }
        }
    }

    /// Every deformation mode and interpolation mode previews through the native patch
    /// without converting the document.
    #[test]
    fn every_transform_preview_mode_stays_native_at_sixteen_bits() {
        let mut wide = sixteen_bit_transform_state();
        wide.begin_transform_session();
        assert!(wide.transform_session.is_active);

        for mode in [
            TransformMode::Affine,
            TransformMode::PerspectiveQuad,
            TransformMode::MeshGrid,
            TransformMode::ThinPlateSpline,
        ] {
            wide.set_transform_mode(mode);
            for interpolation in [
                InterpolationMode::Nearest,
                InterpolationMode::Bilinear,
                InterpolationMode::Bicubic,
            ] {
                wide.transform_session.interpolation = interpolation;
                wide.update_transform_preview();
                assert_eq!(
                    wide.active_layer_format(),
                    PixelFormat::Srgb16,
                    "{mode:?} with {interpolation:?} must not convert the document"
                );
            }
        }
    }

    /// Selection fill/stroke remain native and must not be refused on a 16-bit document.
    #[test]
    fn selection_fill_works_on_a_sixteen_bit_document() {
        let mut wide = sixteen_bit_state();
        wide.selection = Some(aurelia_core::selection::SelectionMask::from_mask_vec(4, 4, vec![255u8; 16]));
        wide.fill_selection_active_layer();
        assert!(
            !wide.status_message.contains("8-bit only"),
            "selection fill must work on a 16-bit document: {}",
            wide.status_message
        );
        let layer = wide.document.active_layer().expect("layer");
        assert_eq!(layer.format(), PixelFormat::Srgb16, "format preserved by fill");
    }
}

#[cfg(test)]
mod preferences_dialog {
    use super::{AppState, PreferencesDraft, AUTOSAVE_INTERVAL_RANGE, UNDO_BUDGET_MB_RANGE};
    use aurelia_core::color::ThemeMode;

    #[test]
    fn preferences_round_trip_through_the_file_format() {
        let mut source = AppState::new(8, 8);
        source.autosave_enabled = false;
        source.autosave_interval_secs = 45;
        source.undo_budget_mb = 64;
        source.jpeg_quality = 77;
        source.document.theme = ThemeMode::Ember;
        source.left_panel_locked = true;
        source.recent_files = vec![std::path::PathBuf::from("C:/work/a.aur")];
        let text = source.preferences_text();

        let mut loaded = AppState::new(8, 8);
        loaded.apply_preferences_text(&text);
        assert!(!loaded.autosave_enabled);
        assert_eq!(loaded.autosave_interval_secs, 45);
        assert_eq!(loaded.undo_budget_mb, 64);
        assert_eq!(loaded.jpeg_quality, 77);
        assert_eq!(loaded.document.theme, ThemeMode::Ember);
        assert!(loaded.left_panel_locked);
        assert_eq!(loaded.recent_files.len(), 1);
        assert_eq!(
            loaded.history.max_bytes(),
            64 * 1024 * 1024,
            "loading the budget must apply it to the live history stack"
        );

        // Writing what was loaded reproduces the same file, so a restart is lossless.
        assert_eq!(loaded.preferences_text(), text);
    }

    #[test]
    fn missing_keys_keep_defaults_and_out_of_range_values_are_clamped() {
        let defaults = AppState::new(8, 8);
        let mut loaded = AppState::new(8, 8);
        loaded.apply_preferences_text(
            "autosave_interval=1\nundo_budget_mb=999999\njpeg_quality=0\ntheme=bogus\nnot_a_key=7\n",
        );
        assert_eq!(loaded.autosave_interval_secs, AUTOSAVE_INTERVAL_RANGE.0);
        assert_eq!(loaded.undo_budget_mb, UNDO_BUDGET_MB_RANGE.1);
        assert_eq!(loaded.jpeg_quality, 1);
        assert_eq!(loaded.document.theme, ThemeMode::Dawn);
        assert_eq!(
            loaded.autosave_enabled, defaults.autosave_enabled,
            "a missing key must not flip an unrelated setting"
        );
    }

    #[test]
    fn invalid_drafts_are_rejected_before_anything_changes() {
        let mut state = AppState::new(8, 8);
        let interval = state.autosave_interval_secs;
        state.preferences_draft = PreferencesDraft {
            autosave_interval_secs: 1,
            ..PreferencesDraft::default()
        };
        assert!(state.preferences_draft.validate().is_err());
        assert!(state.apply_preferences().is_err(), "invalid draft must not be applied");
        assert_eq!(state.autosave_interval_secs, interval);

        for (draft, field) in [
            (
                PreferencesDraft {
                    undo_budget_mb: 0,
                    ..PreferencesDraft::default()
                },
                "undo budget",
            ),
            (
                PreferencesDraft {
                    jpeg_quality: 0,
                    ..PreferencesDraft::default()
                },
                "jpeg quality",
            ),
        ] {
            let error = draft.validate().expect_err(field);
            assert!(error.to_lowercase().contains(field), "{error}");
        }
        assert!(PreferencesDraft::default().validate().is_ok());
    }

    #[test]
    fn applying_a_draft_updates_bound_controls_and_leaves_others_alone() {
        let mut state = AppState::new(8, 8);
        state.left_panel_locked = true;
        state.preferences_draft = PreferencesDraft {
            autosave_enabled: false,
            autosave_interval_secs: 30,
            undo_budget_mb: 32,
            jpeg_quality: 55,
            theme: ThemeMode::Dusk,
        };
        state.apply_preferences_values();

        assert!(!state.autosave_enabled);
        assert_eq!(state.autosave_interval_secs, 30);
        assert_eq!(state.undo_budget_mb, 32);
        assert_eq!(state.history.max_bytes(), 32 * 1024 * 1024);
        assert_eq!(state.jpeg_quality, 55);
        assert_eq!(state.document.theme, ThemeMode::Dusk);
        assert!(state.left_panel_locked, "unrelated persisted settings must survive");
    }

    #[test]
    fn opening_the_dialog_copies_the_effective_settings() {
        let mut state = AppState::new(8, 8);
        state.autosave_interval_secs = 90;
        state.undo_budget_mb = 128;
        state.jpeg_quality = 61;
        state.document.theme = ThemeMode::Ember;
        state.open_preferences();
        assert!(state.show_preferences_dialog);
        assert!(state.preferences_error.is_none());
        assert_eq!(
            state.preferences_draft,
            PreferencesDraft {
                autosave_enabled: state.autosave_enabled,
                autosave_interval_secs: 90,
                undo_budget_mb: 128,
                jpeg_quality: 61,
                theme: ThemeMode::Ember,
            }
        );
    }
}

#[cfg(test)]
mod selection_modes {
    use super::{AppState, SelectionCombineMode, SelectionMask};

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> SelectionMask {
        SelectionMask::from_rect(16, 16, glam::Vec2::new(x0, y0), glam::Vec2::new(x1, y1))
    }

    fn state() -> AppState {
        AppState::new(16, 16)
    }

    #[test]
    fn modifiers_override_the_sticky_mode_for_one_gesture() {
        let mut s = state();
        s.selection_mode = SelectionCombineMode::Replace;
        assert_eq!(s.effective_selection_mode(false, false), SelectionCombineMode::Replace);
        assert_eq!(s.effective_selection_mode(true, false), SelectionCombineMode::Union);
        assert_eq!(s.effective_selection_mode(false, true), SelectionCombineMode::Subtract);
        assert_eq!(s.effective_selection_mode(true, true), SelectionCombineMode::Intersect);
        s.selection_mode = SelectionCombineMode::Intersect;
        assert_eq!(s.effective_selection_mode(false, false), SelectionCombineMode::Intersect);
    }

    #[test]
    fn committed_modes_change_the_selection_the_way_the_core_does() {
        let mut s = state();
        s.commit_selection(rect(0.0, 0.0, 8.0, 8.0), SelectionCombineMode::Replace);
        assert!(s.selection.is_some());

        s.commit_selection(rect(8.0, 0.0, 16.0, 8.0), SelectionCombineMode::Union);
        let union = s.selection.clone().expect("union");
        assert_eq!(union.mask.iter().filter(|&&v| v > 0).count(), 16 * 8, "two halves make a full band");

        s.commit_selection(rect(8.0, 0.0, 16.0, 8.0), SelectionCombineMode::Subtract);
        let subtracted = s.selection.clone().expect("subtracted");
        assert_eq!(subtracted.mask.iter().filter(|&&v| v > 0).count(), 8 * 8);

        s.commit_selection(rect(0.0, 0.0, 4.0, 4.0), SelectionCombineMode::Intersect);
        let intersected = s.selection.clone().expect("intersected");
        assert_eq!(intersected.mask.iter().filter(|&&v| v > 0).count(), 4 * 4);
    }

    #[test]
    fn subtract_everything_clears_the_selection_and_is_undoable() {
        let mut s = state();
        s.commit_selection(rect(0.0, 0.0, 8.0, 8.0), SelectionCombineMode::Replace);
        let before = s.selection.clone().expect("initial selection");
        s.commit_selection(rect(0.0, 0.0, 16.0, 16.0), SelectionCombineMode::Subtract);
        assert!(s.selection.is_none(), "an empty result must clear the selection");

        assert_eq!(s.undo_action(), Some("Selection"));
        let restored = s.selection.clone().expect("undo restores the mask");
        assert!(restored.same_coverage(&before));

        assert_eq!(s.redo_action(), Some("Selection"));
        assert!(s.selection.is_none(), "redo re-applies the subtraction");
    }

    #[test]
    fn a_no_op_commit_records_no_history_entry() {
        let mut s = state();
        s.commit_selection(rect(0.0, 0.0, 8.0, 8.0), SelectionCombineMode::Replace);
        let depth = s.selection_undo.len();
        s.commit_selection(rect(0.0, 0.0, 8.0, 8.0), SelectionCombineMode::Union);
        assert_eq!(s.selection_undo.len(), depth, "unchanged coverage must not add an undo step");
    }

    #[test]
    fn document_undo_takes_over_when_a_document_edit_is_newer() {
        use aurelia_core::history::Command;

        #[derive(Debug)]
        struct Noop;
        impl Command for Noop {
            fn name(&self) -> &'static str {
                "Noop"
            }
            fn undo(&mut self, _doc: &mut aurelia_core::document::Document) {}
            fn redo(&mut self, _doc: &mut aurelia_core::document::Document) {}
        }

        let mut s = state();
        s.commit_selection(rect(0.0, 0.0, 4.0, 4.0), SelectionCombineMode::Replace);
        s.history.push(Box::new(Noop));

        // The document edit is newer, so the first undo must go to the document, not the mask.
        assert_eq!(s.undo_action(), Some("Noop"));
        assert!(s.selection.is_some(), "the selection edit is not undone by a document undo");
    }
}


#[cfg(test)]
mod mask_editing {
    use super::AppState;
    use aurelia_core::layer::PixelFormat;

    fn state_with_mask(format: PixelFormat) -> AppState {
        let mut state = AppState::new(8, 8);
        if format == PixelFormat::Srgb16 {
            let id = state.document.active_layer_id;
            if let Some(layer) = state.document.get_layer_mut(id) {
                layer.convert_format(PixelFormat::Srgb16);
            }
        }
        let id = state.document.active_layer_id;
        state.document.add_layer_mask(id, None);
        state
    }

    #[test]
    fn stroke_paints_mask_alpha_without_touching_pixels() {
        for format in [PixelFormat::Srgb8, PixelFormat::Srgb16] {
            let mut state = state_with_mask(format);
            let id = state.document.active_layer_id;
            let pixels_before = state.document.get_layer(id).expect("layer").storage_bytes();
            state.mask_editing = true;

            state.paint_mask_dab(glam::Vec2::new(4.0, 4.0));
            state.paint_mask_segment(glam::Vec2::new(2.0, 6.0), glam::Vec2::new(6.0, 6.0));

            let layer = state.document.get_layer(id).expect("layer");
            assert_eq!(layer.format(), format, "mask painting must preserve storage format");
            assert_eq!(layer.storage_bytes(), pixels_before, "pixels must not be touched");
            let mask = layer.mask.as_ref().expect("mask");
            assert_ne!(mask.pixels, vec![255u8; 64], "the stroke must change the mask");
        }
    }

    #[test]
    fn toggle_refuses_without_a_mask_and_accepts_with_one() {
        let mut state = AppState::new(8, 8);
        state.toggle_mask_editing();
        assert!(!state.mask_editing, "no mask, nothing to edit");
        assert!(state.status_message.contains("Add a layer mask"));

        let id = state.document.active_layer_id;
        state.document.add_layer_mask(id, None);
        state.toggle_mask_editing();
        assert!(state.mask_editing);
        state.toggle_mask_editing();
        assert!(!state.mask_editing);
    }

    #[test]
    fn eraser_conceals_even_with_a_white_primary_colour() {
        let mut state = state_with_mask(PixelFormat::Srgb8);
        state.brush.primary_color = aurelia_core::color::Color::WHITE;
        state.brush.tool = aurelia_core::brush::ToolType::Eraser;
        state.mask_editing = true;
        state.paint_mask_dab(glam::Vec2::new(4.0, 4.0));
        let mask = state
            .document
            .active_layer()
            .and_then(|layer| layer.mask.as_ref())
            .expect("mask");
        assert_eq!(mask.alpha_at(4, 4), 0, "the eraser always conceals");
    }
}

