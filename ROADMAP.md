# Aurelia · Roadmap

Tracks what shipped in `v2.0.0`, what is deliberately next, and what is
explicitly out of scope for the Windows-native build.

Legend: ✅ shipped · 🚧 in progress · ⏳ planned · 💤 deferred (needs design work)

---

## Shipped in v2.1.0

### Packaging & release engineering

| Item | Status | Notes |
| :--- | :--- | :--- |
| Morrow-style staged pipeline | ✅ | `scripts/pipeline.py`: preflight → version → build → test → package → verify → publish → ship, resumable with `--from`/`--skip` |
| Per-tag release records | ✅ | `releases/<tag>.json` (assets, hashes, tests, republish count) + `release-notes/<tag>.md` |
| Git-only publishing | ✅ | Commit pipeline-owned files, annotated tag, push, verify the remote tag object |
| REST artifact upload | ✅ | `scripts/release_page.py` (no forge CLI), refuses to upload bytes that disagree with `SHA256SUMS.txt` |
| Inno Setup installer | ✅ | Per-user `%LOCALAPPDATA%\Programs\Aurelia`, optional `.aur`/`.hcv` associations, smart uninstall |
| Portable zip + manifest | ✅ | `Aurelia-v<ver>-win64.zip`, `SHA256SUMS.txt` |
| VPack retired | ✅ | No `.vpack`, no signing stage, no VirusTotal upload (see below) |


### Rebrand: Hollow Canvas → Aurelia

| Item | Status | Notes |
| :--- | :--- | :--- |
| Product, docs and code renamed | ✅ | Crates `aurelia-{core,render,io,ui,app}`, binary `aurelia.exe`, `Aurelia` window class |
| New project format | ✅ | `.aur` with `AUR\x01` signature; legacy `.hcv` v2–v5 still readable |
| Explorer associations | ✅ | `.aur` + `.hcv`, `Aurelia.Project` ProgID, pre-2.0 keys auto-removed |
| Settings relocation | ✅ | `%LOCALAPPDATA%\Aurelia` (preferences + autosave) |
| New branding graphics | ✅ | Gold "A" prism icon (PNG/ICO regenerated), new banner and icon SVG |
| Aurelia Dawn / Dusk / Ember themes | ✅ | Warm-ink studio retheme across panels, rulers, guides and graders |

### Save safety (was: silent data loss)

| Item | Status | Notes |
| :--- | :--- | :--- |
| Unsaved-changes guard on window close | ✅ | `WM_CLOSE` → Save / Discard / Cancel |
| Project path tracking + quick save | ✅ | `Ctrl+S` writes straight to the known path |
| Save As | ✅ | `Ctrl+Shift+S` |
| Recent projects menu | ✅ | Persisted in `%LOCALAPPDATA%\Aurelia\preferences.txt` |
| Atomic project writes | ✅ | `art.aur.tmp` → rename over target |
| Autosave + crash recovery | ✅ | Snapshot every 120 s of dirty edits; restore prompt on launch |
| Dirty marker in the title bar | ✅ | `Aurelia · art.aur *` |

### Input

| Item | Status | Notes |
| :--- | :--- | :--- |
| Stylus pressure (`WM_POINTER` + `GetPointerPenInfo`) | ✅ | Pressure overrides velocity dynamics; mouse path untouched |
| Pen tilt sampling | ✅ | Captured into `pen_tilt`, reserved for tilt-aware dynamics |
| Velocity dynamics / stroke stabilization | ✅ | Pre-existing |

### Compositing & selection

| Item | Status | Notes |
| :--- | :--- | :--- |
| Selection intersect | ✅ | `SelectionMask::intersect` |
| Select from layer alpha | ✅ | `SelectionMask::from_layer_alpha` |
| `∩ Intersect with Layer Alpha` | ✅ | Select menu |

### Colour & filters

| Item | Status | Notes |
| :--- | :--- | :--- |
| Levels (black/white/gamma) | ✅ | Live preview modal |
| Colorize (hue/saturation over luminance) | ✅ | Live preview modal |
| Pixelate | ✅ | Configurable block size |
| Motion blur | ✅ | Distance + angle |
| JPEG export quality | ✅ | 1–100 slider in File → Export Image |
| PNG / JPEG / WebP export | ✅ | JPEG flattens alpha over white |

