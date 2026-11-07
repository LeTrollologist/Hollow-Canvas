use crate::color::Color;
use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ToolType {
    #[default]
    Brush,
    Pencil,
    Watercolor,
    Chalk,
    Spray,
    Smudge,
    Line,
    Fill,
    Gradient,
    Eraser,
    Rect,
    Ellipse,
    Polygon,
    Text,
    Marquee,
    Lasso,
    Wand,
    Move,
    Transform,
    Crop,
    Eyedropper,
    SelectionBrush,
    SelectionEraser,
}

impl ToolType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Brush => "Brush",
            Self::Pencil => "Pencil",
            Self::Watercolor => "Water",
            Self::Chalk => "Chalk",
            Self::Spray => "Spray",
            Self::Smudge => "Smudge",
            Self::Line => "Line",
            Self::Fill => "Fill",
            Self::Gradient => "Gradient",
            Self::Eraser => "Eraser",
            Self::Rect => "Rect",
            Self::Ellipse => "Oval",
            Self::Polygon => "Polygon",
            Self::Text => "Text",
            Self::Marquee => "Marquee",
            Self::Lasso => "Lasso",
            Self::Wand => "Wand",
            Self::Move => "Move",
            Self::Transform => "Transform",
            Self::Crop => "Crop",
            Self::Eyedropper => "Eyedropper",
            Self::SelectionBrush => "Sel Brush",
            Self::SelectionEraser => "Sel Erase",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Brush => "✦",
            Self::Pencil => "✎",
            Self::Watercolor => "≋",
            Self::Chalk => "░",
            Self::Spray => "⁕",
            Self::Smudge => "≈",
            Self::Line => "╱",
            Self::Fill => "⯀",
            Self::Gradient => "▨",
            Self::Eraser => "⌫",
            Self::Rect => "▭",
            Self::Ellipse => "○",
            Self::Polygon => "⬟",
            Self::Text => "T",
            Self::Marquee => "▢",
            Self::Lasso => "⟳",
            Self::Wand => "★",
            Self::Move => "✛",
            Self::Transform => "⤢",
            Self::Crop => "⛶",
            Self::Eyedropper => "◉",
            Self::SelectionBrush => "🖌",
            Self::SelectionEraser => "🧹",
        }
    }

    pub fn is_freehand_stroke_tool(&self) -> bool {
        matches!(
            self,
            Self::Brush
                | Self::Pencil
                | Self::Watercolor
                | Self::Chalk
                | Self::Spray
                | Self::Smudge
                | Self::Eraser
                | Self::SelectionBrush
                | Self::SelectionEraser
        )
    }

    pub fn is_selection_stroke_tool(&self) -> bool {
        matches!(self, Self::SelectionBrush | Self::SelectionEraser)
    }

    pub fn is_shape_tool(&self) -> bool {
        matches!(
            self,
            Self::Line | Self::Rect | Self::Ellipse
        )
    }

    pub fn is_selection_tool(&self) -> bool {
        matches!(
            self,
            Self::Marquee | Self::Lasso | Self::Wand | Self::SelectionBrush | Self::SelectionEraser
        )
    }

    pub fn is_painting_tool(&self) -> bool {
        (self.is_freehand_stroke_tool() && !self.is_selection_stroke_tool())
            || self.is_shape_tool()
            || *self == Self::Polygon
            || *self == Self::Fill
            || *self == Self::Gradient
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShapeFillMode {
    #[default]
    Stroke,
    Fill,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EraserMode {
    #[default]
    Soft,
    HardPixel,
    ColorErase,
}

impl EraserMode {
    pub const ALL: &'static [Self] = &[Self::Soft, Self::HardPixel, Self::ColorErase];
    pub fn label(&self) -> &'static str {
        match self {
            Self::Soft => "Soft Alpha",
            Self::HardPixel => "Hard 1-Bit Pixel",
            Self::ColorErase => "Color Target",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GradientType {
    #[default]
    Linear,
    Radial,
}

impl GradientType {
    pub const ALL: &'static [Self] = &[Self::Linear, Self::Radial];
    pub fn label(&self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Radial => "Radial",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TextAlign {
    pub const ALL: &'static [Self] = &[Self::Left, Self::Center, Self::Right];
    pub fn label(&self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Center => "Center",
            Self::Right => "Right",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Left => "⇤",
            Self::Center => "⇥⇤",
            Self::Right => "⇥",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSettings {
    pub content: String,
    pub font_name: String,
    pub font_path: Option<String>,
    #[serde(skip)]
    pub font_bytes: Option<Vec<u8>>,
    pub font_size: f32,
    pub line_spacing: f32,
    pub letter_spacing: f32,
    pub align: TextAlign,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            content: "Aurelia".to_string(),
            font_name: "Default Sans (Segoe UI / Arial)".to_string(),
            font_path: None,
            font_bytes: None,
            font_size: 36.0,
            line_spacing: 1.2,
            letter_spacing: 0.0,
            align: TextAlign::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushPoint {
    pub position: Vec2,
    pub pressure: f32, // 0.0 ..= 1.0
}

impl BrushPoint {
    pub fn new(position: Vec2, pressure: f32) -> Self {
        Self {
            position,
            pressure: pressure.clamp(0.01, 1.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrushSettings {
    pub tool: ToolType,
    pub size: f32,
    pub opacity: f32,
    pub smoothing: f32,
    pub hardness: f32,
    pub spacing: f32,
    pub primary_color: Color,
    pub secondary_color: Color,
    pub eraser_mode: EraserMode,
    pub color_erase_tolerance: u8,
    pub eraser_to_background: bool,
    pub shape_fill_mode: ShapeFillMode,
    pub gradient_type: GradientType,
    pub gradient_dither: bool,
    pub corner_radius: f32,
    pub spray_density: f32,
    pub watercolor_wetness: f32,
    pub chalk_grain: f32,
    pub smudge_strength: f32,
    // ── Velocity Dynamics & Calligraphy ──
    pub velocity_dynamics: bool,
    pub velocity_taper_strength: f32, // 0.0 ..= 1.0 (how sharply fast strokes thin out)
    pub velocity_min_size: f32,        // 0.05 ..= 1.0 (minimum tip ratio under max speed)
    pub calligraphy_angle: f32,        // 0.0 ..= 180.0 degrees (chisel nib orientation)
    /// How strongly stylus tilt widens the stroke (0 disables tilt response).
    /// Real pens lay flatter as they tilt, spreading the tip over more pixels.
    #[serde(default)]
    pub tilt_size_gain: f32,
    pub calligraphy_weight: f32,       // 0.0 ..= 1.0 (0.0 = round, 1.0 = sharp flat chisel)
    // ── Wet Edge Watercolor Effect ──
    pub wet_edge_strength: f32,        // 0.0 ..= 1.0 (dark pigment pooling at stroke boundaries)
    pub wet_edge_fringe_width: f32,    // 0.05 ..= 0.5 (fringe band thickness ratio)
    // ── Global Stroke Stabilization (S-Levels: 0..=7) ──
    pub stabilization_level: u32,       // 0 = Off (Realtime Raw), 1..=7 (SAI style lazy rope / tremor filter)
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            tool: ToolType::Brush,
            size: 8.0,
            opacity: 1.0,
            smoothing: 0.5,
            hardness: 0.7,
            spacing: 0.25,
            primary_color: Color::BRAND_GOLD,
            secondary_color: Color::from_hex("#130f30").unwrap_or(Color::BLACK),
            eraser_mode: EraserMode::Soft,
            color_erase_tolerance: 32,
            eraser_to_background: false,
            shape_fill_mode: ShapeFillMode::Stroke,
            gradient_type: GradientType::Linear,
            gradient_dither: true,
            corner_radius: 0.0,
            spray_density: 0.5,
            watercolor_wetness: 0.65,
            chalk_grain: 0.75,
            smudge_strength: 0.6,
            velocity_dynamics: true,
            velocity_taper_strength: 0.6,
            velocity_min_size: 0.15,
            calligraphy_angle: 45.0,
            calligraphy_weight: 0.0,
            tilt_size_gain: 0.0,
            wet_edge_strength: 0.0,
            wet_edge_fringe_width: 0.2,
            stabilization_level: 0,
        }
    }
}

impl BrushSettings {
    /// Restores the shared brush shape/dynamics (#42), leaving the active tool, the tool's
    /// own parameters and both colours alone.
    ///
    /// "Shared" means every painting tool reads these: tip size, opacity, hardness, spacing,
    /// smoothing, velocity dynamics, calligraphy and wet-edge shaping, and stabilisation.
    /// Colours are excluded because they are a global paint choice, not a brush setting:
    /// resetting a brush must not change what the user is painting with.
    pub fn reset_brush(&mut self) {
        let defaults = Self::default();
        self.size = defaults.size;
        self.opacity = defaults.opacity;
        self.smoothing = defaults.smoothing;
        self.hardness = defaults.hardness;
        self.spacing = defaults.spacing;
        self.velocity_dynamics = defaults.velocity_dynamics;
        self.velocity_taper_strength = defaults.velocity_taper_strength;
        self.velocity_min_size = defaults.velocity_min_size;
        self.calligraphy_angle = defaults.calligraphy_angle;
        self.calligraphy_weight = defaults.calligraphy_weight;
        self.tilt_size_gain = defaults.tilt_size_gain;
        self.wet_edge_strength = defaults.wet_edge_strength;
        self.wet_edge_fringe_width = defaults.wet_edge_fringe_width;
        self.stabilization_level = defaults.stabilization_level;
    }

    /// Restores the active tool's settings to the values it is created with (#42).
    ///
    /// Only the parameters the tool actually exposes are touched, so resetting the eraser
    /// cannot silently change the gradient or the shape outline another tool uses. Returns
    /// `false` for tools with no adjustable parameters, so the UI can say so instead of
    /// pretending something happened.
    pub fn reset_tool(&mut self) -> bool {
        let defaults = Self::default();
        match self.tool {
            ToolType::Brush | ToolType::Pencil => {
                self.reset_brush();
            }
            ToolType::Eraser => {
                self.reset_brush();
                self.eraser_mode = defaults.eraser_mode;
                self.color_erase_tolerance = defaults.color_erase_tolerance;
                self.eraser_to_background = defaults.eraser_to_background;
            }
            ToolType::Watercolor => {
                self.reset_brush();
                self.watercolor_wetness = defaults.watercolor_wetness;
                self.wet_edge_strength = defaults.wet_edge_strength;
                self.wet_edge_fringe_width = defaults.wet_edge_fringe_width;
            }
            ToolType::Chalk => {
                self.reset_brush();
                self.chalk_grain = defaults.chalk_grain;
            }
            ToolType::Spray => {
                self.reset_brush();
                self.spray_density = defaults.spray_density;
            }
            ToolType::Smudge => {
                self.reset_brush();
                self.smudge_strength = defaults.smudge_strength;
            }
            ToolType::Line | ToolType::Rect | ToolType::Ellipse | ToolType::Polygon => {
                self.reset_brush();
                self.shape_fill_mode = defaults.shape_fill_mode;
                self.corner_radius = defaults.corner_radius;
            }
            ToolType::Gradient => {
                self.gradient_type = defaults.gradient_type;
                self.gradient_dither = defaults.gradient_dither;
            }
            // Wand/fill/selection/move tools keep their settings elsewhere; text keeps its
            // TextSettings. Neither has a brush parameter to reset.
            _ => return false,
        }
        true
    }

    pub fn effective_size(&self, pressure: f32) -> f32 {
        match self.tool {
            ToolType::Pencil => (self.size * 0.5 * (0.3 + pressure * 0.9)).max(1.0),
            ToolType::Eraser => {
                if self.eraser_mode == EraserMode::HardPixel {
                    self.size.max(1.0)
                } else {
                    self.size * (0.75 + pressure * 0.6)
                }
            }
            ToolType::Spray => self.size * 1.5,
            ToolType::Watercolor => self.size * (0.7 + pressure * 0.6),
            _ => (self.size * (0.4 + pressure * 0.9)).max(1.0),
        }
    }

    /// Calculate chisel ribbon width factor (0.1 ..= 1.0) based on stroke tangent direction vs calligraphy angle
    pub fn calligraphy_factor(&self, tangent: Option<Vec2>) -> f32 {
        if self.calligraphy_weight <= 0.001 {
            return 1.0;
        }
        let dir = match tangent {
            Some(d) if d.length_squared() > 0.0001 => d.normalize(),
            _ => return 1.0,
        };

        let chisel_rad = self.calligraphy_angle.to_radians();
        let chisel_vec = Vec2::new(chisel_rad.cos(), chisel_rad.sin());

        // When moving parallel to chisel vector, stroke is thin; perpendicular is thick
        let dot = (dir.x * chisel_vec.x + dir.y * chisel_vec.y).abs();
        let min_factor = 1.0 - self.calligraphy_weight * 0.85;
        (min_factor + (1.0 - dot) * (1.0 - min_factor)).clamp(0.1, 1.0)
    }

    pub fn stabilization_label(&self) -> &'static str {
        match self.stabilization_level {
            0 => "S-0 (Off / Raw)",
            1 => "S-1 (Responsive)",
            2 => "S-2 (Studio Default)",
            3 => "S-3 (Smooth Inking)",
            4 => "S-4 (Fluid Curves)",
            5 => "S-5 (Heavy Streamline)",
            6 => "S-6 (Ultra Precision)",
            7 => "S-7 (Max Lazy Rope)",
            _ => "S-Custom",
        }
    }

    pub fn stabilization_description(&self, level: u32) -> &'static str {
        match level {
            0 => "S-0: Direct raw input without smoothing delay",
            1 => "S-1: Light filtering for fast responsive sketching",
            2 => "S-2: Balanced studio stabilization for general illustration",
            3 => "S-3: Smooth inking stabilizer for clean lineart",
            4 => "S-4: Enhanced curve stabilization for fluid contouring",
            5 => "S-5: Heavy stabilizer filtering out minor hand tremors",
            6 => "S-6: High-precision lazy rope for long confident strokes",
            7 => "S-7: Maximum lazy rope delay for ultra-steady lineart and lettering",
            _ => "Custom stabilization level",
        }
    }

    /// Weight factor (0.0 ..= 0.90) controlling the lazy rope pull lag
    pub fn stabilization_weight(&self) -> f32 {
        match self.stabilization_level {
            0 => 0.0,
            1 => 0.15,
            2 => 0.28,
            3 => 0.42,
            4 => 0.55,
            5 => 0.68,
            6 => 0.78,
            7 => 0.88,
            _ => (self.stabilization_level as f32 * 0.11).clamp(0.0, 0.92),
        }
    }

    /// Minimum cursor distance threshold (deadzone) in canvas pixels to eliminate tremor micro-jitter
    pub fn stabilization_deadzone(&self) -> f32 {
        match self.stabilization_level {
            0 => 0.0,
            1 => 0.0,
            2 => 0.0,
            3 => 0.2,
            4 => 0.4,
            5 => 0.6,
            6 => 0.8,
            7 => 1.2,
            _ => (self.stabilization_level as f32 * 0.2).clamp(0.0, 3.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrushPreset {
    pub name: String,
    pub icon: String,
    pub category: String,
    pub description: String,
    pub settings: BrushSettings,
}

impl BrushPreset {
    pub fn default_library() -> Vec<Self> {
        vec![
            Self {
                name: "G-Pen Inker".to_string(),
                icon: "✒".to_string(),
                category: "Inking".to_string(),
                description: "Crisp manga & comic lineart pen with sharp velocity tapering".to_string(),
                settings: BrushSettings {
                    tool: ToolType::Brush,
                    size: 6.0,
                    opacity: 1.0,
                    hardness: 0.95,
                    smoothing: 0.65,
                    spacing: 0.15,
                    velocity_dynamics: true,
                    velocity_taper_strength: 0.85,
                    velocity_min_size: 0.1,


                    wet_edge_strength: 0.0,
                    ..Default::default()
                },
            },
            Self {
                name: "Studio Pencil (2B)".to_string(),
                icon: "✎".to_string(),
                category: "Sketching".to_string(),
                description: "Graphite texture with organic pressure response".to_string(),
                settings: BrushSettings {
                    tool: ToolType::Pencil,
                    size: 4.0,
                    opacity: 0.85,
                    hardness: 0.85,
                    smoothing: 0.35,
                    chalk_grain: 0.4,
                    velocity_dynamics: true,
                    velocity_taper_strength: 0.7,
                    velocity_min_size: 0.2,
                    ..Default::default()
                },
            },
            Self {
                name: "Calligraphy Nib".to_string(),
                icon: "🖋".to_string(),
                category: "Calligraphy".to_string(),
                description: "45° Chisel ribbon nib with velocity-sensitive stroke width".to_string(),
                settings: BrushSettings {
                    tool: ToolType::Brush,
                    size: 14.0,
                    opacity: 0.95,
                    hardness: 0.9,
                    smoothing: 0.55,
                    spacing: 0.12,
                    calligraphy_angle: 45.0,
                    calligraphy_weight: 0.8,
                    velocity_dynamics: true,
                    velocity_taper_strength: 0.5,
                    velocity_min_size: 0.2,
                    ..Default::default()
                },
            },
            Self {
                name: "Soft Airbrush".to_string(),
                icon: "☁".to_string(),
                category: "Painting".to_string(),
                description: "Ultra-smooth diffuse gradient shading brush".to_string(),
                settings: BrushSettings {
                    tool: ToolType::Brush,
                    size: 36.0,
                    opacity: 0.35,
                    hardness: 0.05,
                    spacing: 0.12,
                    smoothing: 0.4,
                    velocity_dynamics: false,
                    wet_edge_strength: 0.0,
                    ..Default::default()
                },
            },
            Self {
                name: "Wet Watercolor".to_string(),
                icon: "≋".to_string(),
                category: "Painting".to_string(),
                description: "Fluid watercolor with dark wet-edge pigment pooling".to_string(),
                settings: BrushSettings {
                    tool: ToolType::Watercolor,
                    size: 22.0,
                    opacity: 0.55,
                    hardness: 0.5,
                    watercolor_wetness: 0.8,
                    wet_edge_strength: 0.75,
                    wet_edge_fringe_width: 0.25,
                    velocity_dynamics: true,
                    velocity_taper_strength: 0.4,
                    velocity_min_size: 0.3,
                    ..Default::default()
                },
            },
            Self {
                name: "Concept Oil".to_string(),
                icon: "🎨".to_string(),
                category: "Painting".to_string(),
                description: "Rich impasto oil blending with subtle edge definition".to_string(),
                settings: BrushSettings {
                    tool: ToolType::Brush,
                    size: 18.0,
                    opacity: 0.9,
                    hardness: 0.75,
                    smoothing: 0.5,
                    smudge_strength: 0.5,
                    wet_edge_strength: 0.4,
                    wet_edge_fringe_width: 0.15,
                    velocity_dynamics: true,
                    velocity_taper_strength: 0.35,
                    velocity_min_size: 0.25,
                    ..Default::default()
                },
            },
            Self {
                name: "Rough Charcoal".to_string(),
                icon: "░".to_string(),
                category: "Sketching".to_string(),
                description: "Heavy grainy charcoal texture with expressive velocity response".to_string(),
                settings: BrushSettings {
                    tool: ToolType::Chalk,
                    size: 16.0,
                    opacity: 0.85,
                    chalk_grain: 0.85,
                    hardness: 0.6,
                    smoothing: 0.3,
                    velocity_dynamics: true,
                    velocity_taper_strength: 0.5,
                    velocity_min_size: 0.2,
                    ..Default::default()
                },
            },
            Self {
                name: "Copic Marker".to_string(),
                icon: "🖊".to_string(),
                category: "Design".to_string(),
                description: "Semi-transparent layering marker with slight wet edge fringe".to_string(),
                settings: BrushSettings {
                    tool: ToolType::Brush,
                    size: 18.0,
                    opacity: 0.4,
                    hardness: 0.8,
                    spacing: 0.18,
                    calligraphy_angle: 60.0,
                    calligraphy_weight: 0.45,
                    wet_edge_strength: 0.35,
                    wet_edge_fringe_width: 0.2,
                    velocity_dynamics: true,
                    velocity_taper_strength: 0.3,
                    velocity_min_size: 0.35,
                    ..Default::default()
                },
            },
        ]
    }
}

/// Size multiplier for a stylus tilted by `(tilt_x, tilt_y)` degrees.
///
/// A pen laid flatter spreads its tip over more pixels, so the stroke widens: at a
/// full 90 degree tilt the width is `1 + gain` times the base size. `gain` of 0
/// disables the response entirely, which is the default.
pub fn tilt_size_scale(tilt_x_deg: f32, tilt_y_deg: f32, gain: f32) -> f32 {
    let gain = gain.clamp(0.0, 1.0);
    if gain <= 0.0 || !tilt_x_deg.is_finite() || !tilt_y_deg.is_finite() {
        return 1.0;
    }
    let magnitude = (tilt_x_deg * tilt_x_deg + tilt_y_deg * tilt_y_deg).sqrt();
    let normalised = (magnitude / 90.0).clamp(0.0, 1.0);
    1.0 + gain * normalised
}

#[cfg(test)]
mod reset_tests {
    use super::{BrushSettings, EraserMode, GradientType, ToolType};

    /// The brush reset exists so a user can get a known tip back. It must restore every shared
    /// shape/dynamics parameter and nothing else: colours stay, the tool stays, and the
    /// tool's own parameters stay.
    #[test]
    fn reset_brush_restores_only_shared_shape_and_dynamics() {
        let mut brush = BrushSettings {
            tool: ToolType::Gradient,
            size: 240.0,
            opacity: 0.12,
            hardness: 0.02,
            spacing: 3.0,
            smoothing: 0.0,
            velocity_dynamics: false,
            velocity_taper_strength: 0.0,
            velocity_min_size: 1.0,
            calligraphy_angle: 170.0,
            calligraphy_weight: 1.0,
            tilt_size_gain: 1.0,
            wet_edge_strength: 1.0,
            stabilization_level: 7,
            gradient_type: GradientType::Radial,
            gradient_dither: false,
            ..BrushSettings::default()
        };
        let primary = brush.primary_color;
        let secondary = brush.secondary_color;

        brush.reset_brush();

        let default = BrushSettings::default();
        assert_eq!(brush.size, default.size);
        assert_eq!(brush.opacity, default.opacity);
        assert_eq!(brush.hardness, default.hardness);
        assert_eq!(brush.spacing, default.spacing);
        assert_eq!(brush.smoothing, default.smoothing);
        assert_eq!(brush.velocity_dynamics, default.velocity_dynamics);
        assert_eq!(brush.calligraphy_angle, default.calligraphy_angle);
        assert_eq!(brush.stabilization_level, default.stabilization_level);
        assert_eq!(brush.wet_edge_strength, default.wet_edge_strength);

        assert_eq!(brush.tool, ToolType::Gradient, "the active tool must not change");
        assert_eq!(brush.primary_color, primary, "reset must not change the paint colour");
        assert_eq!(brush.secondary_color, secondary);
        assert_eq!(brush.gradient_type, GradientType::Radial, "tool settings are not brush settings");
        assert!(!brush.gradient_dither);
    }

    /// Resetting a tool must be scoped: the eraser's reset cannot reach the gradient or the
    /// watercolour controls, and a tool with no parameters reports that honestly.
    #[test]
    fn reset_tool_is_scoped_to_the_active_tool() {
        let mut brush = BrushSettings {
            tool: ToolType::Eraser,
            eraser_mode: EraserMode::ColorErase,
            color_erase_tolerance: 120,
            eraser_to_background: true,
            gradient_type: GradientType::Radial,
            spray_density: 1.9,
            size: 300.0,
            ..BrushSettings::default()
        };
        assert!(brush.reset_tool());
        assert_eq!(brush.eraser_mode, EraserMode::Soft);
        assert_eq!(brush.color_erase_tolerance, BrushSettings::default().color_erase_tolerance);
        assert!(!brush.eraser_to_background);
        assert_eq!(brush.size, BrushSettings::default().size);
        assert_eq!(brush.gradient_type, GradientType::Radial, "unrelated tool untouched");
        assert_eq!(brush.spray_density, 1.9, "unrelated tool untouched");

        brush.tool = ToolType::Gradient;
        assert!(brush.reset_tool());
        assert_eq!(brush.gradient_type, GradientType::Linear);
        assert!(brush.gradient_dither);
        assert_eq!(brush.eraser_mode, EraserMode::Soft, "eraser stays as it was");

        brush.tool = ToolType::Move;
        assert!(!brush.reset_tool(), "a tool with no settings must say so");
        brush.tool = ToolType::Text;
        assert!(!brush.reset_tool());
    }
}
