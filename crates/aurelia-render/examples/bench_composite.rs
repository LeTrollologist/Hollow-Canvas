//! Composite throughput benchmark — dependency-free so it can run anywhere.
//!
//! Run with:
//!   cargo run --release -p aurelia-render --example bench_composite
//!
//! It reports megapixels composited per second for three shapes of work:
//! a plain stack, a stack with a layer mask, and a stack forced through the
//! general (offset + blending) path. Numbers are printed so a release can record
//! them; the documented floor in ROADMAP.md is what turns them into a target.

use aurelia_core::document::Document;
use aurelia_core::layer::LayerMask;
use std::time::Instant;

const W: u32 = 1920;
const H: u32 = 1080;
const LAYERS: u32 = 4;
const ITERS: u32 = 20;

fn build_doc(masked: bool, offset: bool) -> Document {
    let mut doc = Document::new(W, H);
    for i in 0..LAYERS {
        let id = if i == 0 {
            doc.active_layer_id
        } else {
            doc.add_layer(Some(format!("Layer {i}")))
        };
        if let Some(layer) = doc.get_layer(id).cloned() {
            let _ = layer;
        }
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == id) {
            // A cheap, non-uniform pattern so the blend path cannot be optimised out.
            for (idx, px) in layer.pixels.chunks_exact_mut(4).enumerate() {
                let x = (idx as u32) % W;
                let y = (idx as u32) / W;
                px[0] = (x % 256) as u8;
                px[1] = (y % 256) as u8;
                px[2] = ((x ^ y) % 256) as u8;
                px[3] = 255;
            }
            if offset && i == 1 {
                layer.offset_x = 4;
                layer.offset_y = 3;
            }
        }
        if masked && i == 2 {
            let mut mask = LayerMask::full(W, H, 255);
            for (idx, p) in mask.pixels.iter_mut().enumerate() {
                *p = if (idx % 7) == 0 { 64 } else { 255 };
            }
            if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == id) {
                layer.mask = Some(mask);
            }
        }
    }
    doc
}

fn bench(label: &str, doc: &Document) {
    // Warm up, so the first-touch page faults do not land in the measurement.
    let _ = doc.composite_layers(true);

    let start = Instant::now();
    for _ in 0..ITERS {
        std::hint::black_box(doc.composite_layers(true));
    }
    let secs = start.elapsed().as_secs_f64();
    let megapixels = (W as f64 * H as f64 * ITERS as f64) / 1_000_000.0;
    println!(
        "{label:<22} {:>7.1} MPix/s   ({:.1} ms per {}x{} composite, {} layers)",
        megapixels / secs,
        secs * 1000.0 / ITERS as f64,
        W,
        H,
        LAYERS
    );
}

fn main() {
    println!("Aurelia composite benchmark — {W}x{H}, {LAYERS} layers, {ITERS} iterations\n");
    bench("plain stack", &build_doc(false, false));
    bench("with layer mask", &build_doc(true, false));
    bench("offset + general", &build_doc(false, true));
    println!("\nTargets (see ROADMAP.md): >= 40 MPix/s plain, >= 30 MPix/s masked on a 4-core laptop.");
}
