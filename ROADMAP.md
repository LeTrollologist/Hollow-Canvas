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

### Performance baseline

`cargo run --release -p aurelia-render --example bench_composite` (no dependencies):

| Shape of work | Measured | target |
| :--- | ---: | ---: |
| Plain 4-layer stack | 113.1 MPix/s | >= 40 |
| With a layer mask | 92.6 MPix/s | >= 30 |
| Offset + general blend path | 25.7 MPix/s | >= 60 (open) |

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

The 8-bit assumption is everywhere: `Layer { pixels: Vec<u8> }`, the rasterizer,
filters, blend modes and the project format. The staged plan that avoids a rewrite:

1. Introduce `PixelFormat` on `Document` (`Srgb8`, `Linear16`) and a `LayerBuffer`
   abstraction that only the compositor dereferences, keeping `Vec<u8>` behind it.
2. Move filters and blend modes to operate on `LayerBuffer`, converting at the edges;
   the fast path stays 8-bit when the document is `Srgb8`.
3. Add an ICC profile reference to the project header (v3) and a working-space
   conversion at export; keep sRGB as the only output until colour proofing exists.
4. 16-bit only after the above is green in tests ? the risk is silent banding in blend
   modes, not the storage.

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
