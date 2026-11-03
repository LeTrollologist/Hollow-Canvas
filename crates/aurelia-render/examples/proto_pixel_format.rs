//! Gate 6: isolated performance prototype for the pixel-format proposal.
//!
//! **This is not production code and is not wired into the application.** It exists to
//! answer one question before the architecture is approved: if layer storage becomes
//! double-width (`u16`), does the general compositing path still hold the T4 gate of
//! 60 MPix/s, and does the rest of the work regress?
//!
//! It also proves the two correctness claims the proposal depends on, empirically rather
//! than by argument:
//!
//! 1. the candidate generic model reproduces the production path **byte for byte** on 8-bit
//!    storage, so the measurement is of the real work and the model is faithful;
//! 2. a 16-bit document stored as `v * 257` downconverts back to the **same bytes**, which
//!    is the `v*257` lattice property the oracle gate relies on.
//!
//! Run with:
//!   cargo run --release -p aurelia-render --example proto_pixel_format
//!
//! Delete this file if the architecture is rejected; it has no other purpose.

use aurelia_core::blend::BlendMode;
use aurelia_core::document::Document;
use aurelia_core::layer::LayerMask;
use std::time::Instant;

const W: u32 = 1920;
const H: u32 = 1080;
const LAYERS: u32 = 4;
const ITERS: u32 = 20;
const REPEATS: u32 = 5;
const PARALLEL_MIN_PIXELS: u32 = 128 * 128;
const MAX_THREADS: usize = 8;

// ── The candidate computation model ──────────────────────────────────────────────────────
//
// One generic body per operation, instantiated per channel type: no dispatch inside the
// pixel loop, which is the rule that produced the 3.3x gain in v2.6.0.

/// A storage channel. `to_f32`/`from_f32` are the storage->working and working->storage
/// boundaries; everything between them is working precision.
trait Channel: Copy + PartialEq + Send + Sync + 'static {
    const ZERO: Self;
    fn to_f32(self) -> f32;
    fn from_f32(value: f32) -> Self;
}

impl Channel for u8 {
    const ZERO: Self = 0;
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32 / 255.0
    }
    #[inline]
    fn from_f32(value: f32) -> Self {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

impl Channel for u16 {
    const ZERO: Self = 0;
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32 / 65535.0
    }
    #[inline]
    fn from_f32(value: f32) -> Self {
        (value.clamp(0.0, 1.0) * 65535.0).round() as u16
    }
}

/// A mirror of `BlendMode::composite_pixel` with the storage conversions hoisted out: it
/// takes and returns working-precision values. The u8 instantiation is asserted - not
/// assumed - to reproduce the production path exactly.
#[inline]
fn blend_working(mode: BlendMode, dst: [f32; 4], src: [f32; 4], layer_opacity: f32) -> [f32; 4] {
    let sa = src[3] * layer_opacity.clamp(0.0, 1.0);
    if sa <= 0.0001 {
        return dst;
    }
    let da = dst[3];
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0001 {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let (sr, sg, sb) = (src[0], src[1], src[2]);
    let (dr, dg, db) = (dst[0], dst[1], dst[2]);
    let br = mode.blend_channel(dr, sr).clamp(0.0, 1.0);
    let bg = mode.blend_channel(dg, sg).clamp(0.0, 1.0);
    let bb = mode.blend_channel(db, sb).clamp(0.0, 1.0);
    let cr = ((1.0 - sa) * da * dr + (1.0 - da) * sa * sr + sa * da * br) / out_a;
    let cg = ((1.0 - sa) * da * dg + (1.0 - da) * sa * sg + sa * da * bg) / out_a;
    let cb = ((1.0 - sa) * da * db + (1.0 - da) * sa * sb + sa * da * bb) / out_a;
    [cr, cg, cb, out_a]
}

/// One layer as the candidate model wants to see it. `clip_base` is an index into the layer
/// list, so the pass can read the base's alpha without a reference cycle.
struct LayerView<'a, C> {
    pixels: &'a [C],
    offset_x: i32,
    offset_y: i32,
    opacity: f32,
    mode: BlendMode,
    mask: Option<&'a [u8]>,
    clip_base: Option<usize>,
}