### Interop

| Item | Status | Notes |
| :--- | :--- | :--- |
| TIFF / TGA / ICO decoding | ✅ | Now matches the extensions registered in the Explorer context menu |
| Sprite-sheet export | ✅ | Grid columns configurable |
| Animated GIF + PNG sequence | ✅ | Pre-existing |
| AUR v1 project format | ✅ | Deflate-compressed, reads legacy HCV v2–v5 |

### Platform & scale

| Item | Status | Notes |
| :--- | :--- | :--- |
| Per-monitor V2 DPI awareness | ✅ | `SetProcessDpiAwarenessContext` + `egui` pixels-per-point |
| Undo memory budget | ✅ | 512 MiB default; oldest steps evicted, newest kept |
| Theme persistence | ✅ | Saved with preferences |
| CI | ✅ | Local only — `make check` / `python scripts/pipeline.py <tag> --no-publish` (no GitHub Actions by design) |

---

## Shipped in v2.5.0

### Layer masks

- [x] `LayerMask { width, height, pixels, enabled }` on every layer, composited into
  the alpha of the fast path, the general (offset/blend) path and adjustment layers.
- [x] Create from the current selection, invert, enable/disable, remove; masks copy
  with the layer and never modify pixels.
- [x] **AUR v2** project format with a per-layer mask section; AUR v1 files still load
  and simply have no masks.
- [ ] Painting *into* a mask with the brush (next: route freehand strokes at a mask
  target, the same way strokes currently route at layer pixels).

### Stylus tilt

- [x] `pen_tilt` (sampled since 2.0, previously unused) folds into freehand stroke width
  via `brush::tilt_size_scale`, exposed as the **Tilt widens** slider. Off by default,
  freehand tools only, never mutates the stored brush size.

### Performance: the general (offset + blend) path

**Closed in v2.6.0.** All four release gates met. Reproduce with
`cargo run --release -p aurelia-render --example bench_composite`, which prints the
machine and compiler with the numbers; the table below is from an Intel Alder Lake
(12 logical cores), rustc 1.93.1, release profile, 1920x1080, 4 layers, 20 iterations x 3
interleaved rounds, best round reported.

| Case | v2.5.0 | v2.6.0 |
| :--- | ---: | ---: |
| **General path, opaque** | 25.7 MPix/s | **83.6 MPix/s** (3.3x) |
| General, 50% coverage | - | 128.1 MPix/s |
| General, 10% coverage | - | 209.5 MPix/s |
| General, 10% + layer mask | - | 188.4 MPix/s |
| General, clipping layer | - | 60.4 MPix/s |
| Plain stack | 113.1 MPix/s | 160.2 MPix/s |
| With layer mask | 92.6 MPix/s | 140.7 MPix/s |

All 13 modes on their own, opaque general path: **61.0 (Soft Light) to 95.0 (Darken)
MPix/s** across two runs - no single mode is hiding behind Normal.

Run-to-run variation on this laptop is +-10%: a confirmation run of the same binary gave
91.4 MPix/s for opaque general, 167.6 plain and 254.5 for the sparse clipping case. The
gates are met in both, which is why the benchmark reports best-of-interleaved-rounds and
prints the machine.

#### Improved again: the generic computation model (step 2 of the pixel-format plan)

The general path now runs one generic loop instantiated for `u8`, with the blend maths
factored into a single working-precision implementation (`BlendMode::composite_working`)
that `composite_pixel` and the per-mode `compose_*` functions both call. The 8-bit integer
fast path is untouched, and the 702-combination oracle stayed byte-identical throughout.

| Workload | v2.6.1 | now | change |
| :--- | ---: | ---: | ---: |
| General, opaque | 79.8 | **91.3** | +14% |
| General, 50% coverage | 95.9 | **122.1** | +27% |
| General, 10% coverage | 157.3 | **191.9** | +22% |
| General, 10% + mask | 154.9 | **177.7** | +15% |
| General, clipping | 56.6 | **68.3** | +21% |
| General, clipping 10% | 137.1 | **215.1** | +57% |
| All 13 modes | 64.3 - 79.9 | **80.9 - 94.1** | every mode up |

