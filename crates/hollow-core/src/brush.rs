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
    Clone,
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
    Crop,
    Eyedropper,
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
            Self::Clone => "Clone",
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
            Self::Crop => "Crop",
            Self::Eyedropper => "Eyedropper",
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
                | Self::Clone
                | Self::Eraser
        )
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
            Self::Marquee | Self::Lasso | Self::Wand
        )
    }

    pub fn is_painting_tool(&self) -> bool {
        self.is_freehand_stroke_tool() || self.is_shape_tool() || *self == Self::Polygon || *self == Self::Fill
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShapeFillMode {
    #[default]
    Stroke,
    Fill,
    Both,
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
    pub eraser_to_background: bool,
    pub clone_aligned: bool,
    pub clone_source: Option<Vec2>,
    pub shape_fill_mode: ShapeFillMode,
    pub corner_radius: f32,
    pub spray_density: f32,
    pub watercolor_wetness: f32,
    pub chalk_grain: f32,
    pub smudge_strength: f32,
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
            primary_color: Color::HOLLOW_PURPLE,
            secondary_color: Color::from_hex("#130f30").unwrap_or(Color::BLACK),
            eraser_to_background: false,
            clone_aligned: true,
            clone_source: None,
            shape_fill_mode: ShapeFillMode::Stroke,
            corner_radius: 0.0,
            spray_density: 0.5,
            watercolor_wetness: 0.65,
            chalk_grain: 0.75,
            smudge_strength: 0.6,
        }
    }
}

impl BrushSettings {
    pub fn effective_size(&self, pressure: f32) -> f32 {
        match self.tool {
            ToolType::Pencil => (self.size * 0.5 * (0.3 + pressure * 0.9)).max(1.0),
            ToolType::Eraser => self.size * (0.75 + pressure * 0.6),
            ToolType::Spray => self.size * 1.5,
            ToolType::Watercolor => self.size * (0.7 + pressure * 0.6),
            _ => self.size * (0.5 + pressure * 0.9),
        }
    }
}
