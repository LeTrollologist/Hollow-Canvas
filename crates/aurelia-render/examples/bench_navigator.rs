//! Navigator thumbnail cost — the live-refresh budget.
//!
//! The Navigator rebuilds a 320 px thumbnail from the last composite while a stroke is in
//! progress. That work is small, but it happens per frame; this benchmark measures it rather
//! than assuming. It reports the median of several rounds for 1080p and 4K documents.
//!
//! Run with:
//!   cargo run --release -p aurelia-render --example bench_navigator

use aurelia_core::document::Document;
use aurelia_render::software::SoftwareRenderer;
use glam::Vec2;
use std::hint::black_box;
use std::time::Instant;

const THUMB_W: usize = 320;
const ITERS: u32 = 200;
const ROUNDS: u32 = 7;

fn build_doc(w: u32, h: u32) -> Document {
    let mut doc = Document::new(w, h);
    let id = doc.active_layer_id;
    if let Some(layer) = doc.get_layer_mut(id) {
        let pixels = layer.as_srgb8_exact_mut();
        for (idx, px) in pixels.chunks_exact_mut(4).enumerate() {
            let x = (idx as u32) % w;
            let y = (idx as u32) / w;
            px[0] = (x % 256) as u8;
            px[1] = (y % 256) as u8;
            px[2] = ((x ^ y) % 256) as u8;
            px[3] = if (x * 7 + y * 13) % 100 < 80 { 255 } else { 0 };
        }
    }
    doc
}

fn main() {
    println!(
        "{} | aurelia-render {} | release",
        std::env::consts::ARCH,
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "Navigator thumbnail: {THUMB_W} px wide, {ITERS} iterations x {ROUNDS} rounds, median"
    );
    println!("{:>12} {:>12} {:>14}", "document", "median ms", "max fps");

    for (w, h) in [(1920u32, 1080u32), (3840, 2160)] {
        let doc = build_doc(w, h);
        let mut renderer = SoftwareRenderer::new();
        let mut frame = vec![0u32; (w as usize) * (h as usize)];
        renderer.render_canvas(
            &mut frame,
            w as usize,
            h as usize,
            &doc,
            Vec2::ZERO,
            (w as f32 / 1920.0).max(0.05),
            Vec2::ZERO,
            [0, 0, w as usize, h as usize],
            None,
            &[],
            None,
        );

        let mut samples = Vec::with_capacity(ROUNDS as usize);
        for _ in 0..ROUNDS {
            let start = Instant::now();
            for _ in 0..ITERS {
                black_box(
                    renderer
                        .navigator_thumbnail(black_box(&doc), THUMB_W)
                        .expect("thumbnail"),
                );
            }
            samples.push(start.elapsed().as_secs_f64() * 1000.0 / ITERS as f64);
        }
        samples.sort_by(f64::total_cmp);
        let median = samples[samples.len() / 2];
        println!(
            "{:>12} {:>12.3} {:>14.0}",
            format!("{w}x{h}"),
            median,
            1000.0 / median
        );
    }

    println!("\nBudget: a refresh should stay well below one 60 Hz frame (16.7 ms).");
}
