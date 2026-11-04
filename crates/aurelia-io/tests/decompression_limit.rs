//! Regression test for unbounded decompression when loading AUR projects.
//!
//! A small deflate stream can expand to many gigabytes. The loader must stop at the
//! payload size implied by the (already validated) canvas dimensions instead of
//! buffering the whole expansion and only then comparing lengths.
//!
//! The test measures peak heap use with a tracking allocator because the failure mode
//! is an allocation, not an error code: a parser that decompresses first still returns
//! `CorruptData`, it just does so after reserving the bomb.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use aurelia_core::document::Document;
use aurelia_io::project::{load_project_from_reader, save_project_to_writer};
use aurelia_io::ProjectError;
use flate2::write::DeflateEncoder;
use flate2::Compression;

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

const BOMB_RAW: usize = 64 * 1024 * 1024;
const SIDE: u32 = 4;

/// Offset of the first layer's comp_len field in an AUR v3 file produced by the
/// writer: fixed header, then the layer id/name/kind/parent/flags/opacity/blend/
/// offsets/format fields.
fn comp_len_offset(bytes: &[u8]) -> usize {
    let mut at = 4 + 4 + 4 + 8 + 1 + 1 + 1 + 1 + 4; // magic, w, h, active id, bg, alpha, theme, format, count
    at += 8; // layer id
    let name_len = u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    at += 2 + name_len;
    at += 1 + 8 + 1 + 4 + 1 + 4 + 4 + 1; // kind, parent, flags, opacity, blend, offsets, format tag
    at
}

#[test]
fn decompression_bomb_is_rejected_without_allocating_it() {
    // A real project, so the header and layer prefix are exactly what the writer emits.
    let mut doc = Document::new(SIDE, SIDE);
    doc.is_transparent = true;
    let mut bytes = Vec::new();
    save_project_to_writer(&doc, &mut bytes).expect("save fixture");

    let comp_len_at = comp_len_offset(&bytes);
    let data_at = comp_len_at + 4;
    let original_len =
        u32::from_le_bytes(bytes[comp_len_at..data_at].try_into().unwrap()) as usize;
    assert!(
        original_len > 0 && original_len < (SIDE * SIDE * 4) as usize,
        "the fixture should carry a small deflate stream for its 4x4 sRGB8 layer"
    );

    // Replace that payload with a stream that expands far beyond the 64-byte layer.
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    let chunk = vec![0u8; 1024 * 1024];
    for _ in 0..BOMB_RAW / chunk.len() {
        encoder.write_all(&chunk).expect("compress");
    }
    let bomb = encoder.finish().expect("finish");
    assert!(
        bomb.len() < 256 * 1024,
        "fixture should be a small stream that expands to {} bytes, got {} bytes",
        BOMB_RAW,
        bomb.len()
    );

    let mut crafted = bytes[..comp_len_at].to_vec();
    crafted.extend_from_slice(&(bomb.len() as u32).to_le_bytes());
    crafted.extend_from_slice(&bomb);
    crafted.push(0); // no mask on this layer

    let baseline = LIVE.load(Ordering::Relaxed);
    PEAK.store(baseline, Ordering::Relaxed);

    let result = load_project_from_reader(Cursor::new(&crafted));

    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(baseline);
    assert!(
        matches!(result, Err(ProjectError::CorruptData)),
        "an oversized payload must be rejected as corrupt"
    );
    assert!(
        peak < 8 * 1024 * 1024,
        "loading a {} byte file must not allocate {} bytes (the bomb expands to {} bytes)",
        crafted.len(),
        peak,
        BOMB_RAW
    );
}
