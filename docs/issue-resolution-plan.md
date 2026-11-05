# Issue resolution plan

Status: proposal, not approved. Nothing in this document is implemented.
Baseline: `main` @ `3ecf8cd` (v3.2.0), 112 tests passing, 1 ignored.

Scope: issues #20, #15, #14, #13, plus one add-on requested by the maintainer (D8,
navigator/viewport — no issue number exists for it).

Out of scope by instruction: **#5 Authenticode certificate** is excluded and not
considered anywhere below. It is the only open issue this plan does not address.

---

## 1. Inventory

| # | Title | Type | Effort | Risk | Target |
| :-- | :--- | :--- | :--- | :--- | :--- |
| #20 | Validate animation dimensions and enforce export resource limits | bug | S–M | low | v3.2.1 |
| #15 | Add native 16-bit compositing output | enhancement | M | medium | v3.3.0 |
| #14 | Add 16-bit support to text rendering | enhancement | M | medium | v3.3.0 |
| #13 | Add UI support for 16-bit documents | enhancement | L | **high** | v3.4.0 |
| D8 | Navigator / viewport improvements (add-on, no issue number) | enhancement | M | low–medium | v3.3.0 |

#13 is the largest and the only high-risk item, because it is the change that makes
previously-unreachable code paths reachable. It is deliberately last.

D8 was added at the maintainer's request and is independent of the 16-bit work, so it is the
one item that can be taken at any point without blocking or being blocked.

---

## 2. Repository model, as it affects these issues

Six crates; all platform-agnostic except `aurelia-app`, which is the Win32 host.

| Crate | Role | Relevance |
| :--- | :--- | :--- |
| `aurelia-core` | document, layers, pixel storage, compositor, rasterizer, filters, history | #14, #15, most of #13 |
| `aurelia-io` | AUR v1–v3 project container, image export | #20, #13 (persistence) |
| `aurelia-ui` | egui panels, dialogs, app state | #13, #20 (export dialog) |
| `aurelia-app` | Win32 message loop, pointer input, file actions | #13 (stroke capture), #20 (call sites) |
| `aurelia-render` | software compositor + benchmark examples | #15 (benchmarks) |

Invariants the plan must not break:

1. **Byte-identical 8-bit output.** The 702-combination compositor oracle
   (`crates/aurelia-core/src/tests.rs:1549`+) and the 234 clipping corners are the gate for
   every refactor. Nothing here may relax them.
2. **`v * 257` lattice.** An 8-bit document is an exact subset of a 16-bit one
   (`crates/aurelia-core/src/pixel.rs:115-134`). This is what makes the oracle reusable.
3. **Format-agnostic history.** `LayerPixelsDeltaCommand` stores byte spans and replays them
   through `write_storage_bytes`, so it never interprets a pixel
   (`crates/aurelia-core/src/history.rs:120-140`).
4. **`as_srgb8_exact()` panics** on a 16-bit layer by design
   (`crates/aurelia-core/src/layer.rs:390-410`). Its invariant is documented as *"nothing in
   the application creates a non-`Srgb8` layer yet"*. **#13 is the change that retires that
   invariant**, which is why it needs a hardening pass first.

---

## 3. Findings from reading the code

These are the things the issue text does not say, and they drive the sequencing.

### F1 — 16-bit painting works in core but panics in the app (critical, gates #13)

`StrokeRasterizer::blend_stamp` is generic over `Channel` and is tested on `Srgb16`
(`tests.rs:2004`+). But the application records stroke undo by copying the active layer as
8-bit bytes:

- `crates/aurelia-app/src/main.rs:1375` (Fill), `:1392` (Gradient), `:1471`, `:1503`, `:1525`
  (brush/eraser/other tools) — `layer.as_srgb8_exact().to_vec()`
- `crates/aurelia-app/src/main.rs:1726` — the `after` capture that builds the delta command

So the first brush stroke, fill or gradient on a 16-bit layer panics in `aurelia-app`, even
though the rasterizer supports it. **This is the single biggest gap: the 16-bit painting work
is unreachable through the product.**

Fix is small: `Layer::storage_bytes() -> Vec<u8>` (a `read_storage_bytes` wrapper) and use it
at those six sites. The delta command already consumes raw bytes, so undo/redo keeps working
for both widths with no other change.

### F2 — The compositor silently drops adjustment layers for 16-bit documents

`composite_layers_into` handles adjustment layers in the 8-bit path
(`document.rs:588-635`). `compose_u16_prepared` — the path taken when *any* layer is 16-bit —
skips them at `document.rs:1109-1115` (`layer.is_adjustment()` → `continue`).

So a 16-bit document with an adjustment layer renders **differently**, not just at lower
precision: the adjustment disappears. This contradicts #15's acceptance criterion "mixed-format
compositing produces correct results" and would be a visible bug the day #13 ships.

Root cause: `AdjustmentType::apply_to_rgb` is `(u8,u8,u8) -> (u8,u8,u8)` with `255.0`/`128.0`
hardcoded (`layer.rs:105`+). It needs a working-precision sibling.

### F3 — Structural document operations are 8-bit only (gates #13)

`Document::flip` (`document.rs:402`), `rotate_90` (`:422`, and `rotate_180` via flip), and
`resize_canvas` (`:457`) all build `Vec<u8>` and call `set_pixels`, which replaces the buffer
with `LayerBuffer::Srgb8` (`layer.rs:501`). On a 16-bit layer these panic at the first
`as_srgb8_exact()`. Note `scale_canvas` (`document.rs:497`) is already generic — the pattern to
copy is right next door.