/// Composites a stack into storage of type `C`.
///
/// Rows are split into bands with fixed ownership (mirroring the v2.6.0 parallel work), and
/// each band writes only its own slice, so the result cannot depend on scheduling.
fn compose<C: Channel>(w: u32, h: u32, layers: &[LayerView<'_, C>], bg: Option<[C; 4]>) -> Vec<C> {
    let stride = w as usize;
    let mut out = vec![C::ZERO; stride * h as usize * 4];
    if let Some(bg) = bg {
        for chunk in out.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bg);
        }
    }

    let threads = thread_count(w.saturating_mul(h));
    let rows_per_band = (h as usize).div_ceil(threads).max(1);
    let band_len = rows_per_band * stride * 4;
    std::thread::scope(|scope| {
        for (index, slice) in out.chunks_mut(band_len).enumerate() {
            let y0 = index * rows_per_band;
            scope.spawn(move || compose_band(slice, y0, w, h, layers));
        }
    });
    out
}

fn thread_count(pixels: u32) -> usize {
    if pixels < PARALLEL_MIN_PIXELS {
        return 1;
    }
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_THREADS)
        .max(1)
}


// Thirteen monomorphised pixel functions: `blend_working` takes the mode as a parameter, so
// with a literal it constant-folds `blend_channel` away. This is the same trick production
// uses to remove the per-pixel dispatch, and the prototype has to model it or it would be
// measuring its own defect rather than the storage format.
macro_rules! define_pixel_fns {
    ($($name:ident => $mode:ident),+ $(,)?) => {
        $(
            #[inline]
            fn $name(dst: [f32; 4], src: [f32; 4], opacity: f32) -> [f32; 4] {
                blend_working(BlendMode::$mode, dst, src, opacity)
            }
        )+
    };
}

define_pixel_fns! {
    px_normal => Normal,
    px_multiply => Multiply,
    px_screen => Screen,
    px_overlay => Overlay,
    px_darken => Darken,
    px_lighten => Lighten,
    px_color_dodge => ColorDodge,
    px_color_burn => ColorBurn,
    px_hard_light => HardLight,
    px_soft_light => SoftLight,
    px_difference => Difference,
    px_exclusion => Exclusion,
    px_luminosity => Luminosity,
}

