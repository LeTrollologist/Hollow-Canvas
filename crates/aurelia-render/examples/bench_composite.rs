//! Composite throughput benchmark — dependency-free so it can run anywhere.
//!
//! Run with:
//!   cargo run --release -p aurelia-render --example bench_composite
//!
//! It reports megapixels composited per second for the shapes of work the compositor
//! actually does: a plain stack, a masked stack, the general (offset + blend) path at
//! several coverages, a clipping layer, and every blend mode on its own. The machine is
//! printed alongside the numbers, because a throughput figure without the CPU, core count
//! and compiler version cannot be compared with anything.
//!
//! Every case is measured `REPEATS` times and the best run is reported: on a laptop a
//! single measurement moves by 10% or more with thermal and turbo state, which is larger
//! than the differences this benchmark exists to detect.

use aurelia_core::blend::BlendMode;
use aurelia_core::document::Document;
use aurelia_core::layer::LayerMask;
use std::time::Instant;

const W: u32 = 1920;
const H: u32 = 1080;
const LAYERS: u32 = 4;
const ITERS: u32 = 20;
const REPEATS: u32 = 3;

fn build_doc(masked: bool, offset: bool, coverage: f32) -> Document {
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
                // `coverage` models real artwork: most of a layer is empty.
                px[3] = if coverage >= 1.0 || ((x * 7 + y * 13) % 100) < (coverage * 100.0) as u32 {
                    255
                } else {
                    0
                };
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

/// A stack whose third layer is clipped to the second one, which is the shape that makes
/// the compositor re-read its base layer's alpha for every pixel.
fn build_clip_doc(coverage: f32) -> Document {
    let mut doc = build_doc(false, true, coverage);
    let id = doc.layers[2].id;
    if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == id) {
        layer.clipping_mask = true;
    }
    doc
}

/// The reference stack with the *offset* layer set to `mode`.
///
/// Only one layer carries the mode: setting it on all four would send every layer down
/// the general path and the row would measure the general path four times over instead of
/// how expensive that one mode is.
fn build_mode_doc(mode: BlendMode) -> Document {
    let mut doc = build_doc(false, true, 1.0);
    if let Some(layer) = doc.layers.get_mut(1) {
        layer.blend_mode = mode;
    }
    doc
}

/// One measured shape of work. Built on demand so a long run does not hold a dozen
/// 1920x1080 layer stacks (each ~33 MB) in memory at once.
struct Case {
    label: &'static str,
    make: fn() -> Document,
}

/// Which release tier a throughput figure reaches.
fn tier(rate: f64) -> &'static str {
    if rate >= 60.0 {
        "T4"
    } else if rate >= 50.0 {
        "T3"
    } else if rate >= 40.0 {
        "T2"
    } else if rate >= 30.0 {
        "T1"
    } else {
        "below T1"
    }
}

/// Times every case once per round and keeps the best round per case.
///
/// Cases are interleaved rather than measured one after another: a laptop that throttles
/// under sustained load would otherwise make the first case look fast and the last slow,
/// which is exactly the mistake this benchmark exists to avoid.
fn run(cases: &[Case]) -> Vec<f64> {
    let megapixels = (W as f64 * H as f64 * ITERS as f64) / 1_000_000.0;
    let mut best = vec![f64::INFINITY; cases.len()];
    for _ in 0..REPEATS {
        for (idx, case) in cases.iter().enumerate() {
            let doc = (case.make)();
            // Warm up, so first-touch page faults do not land in the measurement.
            let _ = doc.composite_layers(true);
            let start = Instant::now();
            for _ in 0..ITERS {
                std::hint::black_box(doc.composite_layers(true));
            }
            best[idx] = best[idx].min(start.elapsed().as_secs_f64());
        }
    }
    best.iter().map(|secs| megapixels / secs).collect()
}

fn report(label: &str, rate: f64) {
    println!("{label:<26} {rate:>7.1} MPix/s");
}

fn print_machine() {
    let cpu = std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| String::from("unknown CPU"));
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let rustc = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| String::from("unknown rustc"));
    println!("{cpu}");
    println!("{threads} logical cores | {rustc} | release profile");
}

fn main() {
    print_machine();
    println!(
        "\nAurelia composite benchmark - {W}x{H}, {LAYERS} layers,          {ITERS} iterations x {REPEATS} interleaved rounds (best of)"
    );

    let mut cases = vec![
        Case { label: "plain stack", make: || build_doc(false, false, 1.0) },
        Case { label: "with layer mask", make: || build_doc(true, false, 1.0) },
        // The general path: what the offset/blend branch costs, and how much of that cost
        // is avoidable work on transparent pixels.
        Case { label: "general, opaque", make: || build_doc(false, true, 1.0) },
        Case { label: "general, 50% coverage", make: || build_doc(false, true, 0.5) },
        Case { label: "general, 10% coverage", make: || build_doc(false, true, 0.1) },
        Case { label: "general, 10% + mask", make: || build_doc(true, true, 0.1) },
        Case { label: "general, clipping", make: || build_clip_doc(1.0) },
        Case { label: "general, clipping 10%", make: || build_clip_doc(0.1) },
    ];
    // One case per blend mode: they no longer share a loop, so a slow mode has to be
    // visible instead of being averaged away behind Normal.
    for mode in BlendMode::ALL {
        cases.push(Case {
            label: mode.label(),
            make: mode_doc(*mode),
        });
    }

    let rates = run(&cases);
    let mode_count = BlendMode::ALL.len();
    let split = cases.len() - mode_count;

    println!();
    for (case, rate) in cases[..split].iter().zip(&rates[..split]) {
        report(case.label, *rate);
    }
    println!("\nAll {mode_count} blend modes (opaque, general path):");
    for (case, rate) in cases[split..].iter().zip(&rates[split..]) {
        report(&format!("  {}", case.label), *rate);
    }

    let opaque = rates[2];
    let slowest = cases[split..]
        .iter()
        .zip(&rates[split..])
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map(|(case, rate)| (case.label, *rate))
        .unwrap_or(("?", 0.0));

    println!("\nRelease tiers - general (offset+blend) path, opaque:");
    println!("  T1 30   T2 40   T3 50   T4 60 MPix/s");
    println!("  opaque {opaque:.1} MPix/s -> {}", tier(opaque));
    println!("  slowest mode: {} at {:.1} MPix/s", slowest.0, slowest.1);
    println!(
        "  plain {:.1} MPix/s (floor 40), masked {:.1} MPix/s (floor 30)",
        rates[0], rates[1]
    );
}

/// `fn() -> Document` cannot capture, so the mode is carried in a fresh wrapper per mode.
fn mode_doc(mode: BlendMode) -> fn() -> Document {
    macro_rules! wrappers {
        ($($name:ident => $mode:ident),+ $(,)?) => {
            $(
                fn $name() -> Document {
                    build_mode_doc(BlendMode::$mode)
                }
            )+
            match mode {
                $(BlendMode::$mode => $name,)+
            }
        };
    }
    wrappers! {
        doc_normal => Normal,
        doc_multiply => Multiply,
        doc_screen => Screen,
        doc_overlay => Overlay,
        doc_darken => Darken,
        doc_lighten => Lighten,
        doc_color_dodge => ColorDodge,
        doc_color_burn => ColorBurn,
        doc_hard_light => HardLight,
        doc_soft_light => SoftLight,
        doc_difference => Difference,
        doc_exclusion => Exclusion,
        doc_luminosity => Luminosity,
    }
}
