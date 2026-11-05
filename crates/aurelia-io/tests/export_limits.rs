//! Resource-limit and validation tests for animation export.
//!
//! Every case here asserts a *structured* rejection: the error carries the numbers that caused
//! it, the failure costs no unbounded allocation, and no partial file is left behind. The
//! peak-memory counterpart lives in `export_stream_memory.rs`, in its own binary, because it
//! installs a counting global allocator and must not share a process with parallel tests.

use aurelia_core::document::Document;
use aurelia_io::export::{
    export_animated_gif, export_animated_gif_with_limits, export_flat_image,
    export_flat_image_with_limits, export_frame_sequence, export_sprite_sheet,
    export_sprite_sheet_stream, ExportError, ExportFormat, ExportLimits,
};
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "aurelia-export-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn frame(width: u32, height: u32, fill: u8) -> Vec<u8> {
    vec![fill; (width * height * 4) as usize]
}

fn assert_no_file(path: &Path) {
    assert!(
        !path.exists(),
        "a rejected export must not leave {} behind",
        path.display()
    );
}

fn assert_no_part(path: &Path) {
    let part = path.with_file_name(format!(
        "{}.part",
        path.file_name().unwrap().to_string_lossy()
    ));
    assert!(
        !part.exists(),
        "a rejected export must not leave {} behind",
        part.display()
    );
}