### F4 — Text panics on the interactive path and returns `false` on the layer path (this is #14)

- `rasterize_text` (`rasterizer.rs:1613-1750`, the tool path) has **no format guard**. It calls
  `as_srgb8_exact()` / `as_srgb8_exact_mut()` at `:1712-1731` → panic on a 16-bit layer.
- `rasterize_text_layer_config` (`:1493`) returns `false` for non-8-bit and clears the buffer
  first (`:1508`), which is the deliberate refusal documented in `docs/16bit-plan.md`.

So #14 is not only an enhancement; it removes a live panic in the tool path.

### F5 — No deterministic font, so #14's "byte-identical" cannot be proven as-is

`StrokeRasterizer::load_font` (`rasterizer.rs:1286`) loads from OS paths
(`C:\Windows\Fonts\segoeui.ttf` …). There is **no bundled font anywhere in the repo** (no
`.ttf`/`.otf` under `assets/`). A golden digest for text would therefore be machine-specific.

#14's acceptance criterion "Existing 8-bit output remains byte-identical" needs a bundled font
(OFL/Bitstream Vera–licensed, small). Per D5 it lives in `assets/fonts/` behind the asset seam,
not under `tests/fixtures/`, because brush imports will need the same source → decode shape.
Without it the digest is worthless. This is a real prerequisite, not paperwork.

### F6 — Every filter is 8-bit only, and #13 exposes 15 panic sites

All 15 public filters in `crates/aurelia-core/src/filter.rs` take `&mut [u8]`
(`filter_invert` … `filter_motion_blur`). Called from:

- `crates/aurelia-ui/src/ui.rs:148-152` (filter preview), `:601-629` (Invert/Grayscale/Sepia),
  `:2752`, `:2805`, `:2858`, `:2903`, `:2919`, `:2969`, `:3019`, `:3042`, `:3093`, `:3145`,
  `:3167` (the modal filters)
- `crates/aurelia-ui/src/state.rs:1223-1260`
- `crates/aurelia-app/src/main.rs:1956-1958`

All panic on a 16-bit layer. There is **no open issue for generic filters**. Per D2 they are
blocked with clear feedback rather than reimplemented inside #13: the capability guard and the
format-aware destination writes land with #13, and the generic filter pass is the first
follow-up after v3.4.0 (§8).

### F7 — Other 8-bit-only operations that #13 would expose

| Operation | Site | Recommendation |
| :--- | :--- | :--- |
| `Select Layer Alpha` / `Intersect with Layer Alpha` | `ui.rs:476`, `:496` → `SelectionMask::from_layer_alpha(&[u8], w, h, threshold)` (`selection.rs:239`) | format-aware alpha sampler; matching at 8-bit is acceptable and precedented (flood fill, `docs/16bit-plan.md`) |
| `fill_selection` / `stroke_selection` | `ui.rs:2752`+ (via `state.rs:1417-1461`) | genericize (same `Channel` pattern) or block |
| `FullDocumentSnapshotCommand::cost()` | `history.rs:294` — `as_srgb8_exact().len()` | one-line fix to `storage_len()`; the type has no production users today but is a trap |
| `LayerPixelsSnapshotCommand` undo/redo | `history.rs:40-49` | move to `read/write_storage_bytes`, or delete (only tests construct it) |
| Filter preview restore | `ui.rs:148-149` | guarded by the filter block |
| Image import as layer | `aurelia-app` open-image path | promote `v*257` into the document format, or keep 8-bit and allow mixed |

108 `as_srgb8_exact` call sites exist outside tests/examples, across
`app` (9), `core/animation` (5), `core/document` (9), `core/history` (10), `core/layer` (4),
`core/rasterizer` (16), `ui/state` (30), `ui/ui` (25).

### F8 — There is no document-level format (#13 needs one)

- `Document` has no format field (`document.rs:8-18`).
- `Document::new` / `add_layer` always create `Srgb8` (`document.rs:23`, `:69`).
- The AUR v3 writer *derives* the document tag: `any(layer is 16-bit)`
  (`project.rs:84-93`). The reader reads the tag but only uses it to resolve `INHERIT`
  (`project.rs:362-371`, `:661-671`).
- Consequence: create a 16-bit document, delete its only 16-bit layer, add a new layer → the
  document silently becomes 8-bit and saves as 8-bit, violating #13's "Preserve the storage
  format when saving".

Decided (D1): add `Document::pixel_format: PixelFormat` (serde `default` = `Srgb8`), persist it
as the existing document tag, and have `add_layer` / `add_text_layer` inherit it. Mixed
documents stay representable through the per-layer tags, so the file format does not change.

### F9 — #20: memory amplification is in the caller, not only the encoder

`export_animated_gif` (`export.rs:105-139`):

- `frame_data.clone()` at `:132` is required because `RgbaImage::from_raw` takes `Vec<u8>`
  while the API takes `&[Vec<u8>]`. That is one extra frame at a time — acceptable.
- **The real cost is the caller.** `ui.rs:4003-4007` composites *every* timeline frame into
  `frames_rgba` before exporting, and the timeline has **no frame cap** (`AnimationTimeline::
  add_frame`, `animation.rs:196`; UI calls it at `state.rs:759`). A 4096×4096 document at 30
  frames is ~2 GB resident before the encoder starts.