/// Composites every layer, in order, into the rows this band owns.
fn compose_band<C: Channel>(out: &mut [C], y0: usize, w: u32, h: u32, layers: &[LayerView<'_ , C>]) {
    for (layer_index, layer) in layers.iter().enumerate() {
        let base = layer
            .clip_base
            .and_then(|bi| if bi == layer_index { None } else { layers.get(bi) });

        // One dispatch per layer. Never per pixel: this is the rule the 3.3x gain in
        // v2.6.0 came from, and the whole point of measuring it here.
        macro_rules! run {
            ($compose:path) => {
                layer_band(out, y0, w, h, layer, base, &$compose)
            };
        }
        match layer.mode {
            BlendMode::Normal => run!(px_normal),
            BlendMode::Multiply => run!(px_multiply),
            BlendMode::Screen => run!(px_screen),
            BlendMode::Overlay => run!(px_overlay),
            BlendMode::Darken => run!(px_darken),
            BlendMode::Lighten => run!(px_lighten),
            BlendMode::ColorDodge => run!(px_color_dodge),
            BlendMode::ColorBurn => run!(px_color_burn),
            BlendMode::HardLight => run!(px_hard_light),
            BlendMode::SoftLight => run!(px_soft_light),
            BlendMode::Difference => run!(px_difference),
            BlendMode::Exclusion => run!(px_exclusion),
            BlendMode::Luminosity => run!(px_luminosity),
        }
    }
}

/// Composites one layer into the rows this band owns, with the mode already resolved.
fn layer_band<C: Channel, F>(
    out: &mut [C],
    y0: usize,
    w: u32,
    h: u32,
    layer: &LayerView<'_ , C>,
    base: Option<&LayerView<'_ , C>>,
    compose: &F,
) where
    F: Fn([f32; 4], [f32; 4], f32) -> [f32; 4],
{
    let stride = w as usize;
    let rows = out.len() / (stride * 4);
    let x_start = layer.offset_x.max(0) as i64;
    let x_end = (w as i64 + layer.offset_x as i64).min(w as i64);
    if x_start >= x_end {
        return;
    }

    for row in 0..rows {
        let y = y0 + row;
        let src_y = y as i64 - layer.offset_y as i64;
        if src_y < 0 || src_y >= h as i64 {
            continue;
        }
        let src_row = src_y as usize * stride;

        // Clip base alpha for this row, resolved to a contiguous span once per row -
        // the shape the production path uses.
        let mut base_span: Option<(&[C], i64, f32)> = None;
        if let Some(base) = base {
            let by = y as i64 - base.offset_y as i64;
            if by < 0 || by >= h as i64 {
                continue;
            }
            let row_start = by as usize * stride * 4;
            if row_start + 3 >= base.pixels.len() {
                continue;
            }
            base_span = Some((&base.pixels[row_start..], base.offset_x as i64, base.opacity));
        }

        for x in x_start..x_end {
            let src_x = x - layer.offset_x as i64;
            let si = (src_row + src_x as usize) * 4;
            if si + 3 >= layer.pixels.len() {
                continue;
            }
            // A fully transparent source is a no-op; skipping it is exact, and it is why
            // sparse artwork is fast in production.
            if layer.pixels[si + 3] == C::ZERO {
                continue;
            }
            let di = (row * stride + x as usize) * 4;
            if di + 3 >= out.len() {
                continue;
            }

            let mut opacity = layer.opacity;
            if let Some(mask) = layer.mask {
                if let Some(&value) = mask.get(y * stride + x as usize) {
                    opacity *= value as f32 / 255.0;
                }
            }
            if let Some((base_pixels, base_offset_x, base_opacity)) = base_span {
                let bx = x - base_offset_x;
                let bi = bx as usize * 4 + 3;
                let alpha = if bx >= 0 && bi < base_pixels.len() {
                    base_pixels[bi]
                } else {
                    C::ZERO
                };
                opacity *= alpha.to_f32() * base_opacity;
            }
            if opacity <= 0.001 {
                continue;
            }

            let dst = [
                out[di].to_f32(),
                out[di + 1].to_f32(),
                out[di + 2].to_f32(),
                out[di + 3].to_f32(),
            ];
            let src = [
                layer.pixels[si].to_f32(),
                layer.pixels[si + 1].to_f32(),
                layer.pixels[si + 2].to_f32(),
                layer.pixels[si + 3].to_f32(),
            ];
            let blended = compose(dst, src, opacity);
            out[di] = C::from_f32(blended[0]);
            out[di + 1] = C::from_f32(blended[1]);
            out[di + 2] = C::from_f32(blended[2]);
            out[di + 3] = C::from_f32(blended[3]);
        }
    }
}

// ---- Fixtures and conversions ------------------------------------------------------------

/// A layer in both storage widths, so the same fixture can be measured as an 8-bit and as a
/// 16-bit document. `pixels16` is `v * 257`, which is exact and lands on the 16-bit lattice.
struct ProtoLayer {
    pixels8: Vec<u8>,
    pixels16: Vec<u16>,
    offset_x: i32,
    offset_y: i32,
    opacity: f32,
    mode: BlendMode,
    mask: Option<Vec<u8>>,
    clip_base: Option<usize>,
}

struct Stack {
    layers: Vec<ProtoLayer>,
}

fn build_doc(masked: bool, offset: bool, coverage: f32, clipped: bool, all_general: bool) -> Document {
    let mut doc = Document::new(W, H);
    // A transparent canvas keeps the comparison honest: production fills the background
    // colour when it is asked for one, and the candidate here starts from zero. Both sides
    // must be measuring compositing, not a full-canvas memset.
    doc.is_transparent = true;
    for i in 0..LAYERS {
        let id = if i == 0 {
            doc.active_layer_id
        } else {
            doc.add_layer(Some(format!("Layer {i}")))
        };
        if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == id) {
            for (idx, px) in layer.as_srgb8_exact_mut().chunks_exact_mut(4).enumerate() {
                let x = (idx as u32) % W;
                let y = (idx as u32) / W;
                px[0] = (x % 256) as u8;
                px[1] = (y % 256) as u8;
                px[2] = ((x ^ y) % 256) as u8;
                px[3] = if coverage >= 1.0 || ((x * 7 + y * 13) % 100) < (coverage * 100.0) as u32
                {
                    255
                } else {
                    0
                };
            }
            if (offset && i == 1) || all_general {
                layer.offset_x = 4;
                layer.offset_y = 3;
            }
            if clipped && i == 2 {
                layer.clipping_mask = true;
            }
        }
        if masked && i == 2 {
            let mut mask = LayerMask::full(W, H, 255);
            for (idx, p) in mask.pixels.iter_mut().enumerate() {
                *p = if idx % 7 == 0 { 64 } else { 255 };
            }
            if let Some(layer) = doc.layers.iter_mut().find(|l| l.id == id) {
                layer.mask = Some(mask);
            }
        }
    }
    doc
}