Plain and masked paths are unchanged in code and within noise in measurement.

#### Release gates

| Gate | Status |
| :--- | :--- |
| T1 30 MPix/s | met (36.1 before parallelism) |
| T2 40 MPix/s | met |
| T3 50 MPix/s | met |
| T4 60 MPix/s, opaque general compositing | **met: 83.6** |
| No correctness regressions | met: 54 tests, oracle byte-identical over 936 combinations |
| No measurable regression in ordinary compositing | met: plain and masked paths untouched and no slower |
| Benchmarks for sparse and opaque workloads | met: 10%, 50%, 100% coverage rows |
| Machine, thread count, compiler and config recorded | met: printed by the benchmark |

#### What the time was actually going on

None of it was the blend maths:

1. **Out-of-bounds work.** Every canvas pixel was visited even for an offset layer, each
   through `get_pixel` (four bounds checks and an `Option`). The valid span is now computed
   per row and indexed directly.
2. **Transparent pixels.** `composite_pixel` returns the destination for a transparent
   source, but only after the float path. Skipping those first is exact and is the biggest
   win on real artwork: 10% coverage now runs 2.5x the opaque rate.
3. **Per-pixel blend dispatch.** Resolved once per layer; each mode runs a monomorphised
   loop (`compose_normal`, ...) while the maths stays single-sourced in `composite_pixel`.
4. **Clip base lookups.** A clipped layer resolved its base alpha through `get_pixel` per
   pixel; it is now a contiguous span per row.
5. **One core.** Rows of a layer are independent, so they are composed in parallel: fixed
   row ownership per thread, join before the next layer, cap of 8 threads, and a 128x128
   floor so small canvases stay single-threaded.

#### Methodology, because it matters

- **Interleaved rounds, best of.** Measuring cases one after another on a throttling laptop
  made the first case look fast and the last slow: Normal fell from 31.9 to 7.0 MPix/s
  within a single run of identical work. Cases are now visited round-robin and the best
  round is kept.
- **One general-path layer per mode.** The first per-mode bench set the mode on all four
  layers, which measured the general path four times over and reported every mode at
  ~8 MPix/s ? a harness bug, not a finding.
- **The machine is printed.** A throughput number without CPU, cores and compiler is not
  comparable with anything.

#### Tried and rejected (measured, not assumed)

- **256-entry `n/255.0` lookup table.** 29.7 vs 36.6 MPix/s ? a 19% regression: a `static`
  table can alias the output buffer, so entries are reloaded after every store. Any retry
  needs a stack-local copy, which is now possible because the loop is inlined per mode.
- **Full-row clip pre-pass.** Resolving the base alpha for the whole row before the pixel
  loop touched every pixel even where the layer is empty and made a clipped layer twice as
  slow as an opaque one. Span indexing needs no pre-pass and no allocation.

#### Still open (not blocking)

1. **Persistent thread pool.** A layer spawns its scoped threads on each call (~50 us each).
   Measured overhead is a few percent; a pool would remove it.
2. **Parallelise the fast path.** The plain stack is still single-threaded at ~160 MPix/s;
   the same row split applies.
3. **Stack-local channel table** ? the rejected LUT, retried with the aliasing fixed.

## Next (ordered)

1. **Mask painting** ? target a mask with the brush, keeping it non-destructive.
2. **General-path compositing speed** ? 25.7 MPix/s is ~4x slower than the fast path;
   the per-pixel closure and the offset branch are the cost. Target 60 MPix/s.
3. **Layer styles** ? stroke, glow, drop shadow as composite-time effects.
4. **Selection combine modes** ? Replace / Union / Subtract / Intersect for new selections.
5. **Preferences UI** ? autosave interval, undo budget, JPEG quality, theme.
6. **History panel** ? visible undo stack with memory readout.
7. **Multi-document tabs** ? see the design below.

## Designed, not built

### Colour management / higher bit depth

**Step 1 is done (v2.6.1, uncommitted at the time of writing).** `Layer::pixels` is
private and every caller goes through `as_srgb8()` / `as_srgb8_mut()`, with
`pixels_vec_mut()` for length changes and `set_pixels()` for replacement. 170 call sites
across core, io, ui, app and render were migrated, the fast path hoists the slice once
per layer, and all 78 tests - including the 702-combination byte-identical oracle - pass
unchanged. Nothing about the representation is promised to callers any more, which is the
whole point: the next step is a type change inside `layer.rs`, not a repo-wide rewrite.