- Aggregate arithmetic is unchecked: `sheet_w = width * cols`, `sheet_h = height * rows`
  (`export.rs:180-181`), `vec![0u8; sheet_w * sheet_h * 4]` (`:182`).
- Buffer length is checked only implicitly (`RgbaImage::from_raw(...).ok_or(UnsupportedFormat)`)
  and the error carries no index/expected/actual.
- `export_frame_sequence` (`:141`) has no frame cap either.
- `write_rgba_to_path` (`:66`) does not verify `pixels.len() == w*h*4`; the JPEG branch walks
  `chunks_exact(4)` and can hand the encoder a short buffer.

### F10 — The 16-bit composite path re-allocates canvas-sized buffers every frame

`promote_layers` (`document.rs:1065`) allocates an owned `Vec<u16>` for every 8-bit layer on
every composite, and `composite_layers_into` allocates a fresh `Vec<u16>` result before
downconverting (`:558-562`). `aurelia-render` calls this **every frame**
(`software.rs:198`). For a mixed document that is 3 canvas-sized allocations per frame.
#15's "avoid unnecessary intermediate conversions and allocations" should mean a
`composite_layers_into_srgb16(&mut out)` that reuses the caller's buffer.

### F11 — Transform source sampling is still 8-bit (noted, not in scope)

`render_transformed_patch<C: Channel>` writes natively but samples the source patch through
`sample_interpolated(&[u8], …)` (`transform.rs:809-822`, `:911`). A 16-bit layer used as the
transform *source* is downconverted to 8-bit for sampling, then re-expanded. Worth an issue of
its own after #13; not required by #14/#15.

### F12 — Navigator / viewport (D8)

The mini-map is built from a duplicate of the renderer's geometry rather than from the geometry
itself, and it is rebuilt every frame.

| # | Defect | Site |
| :-- | :--- | :--- |
| NV1 | The framing box is recomputed as `win_w_f - left_panel_w - right_panel_w - 20` instead of using `viewport_rect()` / `viewport_center_offset()` — the values `render_canvas` actually clips with. It mixes **physical pixels** (`win_w_f`) with **point-space** panel metrics, ignores `RULER_PX`, and ignores the 88 px timeline strip. | `ui.rs:1542-1568` vs `state.rs:884-900`, `main.rs:2246-2247` |
| NV2 | `thumb_h = clamp(thumb_w/aspect, 70, 170)` breaks aspect for tall/wide documents, and `painter.image` then stretches the (aspect-correct) texture into the full rect. | `ui.rs:1517-1538` |
| NV3 | The thumbnail is rebuilt **and uploaded every frame**: `navigator_thumbnail` allocates a `Vec<u8>` + `ColorImage` in the render loop with no dirtiness check. | `main.rs:2265-2274` |
| NV4 | `canvas_texture` has two writers with different content and size — the render loop (320 px thumbnail) and `AppState::recomposite_canvas` (full-resolution composite). `recomposite_canvas` has **no callers left**, so `canvas_dirty`, `mark_canvas_dirty` and `canvas_composite_buffer` are an inert dead path (called from `state.rs:665`, `:690`, `:700`; read only at `:740` inside that dead function). | `state.rs:721-745` |
| NV5 | "Fit" targets the whole window, not the viewport: `reset_view_centered` subtracts chrome plus a magic `30.0` and applies `* 0.92`. Same defect class as closed #16 / PR #17. | `state.rs:906-921`, called from `ui.rs:1590`, `:702`, `:2248` |
| NV6 | `100%` resets `pan` to zero, so zooming to actual pixels throws away the user's position. Navigator click centres on the cursor but drag does not carry the box. | `ui.rs:1592-1595`, `:1574-1584` |

---

## 4. Workstreams

### WS-1 — #20: bounded, structured, streaming animation export

**Goal:** no export can be made to allocate unbounded memory, and every rejection is a
structured error rather than an OOM or a generic `UnsupportedFormat`.

1. **`ExportLimits`** in `aurelia-io/src/export.rs`:

   ```rust
   pub struct ExportLimits {
       pub max_dimension: u32,   // 8192: matches the UI canvas clamp
       pub max_frames: usize,    // 600
       pub max_total_bytes: u64, // 1 GiB of RGBA pixel data per export
   }
   impl Default for ExportLimits { … }
   ```

   Limits are a value, not globals, and every `export_*` gains a `_with_limits` variant. The
   existing signatures keep working against `ExportLimits::default()`.

2. **Structured errors.** Extend `ExportError` with variants that carry the numbers:

   ```rust
   DimensionTooLarge { width: u32, height: u32, max: u32 }
   TooManyFrames { frames: usize, max: usize }
   AggregateTooLarge { required: u64, max: u64 }
   FrameSizeMismatch { index: usize, expected: usize, actual: usize }
   SheetTooLarge { width: u64, height: u64 }
   ```

3. **Checked arithmetic.** A private `fn expected_frame_bytes(w, h) -> Result<usize, ExportError>`
   using `checked_mul`; used by every entry point. `sheet_w`/`sheet_h`/`sheet_bytes` likewise.