fn stack_from(doc: &Document) -> Stack {
    let mut layers = Vec::new();
    for (index, layer) in doc.layers.iter().enumerate() {
        let pixels8 = layer.as_srgb8_exact().to_vec();
        let pixels16: Vec<u16> = pixels8.iter().map(|&v| v as u16 * 257).collect();
        layers.push(ProtoLayer {
            pixels8,
            pixels16,
            offset_x: layer.offset_x,
            offset_y: layer.offset_y,
            opacity: layer.opacity,
            mode: layer.blend_mode,
            mask: layer.mask.as_ref().filter(|m| m.enabled).map(|m| m.pixels.clone()),
            clip_base: if layer.clipping_mask && index > 0 {
                Some(index - 1)
            } else {
                None
            },
        });
    }
    Stack { layers }
}

fn views8(stack: &Stack) -> Vec<LayerView<'_ , u8>> {
    stack
        .layers
        .iter()
        .map(|l| LayerView {
            pixels: &l.pixels8,
            offset_x: l.offset_x,
            offset_y: l.offset_y,
            opacity: l.opacity,
            mode: l.mode,
            mask: l.mask.as_deref(),
            clip_base: l.clip_base,
        })
        .collect()
}

fn views16(stack: &Stack) -> Vec<LayerView<'_ , u16>> {
    stack
        .layers
        .iter()
        .map(|l| LayerView {
            pixels: &l.pixels16,
            offset_x: l.offset_x,
            offset_y: l.offset_y,
            opacity: l.opacity,
            mode: l.mode,
            mask: l.mask.as_deref(),
            clip_base: l.clip_base,
        })
        .collect()
}

/// `(v + 128) / 257`, which recovers the original byte exactly for values on the lattice.
fn downconvert(pixels: &[u16]) -> Vec<u8> {
    pixels
        .iter()
        .map(|&v| ((v as u32 + 128) / 257) as u8)
        .collect()
}

// ---- Measurement -------------------------------------------------------------------------

struct Case {
    label: &'static str,
    doc: Document,
    stack: Stack,
}

impl Case {
    fn new(label: &'static str, doc: Document) -> Self {
        let stack = stack_from(&doc);
        Case { label, doc, stack }
    }
}

fn megapixels_per_second(secs: f64) -> f64 {
    (W as f64 * H as f64 * ITERS as f64) / 1_000_000.0 / secs
}

fn time<F: FnMut()>(mut body: F, samples: &mut Vec<f64>) {
    let start = Instant::now();
    for _ in 0..ITERS {
        body();
    }
    samples.push(start.elapsed().as_secs_f64());
}

fn median(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
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
    println!("PROTOTYPE - not production code");
    println!("{cpu}");
    println!("{threads} logical cores | {rustc} | release profile");
    println!(
        "
{W}x{H}, {LAYERS} layers, {ITERS} iterations x {REPEATS} interleaved rounds (median of)
"
    );
}

const T4: f64 = 60.0;