Measured inventory of what a 16-bit move still touches, so the decision is made with
numbers rather than optimism:

| Surface | Size |
| :--- | :--- |
| 8-bit pixel maths (`as u8`, `/255.0`, `*255`) | 211 lines |
| Filters, all `&mut [u8]`, six round-trip through u8 HSL | 20 filters, 793 lines |
| Rasterizer (brushes, gradients, magic wand) | 1285 lines |
| Transform / resampling | 1012 lines |
| Software renderer texture upload | 1154 lines |
| Export (`ImageBuffer<Rgba<u8>>`) | PNG/TIFF can be 16-bit; JPEG/WebP/GIF/TGA/BMP cannot |
| Project format | needs AUR v3 plus v2 migration |

**The cost that decides it:** the compositor is memory-bandwidth bound - 8 threads on 12
logical cores bought only 2.3x, not 8x. Doubling bytes per pixel therefore puts opaque
general compositing at roughly 45-55 MPix/s, below the T4=60 gate shipped in v2.6.0, and
the u8 integer fast path loses half its SIMD lanes. Precision would be bought with the
throughput just earned.

**The migration trick that keeps the oracle:** storing an 8-bit value as `v * 257` puts it
exactly on the 16-bit lattice, and because `65535 = 255 x 257`, `(257v)/65535` and `v/255`
are the same real value - so the f32 blend maths is bit-identical, and `(v16 + 128) / 257`
recovers the original byte exactly. The existing oracle can therefore run in 16-bit mode
and be compared byte-for-byte after downconversion. Only the integer fast path needs a
tolerance of +-1 LSB or a rewrite. Nothing gets weakened to accommodate the change.

The staged plan continues:

1. Introduce `PixelFormat` on `Document` (`Srgb8`, `Linear16`) and a `LayerBuffer`
   abstraction that only the compositor dereferences, keeping `Vec<u8>` behind it.
2. Move filters and blend modes to operate on `LayerBuffer`, converting at the edges;
   the fast path stays 8-bit when the document is `Srgb8`.
3. Add an ICC profile reference to the project header (v3) and a working-space
   conversion at export; keep sRGB as the only output until colour proofing exists.
4. 16-bit only after the above is green in tests - the risk is silent banding in blend
   modes, not the storage.

#### Decision order for the abstraction (GitHub issue #11)

Each row is a gate. Answering a later one while an earlier one is open produces a shape
chosen by accident rather than by design:

| # | Decision | Required outcome |
| :--- | :--- | :--- |
| 1 | Buffer responsibilities | Explicit interface and ownership rules |
| 2 | Format access | Defined immutable and mutable access patterns |
| 3 | Computation | Monomorphised format-specific paths |
| 4 | Conversion | Documented boundaries and scratch ownership |
| 5 | Persistence | AUR v3 format and migration rules |
| 6 | Performance | Prototype meets agreed throughput requirements |
| 7 | Correctness | Existing oracle remains byte-identical after downconversion |

Ordering rationale: conversion boundaries (4) cannot be judged without ownership rules (1);
persistence (5) follows from the access patterns (2); the performance gate (6) needs a
prototype that only exists once computation (3) is settled. Correctness (7) is last only
because it is the final proof - it constrains every earlier row.

#### Draft recommendation: one working format per document

The first 16-bit implementation should use a **document-wide working format**, not
per-layer formats. Per-layer depths cost a format tag in the UI for every row, a
definition for every pair of depths inside one file, a conversion at every operation
boundary instead of at two edges, and filters that silently behave differently depending
on the layer they run on.

The realistic pressure for mixed formats is **import**: opening a 16-bit PNG into an 8-bit
document. That is better answered explicitly than by supporting the matrix - either
**promote the document** to the higher depth on import, or **convert on import** and tell
the person the source depth was reduced. If a concrete use case later demands per-layer
depth, the Phase 1 accessors make it an additive change rather than a rewrite.