4. **Streaming encoders** (this is the actual fix for the caller-side blow-up):

   ```rust
   pub fn export_animated_gif_stream<I>(frames: I, width: u32, height: u32,
       fps: u32, repeat: bool, path: impl AsRef<Path>, limits: &ExportLimits)
       -> Result<(), ExportError>
   where I: IntoIterator<Item = Vec<u8>>;

   pub fn export_sprite_sheet_stream<I>(frames: I, count: usize, …)
       -> Result<(u32, u32), ExportError>
   where I: IntoIterator<Item = Vec<u8>>;
   ```

   The GIF path holds **one** frame at a time; the sheet path sizes itself from `count` up
   front and writes frames directly into the sheet (so it also holds one frame at a time).
   Existing slice-based functions become thin wrappers over the stream versions.

5. **Atomic output.** Write to `<path>.part` and rename on success, so a rejected frame does
   not leave a truncated `.gif` behind.

6. **`write_rgba_to_path`** validates `pixels.len() == expected_frame_bytes(w, h)` and returns
   `FrameSizeMismatch` otherwise (index 0 for the flat/single-frame case).

7. **UI migration** (`ui.rs:3996-4032`): build the frame iterator lazily from
   `state.timeline.frames.iter().map(|f| f.composite_layers(w, h, inc_bg, bg_val))` and hand it
   to the stream API. Delete the `Vec<Vec<u8>>`. Report limits to the user in the dialog
   (frames × dimensions × 4 bytes) so the cap is discoverable rather than mysterious.

8. **Tests** — new `crates/aurelia-io/tests/export_limits.rs`, reusing the peak-heap
   `GlobalAlloc` harness already written for the decompression fix
   (`crates/aurelia-io/tests/decompression_limit.rs:21-59`):

   | Test | Asserts |
   | :--- | :--- |
   | `wrong_frame_length_is_rejected_with_numbers` | `FrameSizeMismatch { expected, actual }`, layer untouched, no output file |
   | `dimension_over_limit_is_rejected` | `DimensionTooLarge` |
   | `frame_count_over_limit_is_rejected` | `TooManyFrames`, before any allocation |
   | `aggregate_over_limit_is_rejected` | `AggregateTooLarge`, checked arithmetic (no overflow) |
   | `sprite_sheet_overflow_is_an_error_not_a_panic` | `SheetTooLarge` for `width = 65536, columns = 65536` — must pass in a debug build where unchecked `*` would panic |
   | `streaming_gif_peak_heap_is_one_frame` | `PEAK < 3 × frame_bytes` for a 20-frame export |
   | `zero_fps_clamps_and_single_frame_encodes` | behaviour preserved |
   | `partial_file_is_not_left_on_failure` | `.gif` absent after a mid-stream rejection |

**Acceptance:** every item in #20's "Proposed Fix" is implemented and covered above;
`cargo test --release --workspace` green; no `unwrap`/`expect` on caller-supplied data.

---

### WS-2 — Hardening: retire the "no 16-bit layer exists" invariant

**Goal:** no user action can panics on a 16-bit document. This is the prerequisite for #13 and
is reusable by #15/#14.

Ordered by blast radius, each its own commit:

1. **`Layer::storage_bytes()`** + the six `before_stroke_pixels` / `after` sites (F1). Add a
   test that paints a stroke on an `Srgb16` layer *through the same capture/commit path* and
   undoes it byte-exactly. (The core has the stroke test; this is the app-level path.)
2. **`Document::flip` / `rotate_90` / `resize_canvas` generic over `Channel`** (F3), modelled on
   `scale_canvas` + `resized_channels` (`layer.rs:206`). Tests: 8-bit digests unchanged
   (extend the existing digest test at `tests.rs:2515-2545`), plus 16-bit flip/rotate/crop
   preserving off-lattice values and format.
3. **History** (F7): `FullDocumentSnapshotCommand::cost` → `storage_len()`;
   `LayerPixelsSnapshotCommand` → `read/write_storage_bytes`; `LayerPixelsSnapshotCommand::cost`
   already uses `len()` of its own buffers (fine).
4. **`SelectionMask::from_layer_alpha`** gains a format-aware source (either
   `from_layer(layer, threshold)` computing alpha from whichever buffer, or an
   `alpha_view()` helper). Alpha *matching* stays at 8-bit for stable region boundaries, exactly
   as flood fill does.
5. **A capability guard** in `AppState`:

   ```rust
   pub fn active_layer_format(&self) -> PixelFormat;
   /// Returns false and sets a status message for operations that are 8-bit only.
   pub fn require_8bit(&mut self, op: &str) -> bool;
   ```

   Applied to the 15 filters, the filter preview, and any selection op not genericized in step 4.
   The message names the operation and states the document format, per #13's acceptance
   criteria. Per D2 the filters are *refused*, not reimplemented: the guard is the deliverable,
   and the generic filter pass follows #13.
6. **Audit closure:** `grep as_srgb8_exact` must resolve to a reviewed list (core hot paths +
   tests + the guarded UI sites), recorded in `docs/16bit-plan.md`.

---

### WS-3 — #14: native 16-bit text

**Goal:** text renders into `Srgb16` without conversion, while 8-bit output stays byte-identical.

