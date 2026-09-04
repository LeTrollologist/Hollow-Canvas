pub mod animation;
pub mod blend;
pub mod brush;
pub mod color;
pub mod document;
pub mod filter;
pub mod history;
pub mod layer;
pub mod perspective;
pub mod rasterizer;
pub mod selection;
pub mod symmetry;
pub mod transform;

pub use animation::{AnimationFrame, AnimationTimeline};
pub use blend::BlendMode;
pub use brush::{BrushPoint, BrushPreset, BrushSettings, EraserMode, GradientType, ShapeFillMode, ToolType};
pub use color::{Color, ThemeMode, DEFAULT_PALETTE};
pub use document::Document;
pub use history::{Command, HistoryStack, LayerPixelsSnapshotCommand};
pub use layer::{AdjustmentConfig, AdjustmentType, Layer, LayerId, LayerKind};
pub use perspective::{PerspectiveConfig, PerspectiveType};
pub use rasterizer::StrokeRasterizer;
pub use selection::{SelectionMask, StrokePosition};
pub use symmetry::{SymmetryConfig, SymmetryMode};
pub use transform::{AffineTransform2D, render_transformed_patch, sample_bilinear, sample_nearest};

#[cfg(test)]
mod tests;
