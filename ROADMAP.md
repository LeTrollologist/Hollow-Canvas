# Aurelia · Roadmap

**Status:** v3.4.1 shipped (2026-11-05). Native 16-bit editing, compositing, filters, text,
selection edits and free transform are complete. The next phase is **stabilization, viewport
correctness and distribution trust**, not another engine feature.

Legend: Now = current focus · Next = queued · Later = designed, not started · Deferred = out of scope

---

## The short version

Aurelia's engine is ahead of its product surface. The most valuable next work is not another
pixel feature; it is making the thing users actually see and install trustworthy:

1. **Viewport truth.** The canvas is the product, and it has had recurring visibility/refresh
   bugs, no visual regression tests, and a 16-bit document is still presented at 8 bits. This is
   the highest-value code work.
2. **Distribution integrity.** The installer is unsigned, and that is now a permanent constraint:
   no public Authenticode path exists and a self-signed certificate would not help SmartScreen.
   The release must therefore make the artifact verifiable through SHA-256 and Ed25519
   signatures, and state the unsigned status honestly.
3. **Release and documentation hygiene.** v3.4.x exposed gaps: `Cargo.lock` is not part of the
   version stage, issues #15 and #28 are implemented but still open, and this roadmap had drifted
   three releases behind.
4. **Measured performance.** Only after correctness: the 16-bit sparse path is at 0.67 against a
   0.60 budget (pass, little headroom), and the bilinear transparent-edge limitation is still
   open.
5. **Then UX gaps**: mask painting, selection combine modes, preferences, history panel.

**Not now:** 32-bit floating-point / linear-light editing, a custom GUI framework to replace
egui, or a cross-platform rewrite. Each is a separate proposal with its own gate and kill
criterion.

---

## What changed since the last roadmap

