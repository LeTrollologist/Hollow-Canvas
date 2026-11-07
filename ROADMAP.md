# Aurelia · Roadmap

**Status:** v3.6.0 shipped 2026-11-07. Native 16-bit editing, compositing, filters, text,
selection edits and free transform are complete. Measurement guides, the straight-edge ruler,
Move modes, snapping and persisted editor state are shipped. v3.6.0 closed the editing-workflow
gaps: mask painting, selection combine modes, a Preferences interface, a History panel, scoped
tool/brush reset, and persistent ruler/guide/snap interaction.

The engine is ahead of the product surface. The next work is **viewport truth, project-file
hardening, release repeatability, and essential editing workflows** — not another engine feature.

**Legend:** Now = current focus · Next = queued · Later = designed, not started · Deferred = out of scope

---

## How this roadmap is organized

Every workstream is planned in four columns:

| Column | Question |
| :--- | :--- |
| **Feature** | What does the user get? |
| **Implementation** | What code/architecture changes make it real? |
| **Hardening** | What failure, security and compatibility cases must it survive? |
| **Testing** | What deterministic evidence proves it works? |

A phase is complete only when all four columns have evidence. A feature with no hardening or
test work is not done.

---

## Release baseline

- **v3.6.0** is the current release. Assets, SHA-256 manifests and Ed25519 signatures verify;
  the installer is unsigned by permanent policy.
- **256 tests pass, 1 ignored**; clippy reports no new lints.
- Sparse 16-bit composite ratio is **0.70** against the **0.60** budget.
- The bilinear transparent-edge behavior is pinned as an accepted limitation; it is not silently
  changing.
- The viewport is still an **8-bit presentation** of 16-bit documents. This is a known display
  limitation, not a storage limitation.
- **Process debt closed:** v3.6.0 release notes were written before the tag, and preflight now
  fails when `release-notes/<tag>.md` is absent.

---

## Shipped — Phase 0: v3.5.1/v3.5.2 viewport and file-trust hardening

**Goal:** no new editing features. Make the canvas trustworthy and make crafted project files
safe to open.

### 0.1 Viewport regression harness

| | |
| :--- | :--- |
| **Feature** | None; this is a reliability release. |
| **Implementation** | Add deterministic rendered fixtures for: empty 8-bit paper, empty 16-bit paper, transparent canvas, dark paper, zoom/pan round trips, DPI-scaled layouts, Navigator framing. Centralize paper/background invalidation so the renderer and thumbnail cannot disagree. |
| **Hardening** | Treat a black viewport as an explicit background value, never a default. Cover dock-width and monitor-DPI changes. No silent fallback to an unpainted buffer. |
| **Testing** | Pixel-level fixtures with stable dimensions and color values; state tests for zoom/pan and DPI conversion; a manual matrix for 100%/125%/150%/200% scaling and two monitors. |

### 0.2 Project-file allocation and parsing audit

| | |
| :--- | :--- |
| **Feature** | None. |
| **Implementation** | Cap every file-controlled allocation before allocating: text content (8 MiB, shipped), font-family length, layer names, masks, decompressed payloads, preview PNG, `AED1` payload. Reject oversized saves as well as oversized loads. |
| **Hardening** | Add a malformed-file corpus: truncated length claims, huge claims, invalid UTF-8, unknown tags, corrupt editor sections, decompression bombs. Keep unknown format tags a hard error; keep optional `AED1` sections defaulting safely. |
| **Testing** | Regression tests for each length field, including “claim is rejected before payload read”; fuzz the AUR/HCV readers with a seeded corpus; verify no panic and bounded memory on malformed input. |

### 0.3 Release-process closure

| | |
| :--- | :--- |
| **Feature** | None. |
| **Implementation** | Require release notes before `run`; include the standard `virustotal:begin/end` block, artifact table, engine count, report links and SHA-256 values. Keep `SECURITY.md` in the version set. |
| **Hardening** | Clean-clone rehearsal; failure injection for journal append, compensation, tag drift and upload interruption; idempotent resume; no secret in repo or logs. |
| **Testing** | `doctor`, `plan`, `run`, `verify`; independently download assets and check hashes/signatures; confirm `main` is clean afterward and the tag is the release commit. |

### 0.4 Quality guard

