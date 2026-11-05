//! Peak-memory proof for streaming animation export.
//!
//! The failure mode being guarded is an allocation, not an error code: an exporter that
//! materialises the whole animation still returns `Ok`, it just reserves the entire animation
//! first. So this measures peak heap rather than asserting on a return value.
//!
//! It lives in its own test binary because the counting allocator is process-global and would
//! otherwise measure the other tests running in parallel.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use aurelia_io::export::{export_animated_gif_stream, ExportLimits};

struct CountingAllocator;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn record(size: usize) {
    let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            record(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            if new_size >= layout.size() {
                record(new_size - layout.size());
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// Peak heap growth above the live set at the moment of the call.
fn peak_growth<F: FnOnce()>(run: F) -> usize {
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    run();
    PEAK.load(Ordering::SeqCst).saturating_sub(baseline)
}

const SIDE: u32 = 64;
const FRAMES: usize = 200;
const FRAME_BYTES: usize = (SIDE * SIDE * 4) as usize;
const TOTAL: usize = FRAMES * FRAME_BYTES;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("aurelia-export-mem-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// A lazy frame source: each frame is allocated as the exporter asks for it, so the hook never
/// holds the animation itself.
fn lazy_frames() -> impl ExactSizeIterator<Item = Vec<u8>> {
    (0..FRAMES).map(|index| {
        let mut frame = vec![0u8; FRAME_BYTES];
        frame[0] = index as u8;
        frame
    })
}

/// Both measurements live in one test because the counting allocator is global: parallel
/// tests would each see the other's allocations as their own peak.
#[test]
fn animation_export_does_not_retain_the_whole_animation() {
    let dir = temp_dir("both");
    let gif_path = dir.join("animation.gif");

    let gif_growth = peak_growth(|| {
        export_animated_gif_stream(
            lazy_frames(),
            SIDE,
            SIDE,
            12,
            true,
            &gif_path,
            &ExportLimits::default(),
        )
        .expect("gif export");
    });

    assert!(gif_path.exists(), "gif export produced no file");
    assert!(
        gif_growth < TOTAL / 4,
        "GIF export peaked {gif_growth} bytes above baseline for a {TOTAL}-byte animation; \
         it is retaining more than a small multiple of one {FRAME_BYTES}-byte frame"
    );

    let sheet_path = dir.join("sheet.png");
    let sheet_growth = peak_growth(|| {
        aurelia_io::export::export_sprite_sheet_stream(
            lazy_frames(),
            SIDE,
            SIDE,
            16,
            &sheet_path,
            &ExportLimits::default(),
        )
        .expect("sheet export");
    });

    assert!(sheet_path.exists(), "sheet export produced no file");
    // The sheet itself is ~TOTAL bytes (1024x832x4), so the bound is "the sheet plus slack",
    // not "one frame": anything near 2x means the frames are also being retained.
    assert!(
        sheet_growth < TOTAL + TOTAL / 2,
        "sprite sheet export peaked {sheet_growth} bytes above baseline for a {TOTAL}-byte \
         sheet; frames are being retained alongside the sheet"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