| Release | Change |
| :--- | :--- |
| v3.2.0-v3.4.0 development | 16-bit documents in the editor (#13), native 16-bit text (#14), native compositing output (#15), 16-bit filters (#28), adjustment layers, merge-visible / merge-down, selection fill/stroke, navigator |
| v3.4.0 | Free transform made native at both storage widths; 16-bit editing complete at `checkpoint/v3.4.0-16bit-complete` (`bfa6709`) |
| v3.4.1 | Live Navigator during strokes; paper/background/transparency invalidation; release verification fix |

Current measurements are in [docs/PERFORMANCE.md](docs/PERFORMANCE.md), the 16-bit record is in
[docs/16bit-plan.md](docs/16bit-plan.md), and release mechanics are in
[RELEASE_PROCESS.md](RELEASE_PROCESS.md).

---

## Now — P0: viewport truth

**Why first:** issue #16 ("Canvas Not Visible in Main Viewport") has been reported once and
fixed once, and the v3.4.x Navigator work showed the same class of bug can return. Users judge
every engine improvement through this surface. A broken viewport makes a correct 16-bit
compositor irrelevant.

Actions:

1. **Add a viewport regression harness.** State/renderer tests for an empty 8-bit document, an
   empty 16-bit document, a transparent document, a dark-paper document, DPI-scaled layouts,
   zoom/pan round trips, and the Navigator framing box. Prefer a small deterministic rendered
   fixture over a screenshot service.
2. **Audit the paper/background model.** One source of truth for `background_value`,
   `is_transparent`, theme paper and the rendered framebuffer. A black canvas must be an explicit
   background choice, never a default that looks like a broken renderer.
3. **Keep the Navigator live and cheap.** Current per-frame updates are correct but should get a
   measured cost budget; coalesce uploads if the downsample becomes visible in frame timing.
4. **Decide the 16-bit presentation question with a timeboxed spike.** Prototype one RGBA16
   texture path through a custom paint callback or native surface. Do not fork egui and do not
   rewrite the UI. The spike succeeds only if it keeps the existing viewport tests green and
   costs no measurable frame rate.

Progress:

- The viewport and Navigator now share `Document::paper_rgb8()` and
  `Document::transparency_checker_rgb8()`; the renderer, thumbnail and UI cannot disagree about
  paper or transparency.
- Deterministic tests now cover empty 8-bit and 16-bit paper, dark paper, the checkerboard,
  zoom/pan bounds, DPI point/pixel conversion, viewport-fraction mapping and dock clamping.
- The Navigator budget is measured: 1.62 ms per refresh at 1080p and 2.72 ms at 4K. Live
  rebuilds are capped at ~30 Hz during a stroke, with an immediate final refresh on mouse-up.
- The 16-bit presentation decision is written:
  [docs/16BIT_PRESENTATION.md](docs/16BIT_PRESENTATION.md). It is deferred behind a 10-bit/HDR
  output proposal, not implemented by adding a texture.

Status: **code complete; observation continues.** The remaining item is the two-release window
with no canvas-invisible/black-viewport report.

Exit criteria:

- [ ] No canvas-invisible/black-viewport report across two consecutive patch releases.
- [x] Viewport tests run in the normal local suite, not only by hand.
- [x] A written decision on 16-bit presentation: adopt, defer with a date, or reject with evidence.

---

## Now — P0: distribution integrity and release hygiene

**Why now:** Authenticode is not available for this project. A self-signed certificate is worse
than useless for SmartScreen because it does not chain to a trusted CA. Unsigned distribution is
therefore a **permanent product constraint**, not a pending task. The production bar is not
"make SmartScreen green"; it is "make the artifact verifiable, the warning honest, and the release
repeatable."

Actions:

1. **Keep the existing Ed25519 artifact signatures and `SHA256SUMS.txt`.** Publish the public-key
   fingerprint and the exact verification command in every release note. This is the integrity
   boundary; it is independent of Windows code signing.
2. **Keep VirusTotal scanning advisory.** Do not gate a release on heuristic/ML detections that
   routinely fire on unsigned software. Provide a false-positive reporting path.
3. **Close implemented issues.** #15 (native 16-bit output) and #28 (16-bit filters) are shipped;
   verify the acceptance criteria, comment with the tests/commits, and close them. Re-scope #5
   (Authenticode) as **won't fix — no public certificate path**, with the unsigned policy linked.
4. **Fix the version-stage gap.** `Cargo.lock` is updated by quality/build after the release
   commit and currently lands in a post-release commit. Add it to `version_files` (or to the
   version stage explicitly) so a release leaves a clean tree without a follow-up commit.
5. **Add failure-injection tests** for the release journal: killed upload, tag pushed but release
   creation failed, draft digest mismatch, interrupted version commit, remote tag drift. The
   happy path is tested; the failure path is still described as "needed" in RELEASE_PROCESS.md.
6. **Keep release notes and version examples current.** Every release should update README
   examples and release notes in the version commit, not by hand afterwards.

Progress:

- The version stage now includes `Cargo.lock` automatically; the git-profile end-to-end test
  asserts the release commit owns the lock and leaves a clean tree.
- The journal's status is no longer sticky: a failed verification that passes on resume now
  reports `published`/`verified`, and `last_error` clears.
- Failure injection covers partial upload resume, digest mismatch, release-creation failure,
  remote tag drift, interrupted operation, and post-publication draft verification.
- #15 and #28 are closed. #5 is out of scope by policy, not blocked on engineering.

Remaining:

- Cut one release through the full pipeline with the unsigned policy in its notes, and verify
  the published hashes and Ed25519 signatures end to end.
- Add the partial-journal-append, compensation-failure and clean-clone rehearsal cases.
- Prove the next release leaves `main` clean (the lock fix is in place but not yet exercised by
  a release).
- Document the additive `AED1` editor-state section in `docs/PROJECT_FORMAT.md`.

Exit criteria:

- Unsigned status and the SHA-256/Ed25519 verification path are stated in README, installer
  notes and release notes.
- Published assets verify against the published hashes and signatures.
- A release run leaves `main` clean with no manual post-release commit.
- #15 and #28 closed; #5 explicitly re-scoped as won't-fix.
- Failure-injection suite covers the listed crash points.

---

## Next — P1: stabilization and performance

**Why:** 16-bit is complete but the remaining performance headroom is thin, and one interpolation
limitation is still documented rather than fixed.

1. **Soak the 16-bit paths.** Long editing sessions across paint, filters, transform, undo/redo,
   save/load, and animation frames. Measure memory, retained history and time-to-first-frame.
2. **Pin bilinear transparent-edge behavior first.** The sampler averages colour and alpha
   independently; a zero-alpha border with non-black RGB can darken interpolated edges. Add a
   regression test that captures the current 8-bit and 16-bit output before changing anything.
   Fix it if the smallest correct change is safe; otherwise record a formal accepted-limitation
   decision with the test, user guidance and a follow-up issue. This is the first P1 item.
3. **Sparse 16-bit throughput.** Current gate-2 ratio is **0.67** against a **0.60** budget.
   Preserve the budget and add regression coverage for sparse 16-bit workloads. The 0.75 target
   remains a goal, not a v3.5.0 gate; record paired rounds in PERFORMANCE.md. Do not bundle
   throughput changes with correctness fixes.
4. **General-path tail.** Slowest blend mode is Luminosity at 82.5 MPix/s; the opaque general path
   is 105.4. Optimise only with a benchmark and a rollback number.
5. **Memory.** Keep the delta-history budget honest at 1080p and 4K; measure, do not infer.

Exit criteria:

- Bilinear edge fixed, or formally accepted with a pinned regression test and user guidance.
- Sparse ratio stays at or above the 0.60 budget with regression coverage.
- Soak results recorded with the machine and build.

---

## Next — P2: UX and editing gaps

These are user-visible gaps, not architecture changes. They are **not** v3.5.0 release gates
unless they regress an existing workflow; the release gate is in
[RELEASE_PROCESS.md](RELEASE_PROCESS.md#9-v350-production-gate).

| Item | Why it matters | Notes |
| :--- | :--- | :--- |
| **Mask painting** | Masks can be created but not brushed; non-destructive workflow is incomplete | Route freehand strokes at a mask target, preserving pixel layers |
| **Selection combine modes** | Replace / Union / Subtract / Intersect is expected in any editor with selections | Extend `SelectionMask` and the Select menu |
| **Preferences UI** | Autosave interval, undo budget, JPEG quality and theme are file-only today | No new backend; expose existing state |
| **History panel** | Undo is invisible; users cannot see what will be undone or how much memory it holds | Read-only list with a memory readout |
| **Navigator/viewport polish** | Fit/centre/zoom now behave; add explicit tests for DPI and dock-width changes | Follows the P0 harness |
| **Measurement guides** | Move the cutting-mat workflow into the viewport | **Shipped:** drag guides out of the rulers, move the grid origin, `+ H` / `+ V` / `Clear`, Move-tool **Selected layer / Full move** modes, per-source snapping to guides/layers/canvas, selection snapping, `Alt` bypass, `Lock guides`, movable/rotatable/stretchable straight-edge ruler with draw-along-ruler strokes and length/angle readout, and AUR persistence of guides/grid origin/ruler |

---

## Later — P3: larger bets

Only after the P0/P1 exit criteria:

- **Layer styles**: stroke, glow, drop shadow as composite-time effects.
- **16-bit presentation**: adopt only if the P0 spike proves it without destabilising the viewport.
- **Multi-document tabs**: `DocumentSession` refactor, per-document history/path/pan/zoom.
- **Colour management**: ICC profile reference, working-space conversion, soft proofing.
- **PSD import/export, animated WebP/MP4**: third-party codecs; separate scoping.
- **Cross-platform port**: `eframe`/`winit`/`wgpu` front-end swap; not a small port while the
  renderer is GDI-hosted.

---

## Deferred / explicitly not started

- **32-bit floating-point / linear-light editing.** It changes how values combine, invalidates
  the byte-identical 8-bit oracle and needs its own proposal, gate and kill criterion. 16-bit
  sRGB storage is the completed milestone.
- **Forking or replacing egui.** The viewport is a GDI framebuffer path, not an egui texture
  limitation. A custom UI framework is not justified by the current bug class.
- **A 16-bit fast path.** No evidence it is needed before the sparse-path work is measured.
- **Authenticode is not deferred**; it is P0 above.

---

## Shipped record

| Area | State |
| :--- | :--- |
| 16-bit sRGB documents | Storage, editing, compositing, filters, text, selection, transforms, merge, save/load |
| Free transform | Affine, quad, mesh, TPS; native at both widths; observational-invariance tests |
| Paint / erase / fill / gradient / shapes | Native 8-bit and 16-bit |
| Layers | Groups, masks, clipping, alpha lock, adjustment layers, text, animation timeline |
| Release engineering | Journaled `tools/release`, Ed25519 signatures, draft/verify/publish, installer + portable zip |
| Platform | Windows 10/11 x86-64, per-monitor V2 DPI, Win32 + GDI |

The detailed historical performance work (v2.5-v2.7 compositor optimisations) is preserved in
[docs/PERFORMANCE.md](docs/PERFORMANCE.md) rather than repeated here.

---

## Working rules

1. **Correctness before throughput.** No optimisation is bundled into a correctness change; no
   correctness fix is hidden inside a performance change.
2. **A test or it did not happen.** New viewport, release and 16-bit claims need a deterministic
   test or a recorded measurement.
3. **One source of truth.** Version numbers, release notes, issue state and roadmap status move
   together in the release commit.
4. **No silent fallback.** An unsupported format or operation refuses with a message; it never
   downconverts behind the user's back.