1. **Bundle a font and the asset seam** (F5, D5). Two things, deliberately one change:
   - `assets/fonts/` gains an OFL/Bitstream-Vera–licensed font, embedded with `include_bytes!`
     so text rendering and its tests are identical on every machine. `load_font` keeps its
     system-path fallback for the UI, but the *test* path uses the bundled bytes.
   - A small `aurelia-core::assets` seam, so this is not a font-only special case:

     ```rust
     pub enum AssetSource { Bundled(&'static [u8]), File(PathBuf) }
     pub fn bundled_font() -> &'static [u8];
     pub fn read(source: &AssetSource) -> Option<Vec<u8>>;
     /// The decode boundary a future brush import reuses: encoded bytes -> RGBA stamp.
     pub fn decode_raster_stamp(bytes: &[u8]) -> Option<Stamp>;
     ```

     Nothing new decodes yet — the point is that **brush imports (ABR/GBR stamps) get the same
     two-step shape** (source → decode boundary → pixels) instead of a second ad-hoc loader
     later. The font is the first consumer and therefore the first test of the seam.

2. **A digest test that pins today's 8-bit output** for both text entry points across: single
   line, multi-line, centre/right align, letter spacing, alpha < 1, alpha-locked,
   selection-masked, and glyphs clipped at each canvas edge. It must be written **against
   unmodified code and pass before any refactor** — it is the baseline `docs/16bit-plan.md`
   ("The text decision") asks for, and the bundled font is what makes the digest portable.
3. **Extract the glyph composite** into one formula in normalised space and route it through the
   existing primitives `read_px` / `store_px` / `u8_px` (`rasterizer.rs:86-120`):

   ```rust
   fn glyph_pixel(dst: [f32; 4], src: [f32; 4], alpha: f32) -> [f32; 4];
   ```

   The current maths is already normalised (`/255.0` … `*255.0.round()`), so the only change at
   `u8` is that the final quantisation goes through `Channel::from_f32`, whose `u8` arm is
   exactly `(v.clamp(0,1) * 255).round()`. The digest from step 2 proves it.
4. **Dispatch once per text draw**, not per pixel: `match layer.buffer_mut()` at the entry point
   of both `rasterize_text_layer_config` (`:1493`) and `rasterize_text` (`:1613`). The guard at
   `:1503` is removed; the buffer clear becomes `Layer::clear()` (already format-aware,
   `layer.rs:710`).
5. **Preserve the subtle rules exactly:** `coverage <= 0.005` skip, `alpha <= 0.001` skip,
   `mask.get_value() < 8` skip, `alpha_locked && dst_alpha == 0` skip (becomes `<= 0.0` in
   working space — exact, as the brush audit established), and `blended[3] = dst[3]` restore.
6. **Tests**

   | Test | Asserts |
   | :--- | :--- |
   | `eight_bit_text_output_is_unchanged` | the step-2 digests, both entry points |
   | `text_renders_natively_on_sixteen_bit_layers` | `true`, layer still `Srgb16`, pixels changed |
   | `sixteen_bit_text_keeps_sub_lattice_alpha` | coverage 0.5 produces a non-multiple-of-257 alpha |
   | `text_reports_unsupported_on_sixteen_bit_layers` | **deleted** — replaced above |
   | `empty_text_clears_without_changing_format` | both widths |
   | `alpha_lock_and_selection_still_apply` | both widths |

7. **Docs:** update the status table in `docs/16bit-plan.md` (the "Text: refused on 16-bit"
   row and the closing "Still 8-bit only" paragraph), and the v3.3.0 release notes.

---

### WS-4 — #15: native 16-bit composite output

**Goal:** one compositor, two output widths, no duplicate blend maths.

1. **Public API** (`document.rs`), alongside the existing 8-bit pair:

   ```rust
   pub enum CompositeError { BufferTooSmall { expected: usize, actual: usize } }

   pub fn composite_layers_srgb16(&self, include_background: bool) -> Vec<u16>;
   pub fn composite_layers_into_srgb16(
       &self, out: &mut [u16], include_background: bool,
   ) -> Result<(), CompositeError>;
   ```

   The new API returns `Result` (D7); the existing 8-bit `composite_layers_into` keeps its
   current infallible signature so no caller churns.

   Both delegate to the existing `promote_layers` + `compose_u16_prepared` (drop the
   `#[allow(dead_code)]`). `composite_layers` keeps its exact current behaviour and becomes:
   *pure 8-bit document → fast/general 8-bit path (untouched); otherwise → 16-bit composite,
   then downconvert.* That is what it already does; the refactor only removes the duplicated
   downconvert loop and the temporary `Vec<u16>` where a caller supplies a buffer.
2. **Fix F2 — apply adjustment layers at 16-bit (they are *not* being removed).**
   The 16-bit path today `continue`s past every adjustment layer; the fix is to make it run
   them, not to drop them from the document or the UI:
   - Add `AdjustmentType::apply_to_rgb_working([f32; 3]) -> [f32; 3]` mirroring each arm at
     normalised scale (replace `255.0`→`1.0`, `128.0`→`0.5`, `2.55`→`0.01`, `1.28`→`0.005`).
   - Keep `apply_to_rgb` (u8) **byte-for-byte as it is** — it stays the definition of 8-bit
     output, so nothing about existing adjustment behaviour changes.
   - Test: for each of the 7 adjustment types, a swept grid of parameters × RGB triples asserts
     `to_u8(apply_working(v/255)) == apply_to_rgb(v)`. If any arm disagrees, that is a real
     rounding difference to resolve deliberately, not to paper over.
   - The 16-bit compositor then runs the adjustment pass instead of `continue`-ing, including
     the clipping-base alpha and mask/opacity inputs it already computes.