| | |
| :--- | :--- |
| **Feature** | None. |
| **Implementation** | Keep `cargo test --release --workspace` and clippy as release gates. Keep `gate_2_throughput_and_memory` as an ignored measurement that asserts the 0.60 budget. |
| **Hardening** | Do not trade correctness for throughput. Any performance change needs a before/after number and a rollback. |
| **Testing** | Unit + integration suite; sparse benchmark; no new ignored tests without a written reason. |

**Exit criteria**

- [ ] Viewport fixtures pass in the normal suite and cover 8-bit, 16-bit, transparent, dark-paper, DPI, zoom/pan and Navigator cases.
- [ ] Every file-controlled allocation is bounded and has a rejection test.
- [ ] A clean clone can run the full release process and leave `main` clean.
- [ ] `release-notes/v3.5.1.md` exists before the tag and includes the standard VirusTotal block.
- [ ] No P0/P1 security or viewport regression is open.

---

## Shipped — Phase 1: v3.6.0 editing workflows

**Status: shipped 2026-11-07.** All five workstreams plus the two follow-ups (#39, #42) landed
with tests; see `release-notes/v3.6.0.md` for the per-issue record.

**Goal:** close the workflow gaps that make the editor feel incomplete without changing the
rendering architecture.

| Workstream | Feature | Implementation | Hardening | Testing |
| :--- | :--- | :--- | :--- | :--- |
| **Mask painting** | Brush directly on a mask, not only create/delete it | Route freehand stroke targets to `LayerMask`; preserve pixel layer and mask alpha; dedicated mask overlay | Alpha lock, selection intersection, undo/redo, 16-bit behavior | Mask pixel invariants, undo/redo, selection + mask composition |
| **Selection combine modes** | Replace / Union / Subtract / Intersect, discoverable in the Select menu | Extend `SelectionMask` operations and UI state; preview before commit | Empty result, fully selected result, antialiased edges, undo | Geometry oracles, boolean-operation tests, edge cases |
| **Preferences UI** | Autosave interval, undo budget, JPEG quality, theme | Bind existing persisted fields; no new backend | Invalid values, migration, defaults, save failure | Settings round-trip, boundary values, restart persistence |
| **History panel** | Show undo entries and memory use | Read-only history API; memory readout | Bounded history eviction, 16-bit snapshots, large documents | Entry ordering, memory arithmetic, undo/redo after eviction |
| **Guides/ruler polish** | Ruler as a snap target; clearer guide handles | Reuse snap candidate model; add angled-ruler intersection cases | Locked guides, empty ruler, off-canvas handles | Snap-target tests, ruler/guide interaction tests |

**Exit criteria**

- Every listed workflow has a feature path, an undo path and a test.
- No workflow requires a hidden modifier or a file edit to use.
- UX work does not regress brush, selection, Move, guide or ruler behavior.

---

## Now — Phase 2: v3.7.0 viewport fidelity

**Goal:** decide and, only if justified, implement a higher-fidelity display path.

| | |
| :--- | :--- |
| **Feature** | A 16-bit document should not be presented as if it were 8-bit, when the display path can support it. |
| **Implementation** | Timeboxed spike for one RGBA16 presentation path; keep the GDI framebuffer and existing UI. No egui fork, no renderer rewrite. Fall back to 8-bit presentation if the path cannot be measured. |
| **Hardening** | Keep the current path selectable; no document-format change; no silent color conversion; verify startup on unsupported hardware. |
| **Testing** | Pixel-diff fixtures at 8-bit and 16-bit, frame-time budget, DPI/scale matrix, fallback test. |
| **Kill criterion** | The spike fails if it destabilizes viewport tests or adds measurable frame cost without a visible fidelity gain. |

Also here: Navigator DPI/dock polish, multi-monitor framing, and any remaining viewport
regressions from Phase 0.

---

## Parallel — stabilization and performance

These are continuous, but only one correctness item is active at a time.

1. **Bilinear transparent edges.** The behavior is pinned by
   `bilinear_transparent_edge_is_pinned_as_known_limitation`. Keep that test; if fixing it, capture
   the 8-bit baseline first and do not bundle the fix with throughput work.
2. **Sparse 16-bit throughput.** Current 0.70 against 0.60. Preserve the budget, add regression
   coverage, and target 0.75 only with paired measurements. Do not chase the target at the cost of
   correctness.
3. **General-path tail.** Luminosity is the slowest mode; optimize only with a benchmark and a
   rollback number.
4. **Memory and soak.** Long 16-bit sessions across paint, filters, transform, undo/redo,
   save/load and animation frames; record machine, build and retained history.
5. **Undo/redo integrity.** Verify bounded history, 16-bit snapshots and selection/mask operations
   under eviction.

**Exit criteria**

- Bilinear behavior is either fixed or explicitly accepted with the pinned test and user guidance.
- Sparse ratio remains ≥0.60 with regression coverage.
- Soak results are recorded; no unbounded memory growth is observed.

---

## Later — P3: larger bets

Only after Phase 0/1 exit criteria and a written proposal:

- **Layer styles**: stroke, glow, drop shadow as composite-time effects.
- **Multi-document tabs**: `DocumentSession` refactor with per-document history/path/pan/zoom.
- **Colour management**: ICC profile reference, working-space conversion and soft proofing.
- **PSD import/export, animated WebP/MP4**: third-party codecs; separate scoping.
- **GPU acceleration**: only if measured CPU limits block real workflows.
- **Cross-platform port**: `eframe`/`winit`/`wgpu` front-end swap; not a small port while the
  renderer is GDI-hosted.

---

## Testing strategy

| Layer | What it proves | Runs |
| :--- | :--- | :--- |
| **Unit / integration** | Core algorithms, format round-trips, state machines, UI-independent behavior | Every change |
| **Golden raster fixtures** | Viewport paper, compositing, transforms, 8/16-bit parity and accepted limitations | Every viewport/rendering change |
| **Fuzz / malformed corpus** | AUR/HCV parser safety, allocation bounds, no panic on crafted input | Every format/parser change; scheduled corpus run |
| **Soak** | Memory, history retention, save/load stability over long sessions | Before a stability release |
| **Performance** | Sparse 16-bit budget, Navigator cost, general-path tail | Every performance change; release record |
| **Release rehearsal** | Clean clone, idempotency, failure injection, signatures, published assets | Every release |
| **Manual hardware** | Pen pressure/tilt, 100–200% DPI, two monitors, install/uninstall | Before a release candidate |

A test is not “coverage” unless it can fail for the bug it claims to prevent. Pixel tests need
stable fixtures; timing tests need paired rounds and recorded machine/build context.

---

## Explicitly not now

- **32-bit floating-point / linear-light editing.** It changes how values combine and invalidates
  the byte-identical 8-bit oracle; it needs its own proposal, gate and kill criterion.
- **Forking or replacing egui.** The viewport is a GDI framebuffer path; a custom UI framework is
  not justified by the current bug class.
- **Authenticode.** No public certificate path exists; self-signing does not establish SmartScreen
  trust. Ed25519 artifact signatures and SHA-256 manifests are the integrity boundary.
- **A 16-bit fast path.** No evidence it is needed before the sparse-path work is measured.

---

## Shipped record

| Area | State |
| :--- | :--- |
| 16-bit sRGB documents | Storage, editing, compositing, filters, text, selection, transforms, merge, save/load |
| Measurement tools | Guides, grid origin, straight-edge ruler, Move modes, snapping, `AED1` persistence |
| Project-file safety | Text-content allocation bound; oversized save rejection |
| Release engineering | Journaled release tool, Ed25519 signatures, SHA-256, VirusTotal advisory scan, installer + portable zip |
| Platform | Windows 10/11 x86-64, per-monitor V2 DPI, Win32 + GDI |
| Editing workflows (v3.6.0) | Mask painting, selection combine modes, Preferences, History panel, tool/brush reset, persistent ruler/guide snapping |

Detailed measurements are in [docs/PERFORMANCE.md](docs/PERFORMANCE.md); the 16-bit record is in
[docs/16bit-plan.md](docs/16bit-plan.md); release mechanics and the production gate are in
[RELEASE_PROCESS.md](RELEASE_PROCESS.md#9-v350-production-gate).

---

## Working rules

1. **Correctness before throughput.** No optimization bundled with a correctness change; no fix
   hidden inside a performance change.
2. **A test or it did not happen.** Viewport, release and 16-bit claims need a deterministic test
   or a recorded measurement.
3. **One source of truth.** Version numbers, release notes, issue state and roadmap status move
   together in the release commit.
4. **No silent fallback.** Unsupported formats and operations refuse with a message; they never
   downconvert behind the user's back.
5. **Unsigned is disclosed, not hidden.** Every release states the unsigned status and the
   verification path.