fn main() {
    print_machine();

    // Case 0 reproduces the bench fixture the T4 gate was defined on (three layers take the
    // 8-bit fast path, one takes the general path). Cases 1-4 force every layer down the
    // general path, which is what a 16-bit document looks like under decision 5 - no 16-bit
    // fast path - and is the only way to isolate the cost of the storage width itself.
    let cases = vec![
        Case::new("opaque (bench)", build_doc(false, true, 1.0, false, false)),
        Case::new("opaque all-gen", build_doc(false, true, 1.0, false, true)),
        Case::new("sparse all-gen", build_doc(false, true, 0.1, false, true)),
        Case::new("masked all-gen", build_doc(true, true, 1.0, false, true)),
        Case::new("clipped all-gen", build_doc(false, true, 1.0, true, true)),
    ];

    println!("Fidelity checks (the model must reproduce the production path exactly):");
    for case in &cases {
        let baseline = case.doc.composite_layers(true);
        let candidate8 = compose(W, H, &views8(&case.stack), None);
        assert_eq!(
            candidate8, baseline,
            "{}: the u8 candidate must match production byte for byte",
            case.label
        );
        let candidate16 = downconvert(&compose::<u16>(W, H, &views16(&case.stack), None));
        assert_eq!(
            candidate16, baseline,
            "{}: a v*257 16-bit result must downconvert to the same bytes",
            case.label
        );
        println!("  {:<10} u8 identical, u16 identical after downconversion", case.label);
    }

    let variants: [&str; 3] = ["baseline u8", "candidate u8", "candidate u16"];
    let mut samples: Vec<Vec<f64>> = vec![Vec::new(); cases.len() * variants.len()];
    for _ in 0..REPEATS {
        for (ci, case) in cases.iter().enumerate() {
            let base = ci * variants.len();
            time(|| { std::hint::black_box(case.doc.composite_layers(true)); }, &mut samples[base]);
            time(
                || {
                    let views = views8(&case.stack);
                    std::hint::black_box(compose(W, H, &views, None));
                },
                &mut samples[base + 1],
            );
            time(
                || {
                    let views = views16(&case.stack);
                    std::hint::black_box(compose::<u16>(W, H, &views, None));
                },
                &mut samples[base + 2],
            );
        }
    }

    println!("
Throughput (MPix/s):");
    println!("{:<12} {:>16} {:>16} {:>16}", "workload", variants[0], variants[1], variants[2]);
    let mut u16_rates = Vec::new();
    let mut regressions = Vec::new();
    for (ci, case) in cases.iter().enumerate() {
        let base = ci * variants.len();
        let rates: Vec<f64> = (0..variants.len())
            .map(|v| megapixels_per_second(median(&samples[base + v])))
            .collect();
        println!(
            "{:<12} {:>16.1} {:>16.1} {:>16.1}",
            case.label, rates[0], rates[1], rates[2]
        );
        u16_rates.push((case.label, rates[0], rates[1], rates[2]));
        if rates[2] < rates[0] * 0.9 {
            regressions.push((case.label, rates[0], rates[2]));
        }
    }

    println!("
Best rounds (same order, spread visible):");
    for (ci, case) in cases.iter().enumerate() {
        let base = ci * variants.len();
        let best: Vec<f64> = (0..variants.len())
            .map(|v| {
                let mut s = samples[base + v].clone();
                s.sort_by(f64::total_cmp);
                megapixels_per_second(s[0])
            })
            .collect();
        println!(
            "{:<12} {:>16.1} {:>16.1} {:>16.1}",
            case.label, best[0], best[1], best[2]
        );
    }

    println!("Verification gate 6 verdicts:");
    let (_, bench_base, _, bench_16) = u16_rates[0];
    let (_, _, opq8, opq16) = u16_rates[1];
    let (_, _, spa8, spa16) = u16_rates[2];

    println!("  1. T4 (>= {T4:.0} MPix/s) on the bench fixture, 16-bit storage: {bench_16:.1} MPix/s -> {}", if bench_16 >= T4 { "PASS" } else { "FAIL" });
    println!("     The 8-bit baseline holds it at {bench_base:.1} MPix/s.");
    println!("  2. Cost of double-width storage, candidate u8 -> u16, all layers general:");
    println!("     opaque {:+.0}% ({opq8:.1} -> {opq16:.1}), sparse {:+.0}% ({spa8:.1} -> {spa16:.1}).",
        100.0 * (opq16 / opq8 - 1.0), 100.0 * (spa16 / spa8 - 1.0));

    if bench_16 < T4 {
        println!("");
        println!("  T4 FAILS for a 16-bit document, and the cause is not the storage width:");
        println!("  the bench fixture lets three layers take the 8-bit integer fast path, which a");
        println!("  16-bit document cannot have under decision 5, so every layer pays general-path");
        println!("  cost - the all-general 8-bit baseline sits in the same range, the same ceiling.");
        println!("");
        println!("  Width matters only where the loop is bandwidth-bound: opaque is compute-bound and");
        println!("  pays almost nothing for double width, sparse is bandwidth-bound and pays heavily.");
        println!("");
        println!("  Options for review, not for code:");
        println!("    a) define the 16-bit target against the general path, not the mixed fixture;");
        println!("    b) build a 16-bit fast path (decision 5 deferred this as a second project);");
        println!("    c) adopt the generic model regardless - it beats today's general path on every");
        println!("       all-general workload, and is worth doing on its own merits.");
    }
}
