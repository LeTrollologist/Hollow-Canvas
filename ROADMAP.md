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

## Next (ordered)

1. ⏳ **Layer raster masks** — per-layer `Option<Vec<u8>>` mask, paint-mode toggle, AUR v2 bump with AUR v1 fallback.
2. ⏳ **Layer styles** — stroke, glow, drop shadow as composite-time effects.
3. ⏳ **Tilt-aware brush dynamics** — consume `pen_tilt` for size/opacity curves.
4. ⏳ **Selection combine modes** — Replace / Union / Subtract / Intersect applied to new selections (wand, marquee, lasso).
5. ⏳ **Preferences UI** — expose autosave interval, undo budget, JPEG quality, theme.
6. ⏳ **Multi-document tabs** — `Vec<Document>` + per-tab history/path/dirty state.
7. ⏳ **History panel** — visible undo stack with memory readout.

## Deferred (needs an architecture decision)

- 💤 **Cross-platform port** — the app is Win32 + GDI (`SetDIBitsToDevice`) with a hand-rolled message loop; moving to `eframe`/`winit`/`wgpu` is a rewrite of `aurelia-app`, not a patch. Until then the README claims Windows only.
- 💤 **ICC colour management / 16-bit pipelines** — every layer is 8-bit sRGB `Vec<u8>`; a colour-managed float pipeline touches the rasterizer, filters, blend modes and the project format.
- 💤 **PSD import/export**, **APNG / animated WebP / MP4 export** — need third-party codecs (the `image` crate cannot write animated WebP).
- 💤 **`render_ui` split** — 3.4k-line function; panel extraction is mechanical but touch-heavy, best done behind the new tests.
- 💤 **VPack + Ed25519 signing** — retired in v2.1.0 for simplicity: releases are now a portable zip, an Inno Setup installer and a SHA-256 manifest. `keys/aurelia-publisher.*` are kept unused so signing can return without minting a new identity. If it does, the plan is a `sign` stage between `verify` and `publish`, plus a `.vpack` (or `.zip.sig`) asset — not a change to the stage order.
- 💤 **Code-signing certificate** — the installer and `aurelia.exe` are unsigned, so SmartScreen warns on first run; an Authenticode certificate would remove that prompt and is the higher-value integrity upgrade compared with archive signing.