No implementation starts until the proposal in #11 is written and reviewed. Phase 1 was
valuable precisely because it removed the obstacle without guessing at the answer.

**Proposal:** `docs/pixel-format-proposal.md` answers gates 1-5 and *specifies* 6-7 as
acceptance gates (measured, not asserted). Three concepts are kept separate there and must
never be merged into one setting:

| Concept | Responsibility |
| :--- | :--- |
| Storage format | How layer pixels are represented |
| Working precision | Precision used during filters and compositing |
| Working colour space | How colour values are interpreted and combined |

16-bit sRGB storage does not provide linear-light compositing; f32 filter intermediates do
not require f32 storage; and a colour-space change alters output *by definition*, so it
cannot ride along with a storage change - that would invalidate the oracle that makes the
storage change safe.

Two findings from the proposal that constrain the rest of the work: scratch buffers must be
**per row band, not per document** (rows compose in parallel; one shared buffer would
serialise or lock the hot loop), and import policy is a **product decision**, not an
implementation detail - a 16-bit image into an 8-bit document either promotes the document
or converts with a visible notice.

A prototype measuring four workloads - opaque, sparse, masked, clipped - validates the
computation model before the architecture is approved, with the kill criterion stated in
advance: if it cannot hold T4 on opaque, the storage change does not ship in that form.

**Prototype result (gate 6, measured).** `crates/aurelia-render/examples/proto_pixel_format.rs`

**16-bit plan:** `docs/16bit-plan.md` turns that into three gates - fidelity and
correctness, performance and memory, then production architecture - and settles the naming
question: the variant is `Srgb16`, not `Linear16`. Linear light is a colour-space change
that alters every blend result by definition and therefore cannot be validated by the
byte-identical oracle; the fidelity problem that motivates 16-bit work is banding, which
is storage precision. `Linear16` stays reserved for a future linear-light working space,
which the f32 generic model from v2.7.0 makes reachable without another storage change.
implements the model in isolation and asserts, before measuring, that its `u8` instantiation
reproduces the production compositor byte for byte and that a `v * 257` 16-bit document
downconverts to the same bytes - both hold. Throughput: **T4 fails at 43.5 MPix/s against 60,
and the storage width is not the cause.** The bench fixture lets three of four layers use the
8-bit fast path, which a 16-bit document cannot have under the decision to keep the fast path
8-bit; the all-general 8-bit baseline is 24.6 MPix/s, the same ceiling or lower. Width costs
+9% where the loop is compute-bound (opaque) and **-39% where it is bandwidth-bound (sparse)**,
which is the realistic artwork case. The generic model is faster than today's general path on
every all-general workload, so migrating to it pays for itself independent of 16-bit.

Two things the prototype changed: the first implementation must quantise **per layer** (that
is what keeps the oracle byte-identical; a whole-pass f32 buffer is a separate proposal with
its own re-baselining), and the 16-bit performance target must be defined against the general
path rather than the mixed fixture - or a 16-bit fast path must be built.

### Multi-document

`AppState` owns exactly one `Document`, and ~40 places reach into `state.document`
directly. The plan:

1. Introduce `DocumentId` + `Vec<Document>` inside `AppState` with `active` accessor;
   replace `state.document` call sites mechanically (compiler-guided).
2. Per-document: history stack, file path, dirty flag, pan/zoom, selection, timeline.
   These already live in `AppState`, so they move into a `DocumentSession` struct.
3. UI: a tab strip above the canvas, drag-reorder, `Ctrl+Tab` cycling, close prompting
   for dirty documents (the existing quit guard already knows how to ask).
4. Autosave keys by document id; the recovery dialog lists every snapshot.

### Deferred (unchanged)

- ?? **Cross-platform port** ? the host is Win32 + GDI; `eframe`/`winit`/`wgpu` is a rewrite.
- ?? **PSD import/export, animated WebP / MP4** ? needs third-party codecs.
- ?? **`render_ui` split** ? mechanical but touch-heavy; best done behind the new tests.
- ?? **ED25519 signing** ? retired in 2.1.0; plan is a `sign` stage between verify and publish.
- ?? **Authenticode certificate** ? still the highest-value integrity upgrade; the
   unsigned installer keeps 2-4 generic heuristic detections.