3. **Allocation policy** (F10): the 16-bit path takes the caller's buffer; `promote_layers` is
   documented as mixed-document-only cost; note that a document created by the #13 UI is
   single-format, so the promotion cost does not apply to it.
4. **Consumers** (the issue's "identify the requirements"):
   - *Display:* egui 0.28 `ColorImage` is 8-bit. The viewport keeps showing a downconverted
     presentation of a 16-bit document; that is a display limitation, not data loss, and should
     be stated in the UI. No 16-bit texture path is proposed here.
   - *Export:* PNG can carry RGBA16 (`image::ExtendedColorType::Rgba16`), so image export gains
     a native path once this API exists. Proposed as a follow-up issue, sequenced with #13.
   - *Save:* already native (AUR v3).
5. **Tests**

   | Test | Asserts |
   | :--- | :--- |
   | `native_sixteen_bit_output_keeps_sub_lattice_values` | public API returns values that are not multiples of 257 |
   | `mixed_format_output_downconverts_to_the_eight_bit_result` | `srgb16_to_srgb8(native)` == `composite_layers()` for lattice fixtures |
   | `adjustment_layers_apply_in_sixteen_bit_documents` | F2 regression: adjusted output ≠ unadjusted, and 8-bit oracle unchanged |
   | `composite_into_srgb16_rejects_short_buffers` | `CompositeError::BufferTooSmall { expected, actual }`, buffer untouched |
   | `native_output_is_deterministic_across_runs` | mirror the existing parallel-vs-serial test, including a fixture asserted to take the parallel path |
   | existing 702-case oracle | unchanged |

6. **Benchmark:** extend `crates/aurelia-render/examples/bench_composite.rs` with an `srgb16`
   mode so both output widths are measured with the same harness and fixture set
   (`-p aurelia-render --example bench_composite`).

---

### WS-5 — #13: 16-bit documents in the UI

**Preconditions:** WS-2 and WS-3 merged. Filters are **blocked, not implemented** (D2): the
destination-write plumbing and the 8-bit capability guard they need land here as part of #13,
so the generic filter work starts immediately after this ships rather than inside it.

1. **Document format** (F8, D1 — decided: an explicit field):
   - `Document::pixel_format: PixelFormat` with `#[serde(default)]`; `Document::new` stays
     `Srgb8`; add `Document::new_with_format(w, h, format)`.
   - `add_layer`, `add_text_layer`, group/merge/flatten paths inherit `self.pixel_format`.
   - `save_project_to_writer` writes `doc.pixel_format` (replacing the derived value);
     `load_project_from_reader` stores the tag. Old files are unaffected: the writer only ever
     emitted `1` when a layer was 16-bit, so read-back reproduces the same document.
   - Document the rule in `docs/PROJECT_FORMAT.md`: the document tag is authoritative for new
     layers; per-layer tags still permit mixed files.
   - **No format bump.** The v3 document tag already exists, so this adds no new file version
     and does not touch v1/v2 reading. Per D3, compatibility is forward-only — see §7.
2. **UI surface:**
   - New Canvas dialog (`ui.rs:2296-2384`): a "Color depth" row — `RGBA 8-bit` / `RGBA 16-bit` —
     next to the existing Background row. Replace `PendingFileAction::NewCanvas(u32, u32, u8)`
     (`state.rs:136`) with a small struct (`NewCanvasSpec { width, height, bg_mode, format }`) so
     the tuple does not grow again.
   - `main.rs:2438-2460`: construct with `Document::new_with_format`, and mention the format in
     the status line.
   - Footer (`ui.rs:964`, the `W × H px` metric): append `· RGBA16` when the document is 16-bit.
   - Layers panel: a per-layer format badge, so a mixed file is legible.
   - Open: `api` already loads 16-bit AUR v3; add the format to the "Loaded …" status.
   - Image import (open image → layer): decode as 8-bit, then promote `v*257` into the document
     format, so a 16-bit document stays single-format (D6). Mixed documents stay legal for
     files opened from disk; the editor simply does not create them.
   - Save: already preserves; add a round-trip test.
3. **Guarded operations** (WS-2 step 5): filters refuse with
   `"<op> is 8-bit only; this document is 16-bit sRGB"` and mutate nothing. Text works (WS-3).
   Flip/rotate/crop work (WS-2 step 2). Painting works (WS-2 step 1).
4. **Tests** (the issue asks for UI tests; put the logic in `AppState` so it is testable without
   egui rendering — `aurelia-ui` already has unit tests, e.g. `state::discord_presence_defaults`):

   | Test | Asserts |
   | :--- | :--- |
   | `new_document_creates_the_requested_format` | base layer is `Srgb16`, doc reports 16-bit |
   | `new_layers_inherit_the_document_format` | `add_layer` / `add_text_layer` stay 16-bit |
   | `save_and_reopen_preserves_native_sixteen_bit_data` | off-lattice bytes survive the round trip |
   | `deleting_the_last_sixteen_bit_layer_does_not_change_the_document_format` | F8 regression |
   | `eight_bit_only_operations_are_refused_with_a_message` | filters: status set, pixels unchanged |
   | `image_import_promotes_to_the_document_format` | an 8-bit source becomes `Srgb16` by `v*257` (D6) |
   | `sixteen_bit_document_reports_its_format` | footer/badge source values |
   | existing 8-bit state tests | unchanged |

5. **Docs/release:** ROADMAP "Next (ordered)", `docs/PROJECT_FORMAT.md`,
   `docs/16bit-plan.md` limitations section, README feature bullets, v3.4.0 release notes.

---

### WS-6 — D8: Navigator / viewport

**Goal:** the mini-map frames what the renderer is actually showing, updates only when the
document changes, and the fit/zoom actions target the real viewport. No new dependencies.

Independent of WS-1 … WS-5; it can be taken first, last, or in parallel.

1. **One geometry source** (NV1). Derive the framing box from `AppState::viewport_rect()` and
   `viewport_center_offset()` — the same values `render_canvas` clips with
   (`main.rs:2246-2247`) — instead of the duplicate `win_w_f - left - right - 20` computation at
   `ui.rs:1542-1568`. This removes the physical-pixel/point-space mix, the ignored `RULER_PX`,
   and the ignored 88 px timeline strip in one change.
2. **Aspect-correct thumbnail** (NV2). Letterbox the texture inside the allocated rect and
   compute the framing box in that inner rect's space, so tall and wide documents are neither
   squashed nor offset.
3. **Dirty-driven thumbnail** (NV3). Gate `navigator_thumbnail` on a document revision counter
   bumped by edits/resizes; pan and zoom do not invalidate it. Target: zero rebuilds and zero
   texture uploads on an idle frame.
4. **One owner for the texture** (NV4). Delete `recomposite_canvas`, `canvas_composite_buffer`
   and the inert `canvas_dirty` flag (its only reader is that dead function), or repurpose the
   revision counter from step 3 in its place. Rename the live texture to `navigator_texture` so
   its single purpose is obvious.
5. **Fit targets the viewport** (NV5). `reset_view_centered` computes from `viewport_rect()`
   without the `30.0` fudge or the `0.92` factor; the margin, if kept, becomes a named constant.
   This is the defect class behind #16 / PR #17, so it is worth a regression test rather than a
   visual check.
6. **Interaction** (NV6). Keep click-to-pan, add drag-carry so grabbing the viewport box does not
   jump the cursor to its centre, and make `100%` zoom about the cursor instead of resetting
   `pan` to zero.
7. **Tests** (state-level, no egui rendering):

   | Test | Asserts |
   | :--- | :--- |
   | `navigator_framing_box_matches_the_renderer_viewport` | panel widths × DPI (`ui_ppp`) × timeline on/off |
   | `thumbnail_letterboxes_without_distortion` | 1080×1920, 1920×1080, 300×100, 100×3000 |
   | `fit_to_viewport_is_exact` | known viewport → expected zoom, no fudge factor |
   | `idle_frame_rebuilds_nothing` | revision counter unchanged ⇒ no thumbnail rebuild |
   | `drag_carries_the_viewport_box` | pan delta equals drag delta ÷ thumbnail scale |
   | `zoom_at_cursor_keeps_the_cursor_pixel_fixed` | point under cursor invariant |

---

## 5. Sequencing

```
v3.2.1  WS-1 (#20)                        independent, no API break
        │
v3.3.0  WS-2 step 1 (stroke capture)      unblocks manual 16-bit testing
        WS-2 steps 2-6 (hardening)
        WS-3 (#14, text)   ─┐  parallel-safe with each other and with WS-6
        WS-4 (#15, output) ─┤
        WS-6 (D8, navigator/viewport) ─┘
        │
v3.4.0  WS-5 (#13, UI)                    only after the panic surface is zero
```

Rationale for the order:

- **#20 first** because it is the only bug, it is self-contained, and it touches no shared
  invariant. It can ship alone as a patch release.
- **WS-2 before #13** because #13 is the change that makes 108 panic sites reachable. Doing it
  first would turn every harden item into a crash report.
- **#14, #15 and the navigator work are independent** of each other and of WS-2, so they can
  proceed in parallel once WS-2 step 1 lands. WS-6 is the only workstream that can be taken at
  any point, including before #20, if a quick visible win is wanted.
- **#13 last** because it is a product decision layered on top of the plumbing, and its
  acceptance criteria ("clearly communicate unsupported operations", "no silent conversion")
  depend on the capability layer built in WS-2.

Merge-order note: WS-6 rewrites the navigator block in `ui.rs:1487-1614`, and #13 edits the
footer metric at `ui.rs:964` and the New Canvas dialog at `ui.rs:2296`. They do not overlap, but
WS-6 should land first so #13 rebases on the smaller diff.

Suggested release notes framing: v3.2.1 "export hardening"; v3.3.0 "16-bit: text and native
output, plus a navigator that frames what you are actually seeing"; v3.4.0 "16-bit documents in
the editor".

---

## 6. Quality gates (apply to every workstream)

1. `cargo test --release --workspace` — 112 existing tests plus new ones, zero failures.
2. The 702-combination compositor oracle and the 234 clipping corners, unchanged.
3. The existing digest tests (`tests.rs:2515-2545`) unchanged for 8-bit paths.
4. `cargo clippy --workspace` — no new lints versus `3ecf8cd`.
5. `cargo run --release -p aurelia-render --example bench_composite` — no >5 % regression on the
   8-bit path (WS-1 and WS-2 touch no compositor code, so this is a guard, not an expectation).
6. A manual smoke script (there is precedent: `scripts/smoke_delete_layer.ps1`) covering create
   16-bit → paint → text → adjust → flip → save → reopen → export.

---

## 7. Decisions

All eight are settled. Recorded with their reasoning so the implementation does not re-litigate
them. "Delegated" means the maintainer left the choice to the implementer; the choice is stated
and is not to be reopened casually.

| # | Decision | By |
| :-- | :--- | :--- |
| D1 | `Document` gets an explicit `pixel_format` field (serde-defaulted), and the AUR v3 document tag becomes authoritative instead of derived. Deriving loses the format when the last 16-bit layer is deleted (F8). | delegated |
| D2 | Filters stay **out of #13** and are refused at runtime with a clear message. The plumbing they depend on — format-aware destination writes plus the 8-bit capability guard — lands *with* #13, so generic filters start immediately after it ships. | maintainer |
| D3 | `ExportLimits::default()` = 8192 px max dimension, 600 frames, 1 GiB aggregate; configurable through the API, not exposed in the UI in v1. Compatibility is forward-only (below). | maintainer |
| D4 | Adjustment layers are **generalized to working precision, never dropped**. Nothing is removed from the document model or the UI. | maintainer (clarified) |
| D5 | Bundle an OFL/Bitstream-Vera font via `include_bytes!`, and put it behind a small asset seam so brush imports reuse the same source → decode boundary shape. | maintainer |
| D6 | Image import promotes `v*257` into the document format, keeping editor-created documents single-format; mixed files remain legal on disk. | delegated |
| D7 | The new 16-bit API returns `Result<(), CompositeError>`; existing 8-bit signatures are untouched. | delegated |
| D8 | Navigator/viewport improvements added as WS-6, independent of the 16-bit work. | maintainer |

### D4 — adjustments are not being removed

To be explicit, because the earlier wording could be read the wrong way:

- **Today, 8-bit documents**: adjustment layers work. `composite_layers_into` runs them at
  `document.rs:588-635`.
- **Today, any document containing a 16-bit layer**: the 16-bit path `continue`s past every
  adjustment layer (`document.rs:1109-1115`). That is an existing silent omission, not a
  proposal. No user hits it today only because the UI cannot create a 16-bit document.
- **The fix** is to run the adjustment pass at working precision
  (`AdjustmentType::apply_to_rgb_working`), keeping the 8-bit `apply_to_rgb` untouched so 8-bit
  output does not move. Adjustment layers keep working in both formats; the 8-bit definition
  stays the reference.

The "defer" alternative is what would have removed adjustments from 16-bit documents. It is off
the table, and it was never a proposal to change the document model.

### D3 — compatibility policy

Forward-only, per the maintainer: "there's only one way direction."

- **v1/v2 reading stays.** It is implemented, tested, and costs almost nothing to keep; nothing
  in this plan requires dropping it, so nothing is dropped.
- **No effort goes into making new files readable by older builds.** A 16-bit AUR v3 document
  cannot be opened by Aurelia < v3.0.0, and that is accepted rather than worked around.
- **No legacy writers.** `write_project_version` stays test-only; there will be no "save as v2".
- **No format bump is needed for this work.** The v3 document tag already exists, so #13 adds a
  field and uses the tag that is already written and already read.

### D5 — why the font lives behind an asset seam

A test-only font under `tests/fixtures/` would solve #14's determinism problem and nothing else.
The seam exists because brush imports (Photoshop `.abr`, GIMP `.gbr` stamps) need the same two
steps — obtain bytes from a bundled or user source, then decode through one boundary into RGBA
pixels. Establishing that shape now means the brush import later is a decoder, not a second
loader with its own file handling, error type and test strategy. Only the seam and the font
ship in v3.3.0; no brush format is parsed in this plan.

---

## 8. Follow-ups this plan deliberately does not absorb

- **Generic 16-bit filters** (15 functions + modals) — new issue. Blocked by nothing once #13
  ships: that is where the destination-write plumbing and the capability guard land (D2). This
  is the first thing to schedule after v3.4.0.
- **Brush imports** (`.abr` / `.gbr` stamps) — new issue. The `assets` seam from D5/WS-3 is the
  intended entry point; no brush format is parsed in this plan.
- 16-bit PNG export via the new composite output — new issue, after #15.
- Native 16-bit transform *source* sampling (F11) — new issue.
- 16-bit mask storage — explicitly out of scope in `docs/16bit-plan.md`.
- Display of 16-bit data on an 8-bit egui texture (no plan needed; document the limitation).
- Navigator: onion-skin overlay in the mini-map. Adjacent to WS-6 but a feature, not a defect;
  left out so WS-6 stays a correctness and performance change.

---

## 9. What was verified while writing this

- Remote `main` is `3ecf8cd`; the local checkout was 7 docs-only commits behind and has been
  fast-forwarded. No source changed between `76cc4b6` and `3ecf8cd`.
- `cargo test --workspace`: 112 passed, 1 ignored (the `--ignored` benchmark at
  `tests.rs:1689`), 6 targets.
- The 108 `as_srgb8_exact` count excludes `crates/aurelia-core/src/tests.rs` and the
  `examples/` targets (151 including them).
- Navigator findings NV1–NV6 were read from the code, not observed at runtime: two writers for
  `canvas_texture` (`main.rs:2266`, `state.rs:731`), `recomposite_canvas` with zero callers,
  and `canvas_dirty` written at `state.rs:665`/`:690`/`:700` and read only at `:740` inside that
  dead function. Runtime confirmation happens in WS-6 step 3, where the idle-frame counter makes
  it measurable.
- No code was modified. The working tree gain is this document; the unrelated untracked
  `test_release.py` that was already present is untouched.
