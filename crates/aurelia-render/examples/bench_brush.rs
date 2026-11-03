//! Brush throughput benchmark - dependency-free, sibling to `bench_composite`.
//!
//! Measures painting on 1920x1080 layers at both storage widths, because the brush pipeline
//! is now format-generic: the question is whether 16-bit painting costs anything, and whether
//! the generic 8-bit path regressed against the pre-genericisation rasterizer.
//!
//! Run with:
//!   cargo run --release -p aurelia-render --example bench_brush

use aurelia_core::brush::{BrushPoint, BrushSettings, ToolType};
use aurelia_core::color::Color;
use aurelia_core::document::Document;
use aurelia_core::layer::LayerBuffer;
use aurelia_core::rasterizer::StrokeRasterizer;
use aurelia_core::symmetry::SymmetryConfig;
use std::time::Instant;

const W: u32 = 1920;
const H: u32 = 1080;
const DABS: u32 = 240;
const SEGMENTS: u32 = 40;
const ROUNDS: u32 = 5;

fn layer_doc(sixteen_bit: bool) -> Document {
    let mut doc = Document::new(W, H);
    doc.is_transparent = true;
    if sixteen_bit {
        let id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(id) {
            let channels: Vec<u16> = (0..(W as usize * H as usize * 4))
                .map(|index| ((index as u32 % 251) as u16) * 257)
                .collect();
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }
    }
    doc
}

fn brush() -> BrushSettings {
    let mut brush = BrushSettings::default();
    brush.tool = ToolType::Brush;
    brush.size = 26.0;
    brush.hardness = 0.8;
    brush.opacity = 0.7;
    brush.primary_color = Color::from_rgba8(220, 180, 90, 255);
    brush
}

/// One session: scattered dabs plus sustained segments, which is roughly how a stroke feels.
fn session(doc: &mut Document, symmetry: &SymmetryConfig, brush: &BrushSettings) -> f32 {
    let start = Instant::now();
    for step in 0..DABS {
        let x = (step as f32 * 7.3) % (W as f32 - 40.0) + 20.0;
        let y = (step as f32 * 11.7) % (H as f32 - 40.0) + 20.0;
        StrokeRasterizer::paint_dot(
            doc,
            BrushPoint { position: glam::Vec2::new(x, y), pressure: 1.0 },
            brush,
            symmetry,
            None,
        );
    }
    for step in 0..SEGMENTS {
        let y = (step as f32 * 23.0) % (H as f32 - 60.0) + 30.0;
        StrokeRasterizer::paint_segment(
            doc,
            BrushPoint { position: glam::Vec2::new(30.0, y), pressure: 1.0 },
            BrushPoint { position: glam::Vec2::new(W as f32 - 30.0, y + 40.0), pressure: 1.0 },
            brush,
            symmetry,
            None,
        );
    }
    start.elapsed().as_secs_f64() as f32
}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn main() {
    let cpu = std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| String::from("unknown CPU"));
    let rustc = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    println!("{cpu} | {rustc} | release");
    println!("{W}x{H}, {DABS} dabs + {SEGMENTS} full-width segments, {ROUNDS} interleaved rounds (median)
");

    let symmetry = SymmetryConfig::default();
    let brush = brush();
    let mut eight = layer_doc(false);
    let mut sixteen = layer_doc(true);
    let mut s8 = Vec::new();
    let mut s16 = Vec::new();
    // The u8-only switch exists so this benchmark can be run against rasterizers that
    // predate 16-bit support, for a before/after comparison.
    let u8_only = std::env::var("BENCH_BRUSH_U8_ONLY").is_ok();
    for _ in 0..ROUNDS {
        s8.push(session(&mut eight, &symmetry, &brush) as f64);
        if !u8_only {
            s16.push(session(&mut sixteen, &symmetry, &brush) as f64);
        }
    }
    if u8_only {
        s16.push(*s8.last().unwrap());
    }
    let m8 = median(s8.clone());
    let m16 = median(s16.clone());
    let ops = (DABS + SEGMENTS) as f64;
    println!("{:<14} {:>10} {:>14} {:>12}", "layer", "session ms", "operations/s", "vs u8");
    println!("{:<14} {:>10.1} {:>14.0} {:>12}", "Srgb8", m8 * 1000.0, ops / m8, "-");
    println!("{:<14} {:>10.1} {:>14.0} {:>11.0}%", "Srgb16", m16 * 1000.0, ops / m16, 100.0 * (m8 / m16 - 1.0));
    println!("
spread u8: {:.1}..{:.1} ms", s8.iter().cloned().fold(f64::INFINITY, f64::min) * 1000.0, s8.iter().cloned().fold(0.0, f64::max) * 1000.0);
    println!("spread u16: {:.1}..{:.1} ms", s16.iter().cloned().fold(f64::INFINITY, f64::min) * 1000.0, s16.iter().cloned().fold(0.0, f64::max) * 1000.0);
}