#[test]
fn wrong_frame_length_is_rejected_with_numbers() {
    let dir = temp_dir("bad-len");
    let path = dir.join("a.gif");

    let error = export_animated_gif(&[vec![0u8; 10]], 4, 4, 12, true, &path).expect_err("short frame");

    match error {
        ExportError::FrameSizeMismatch {
            index,
            expected,
            actual,
        } => assert_eq!((index, expected, actual), (0, 64, 10)),
        other => panic!("expected FrameSizeMismatch, got {other:?}"),
    }
    assert_no_file(&path);
    assert_no_part(&path);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dimension_over_limit_is_rejected_before_allocating() {
    let dir = temp_dir("dim");
    let path = dir.join("a.gif");
    let oversized = frame(9000, 10, 1);

    let error = export_animated_gif(&[oversized], 9000, 10, 12, true, &path).expect_err("too wide");

    match error {
        ExportError::DimensionTooLarge { width, height, max } => {
            assert_eq!((width, height, max), (9000, 10, 8192));
        }
        other => panic!("expected DimensionTooLarge, got {other:?}"),
    }
    assert_no_file(&path);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn frame_count_over_limit_is_rejected() {
    let dir = temp_dir("frames");
    let path = dir.join("a.gif");
    let limits = ExportLimits {
        max_frames: 2,
        ..ExportLimits::default()
    };
    let frames = vec![frame(4, 4, 0), frame(4, 4, 1), frame(4, 4, 2)];

    let error = export_animated_gif_with_limits(&frames, 4, 4, 12, true, &path, &limits)
        .expect_err("three frames over a limit of two");

    match error {
        ExportError::TooManyFrames { frames, max } => assert_eq!((frames, max), (3, 2)),
        other => panic!("expected TooManyFrames, got {other:?}"),
    }
    assert_no_file(&path);
    assert_no_part(&path);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn aggregate_over_limit_is_rejected() {
    let dir = temp_dir("aggregate");
    let path = dir.join("a.gif");
    let limits = ExportLimits {
        max_total_bytes: 100,
        ..ExportLimits::default()
    };
    let frames = vec![frame(4, 4, 0), frame(4, 4, 1)];

    let error = export_animated_gif_with_limits(&frames, 4, 4, 12, true, &path, &limits)
        .expect_err("128 bytes of frames over a 100 byte budget");

    match error {
        ExportError::AggregateTooLarge { required, max } => {
            assert_eq!((required, max), (128, 100));
        }
        other => panic!("expected AggregateTooLarge, got {other:?}"),
    }
    assert_no_file(&path);

    let _ = std::fs::remove_dir_all(&dir);
}

/// A sheet whose combined dimensions do not fit the container must be an error, not an
/// overflow. Run in a debug build the unchecked multiplication this replaced would have
/// panicked here; the limits are lifted so the sheet check is what rejects it.
#[test]
fn sprite_sheet_overflow_is_an_error_not_a_panic() {
    let dir = temp_dir("sheet-overflow");
    let path = dir.join("sheet.png");

    // Frames are never allocated: the sheet is sized from the iterator's length before the
    // first frame is consumed, which is exactly the point.
    let frames = (0..100_000usize).map(|_| Vec::new());

    let error = export_sprite_sheet_stream(
        frames,
        100_000,
        1,
        100_000,
        &path,
        &ExportLimits::unlimited(),
    )
    .expect_err("a 100000 x 100000 sheet cannot be encoded");

    match error {
        ExportError::SheetTooLarge { width, height } => {
            assert_eq!((width, height), (10_000_000_000, 1));
        }
        other => panic!("expected SheetTooLarge, got {other:?}"),
    }
    assert_no_file(&path);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn short_sprite_sheet_frame_is_rejected() {
    let dir = temp_dir("sheet-short");
    let path = dir.join("sheet.png");

    let error = export_sprite_sheet(&[frame(8, 8, 0), vec![0u8; 12]], 8, 8, 2, &path)
        .expect_err("second frame is too short");

    match error {
        ExportError::FrameSizeMismatch {
            index,
            expected,
            actual,
        } => assert_eq!((index, expected, actual), (1, 256, 12)),
        other => panic!("expected FrameSizeMismatch, got {other:?}"),
    }
    assert_no_file(&path);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn single_frame_at_zero_fps_still_encodes() {
    let dir = temp_dir("single");
    let path = dir.join("one.gif");

    export_animated_gif(&[frame(8, 8, 200)], 8, 8, 0, false, &path).expect("single frame export");

    let bytes = std::fs::read(&path).expect("read gif");
    assert!(bytes.starts_with(b"GIF8"), "not a GIF: {:?}", &bytes[..4]);
    assert_no_part(&path);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_partial_file_is_left_behind_on_a_mid_stream_failure() {
    let dir = temp_dir("partial");
    let path = dir.join("animation.gif");
    // Frame 1 is the wrong length, so the encoder fails after frame 0 was already written.
    let frames = vec![frame(4, 4, 0), vec![0u8; 3], frame(4, 4, 2)];

    let error = export_animated_gif(&frames, 4, 4, 12, true, &path).expect_err("bad middle frame");

    match error {
        ExportError::FrameSizeMismatch { index, .. } => assert_eq!(index, 1),
        other => panic!("expected FrameSizeMismatch, got {other:?}"),
    }
    assert_no_file(&path);
    assert_no_part(&path);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn frame_sequence_respects_the_frame_limit() {
    let dir = temp_dir("sequence");
    let limits = ExportLimits {
        max_frames: 1,
        ..ExportLimits::default()
    };
    let frames = vec![frame(4, 4, 0), frame(4, 4, 1)];

    // The wrapper takes default limits, so exercise the bounded path through the same public
    // surface the app uses by calling the stream form directly.
    let error = aurelia_io::export::export_frame_sequence_stream(
        frames,
        4,
        4,
        &dir,
        "frame",
        ExportFormat::Png,
        &limits,
    )
    .expect_err("two frames over a limit of one");

    match error {
        ExportError::TooManyFrames { frames, max } => assert_eq!((frames, max), (2, 1)),
        other => panic!("expected TooManyFrames, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn flat_export_rejects_an_oversized_document() {
    let dir = temp_dir("flat");
    let path = dir.join("big.png");
    let doc = Document::new(9000, 10);

    let error = export_flat_image(&doc, &path, ExportFormat::Png, true).expect_err("too wide");

    match error {
        ExportError::DimensionTooLarge { width, height, max } => {
            assert_eq!((width, height, max), (9000, 10, 8192));
        }
        other => panic!("expected DimensionTooLarge, got {other:?}"),
    }
    assert_no_file(&path);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn flat_export_honours_a_custom_budget() {
    let dir = temp_dir("flat-budget");
    let path = dir.join("ok.png");
    let doc = Document::new(8, 8);
    // 8 * 8 * 4 == 256 bytes, over a 128-byte budget.
    let tight = ExportLimits {
        max_total_bytes: 128,
        ..ExportLimits::default()
    };

    let error = export_flat_image_with_limits(&doc, &path, ExportFormat::Png, true, None, &tight)
        .expect_err("a single frame still has to fit the budget");

    match error {
        ExportError::AggregateTooLarge { required, max } => {
            assert_eq!((required, max), (256, 128));
        }
        other => panic!("expected AggregateTooLarge, got {other:?}"),
    }
    assert_no_file(&path);

    // The same document exports fine under the default budget.
    export_flat_image_with_limits(
        &doc,
        &path,
        ExportFormat::Png,
        true,
        None,
        &ExportLimits::default(),
    )
    .expect("8x8 export well inside the default budget");
    assert!(path.exists());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sprite_sheet_wrapper_still_packs_a_small_grid() {
    let dir = temp_dir("sheet-small");
    let path = dir.join("sheet.png");

    let (width, height) =
        export_sprite_sheet(&[frame(8, 8, 10), frame(8, 8, 20)], 8, 8, 2, &path).expect("sheet");

    assert_eq!((width, height), (16, 8));
    let bytes = std::fs::read(&path).expect("read png");
    assert_eq!(&bytes[1..4], b"PNG", "not a PNG: {:?}", &bytes[..8]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn frame_sequence_wrapper_writes_every_frame() {
    let dir = temp_dir("sequence-ok");
    let frames = vec![frame(4, 4, 0), frame(4, 4, 1), frame(4, 4, 2)];

    let paths = export_frame_sequence(&frames, 4, 4, &dir, "frame", ExportFormat::Png)
        .expect("sequence export");

    assert_eq!(paths.len(), 3);
    for path in &paths {
        assert!(path.exists(), "{} was not written", path.display());
    }

    let _ = std::fs::remove_dir_all(&dir);
}
