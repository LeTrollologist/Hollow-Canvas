# Pixel format abstraction — proposal (draft for review)

**Status:** draft, for review on GitHub issue #11. Gates 1–5 are answered below. Gates 6
(performance) and 7 (correctness) are *acceptance gates*: they are specified here but can
only be satisfied by a prototype and by the oracle, not by prose.

**Scope:** a document-wide working format. Mixed-format documents are explicitly out of
scope for the first implementation (see §5.3 for why, and for what is still reserved in
the file format so this is not a dead end).

---

## 0. Three concepts that must not be merged

These are separate settings. Conflating them is how a "we support 16-bit now" release ends
up claiming linear-light compositing it does not have, or paying linear-light's cost for a
storage change that did not need it.

| Concept | Responsibility | Decided by |
| :--- | :--- | :--- |
| **Storage format** | How layer pixels are represented on disk and in RAM | AUR format + `LayerBuffer` |
| **Working precision** | Precision used *during* filters and compositing | Each operation's implementation |
| **Working colour space** | How colour values are interpreted and combined | A separate, later decision |

Worked consequences:

- **16-bit sRGB storage does not provide linear-light compositing.** Storing more levels of
  a gamma-encoded signal preserves the same gamma-encoded arithmetic; blends still happen
  on encoded values. Linear light is a *colour space* change.
- **f32 filter intermediates do not require f32 persistent storage.** A blur can accumulate
  in f32 and quantise once on write-back into 8-bit or 16-bit storage.
- **A colour-space change alters output by definition**; a storage change must not. That is
  why colour space cannot ride along with the first implementation: it would invalidate the
  byte-identical oracle that makes the storage change safe.

This proposal changes **storage** only. Working precision stays as it is today (f32 for
compositing, per-filter as today). Working colour space stays sRGB-encoded. That keeps the
`v * 257` oracle gate (§7) meaningful.

---

## 1. Buffer responsibilities (gate 1)

```rust
/// The pixel storage of one layer. One variant today; a second is the point of the design.
pub enum LayerBuffer {
    Srgb8(Vec<u8>), // RGBA8, row-major, len == width * height * 4
}
```

Rules — these are the interface contract, not implementation notes:

1. **The buffer owns bytes; the `Layer` owns the buffer.** No borrow of a buffer may
   outlive a mutation of the layer.
2. **The buffer knows nothing else.** Not the document, the canvas size, masks, blend
   modes, or the file format. Anything that needs two of those belongs in the caller.
3. **The buffer does not convert.** Conversion is an operation on buffers, not something a
   buffer does to itself behind an accessor.
4. **No scratch inside the buffer.** Temporary storage is owned by whoever performs the
   operation (§4.3), so its lifetime and its threading are visible.
5. **Structural change is named by intent**, not by `Vec` method: `resize_to(width,
   height)`, `replace_with(buffer)`, `clear()`. Phase 1's `pixels_vec_mut()` is deleted,
   not generalised — it existed to make `resize`/`truncate` possible, and those are now
   named operations with defined semantics for *every* variant.
6. **Construction is explicit**: `LayerBuffer::srgb8(width, height)` for a zeroed buffer,
   `LayerBuffer::srgb8_from(Vec<u8>)` when adopting bytes (import, history restore).

Ownership rules for conversion follow from rule 4: a conversion either returns a new
`LayerBuffer` or writes into a caller-provided one. It never allocates scratch the caller
cannot see, and it never mutates the source.

---

## 2. Format access (gate 2)

```rust
impl LayerBuffer {
    pub fn as_srgb8(&self) -> Option<&[u8]>;
    pub fn as_srgb8_mut(&mut self) -> Option<&mut [u8]>;
    pub fn as_srgb8_exact(&self) -> &[u8];   // documented panic; for provably-8-bit callers
    pub fn format(&self) -> PixelFormat;
}
```

Rules:

1. **`Option`, not panic, for general access.** With one variant a panic is a latent
   assertion; with two it is a production crash on a user's document. `Option` forces the
   caller to choose between converting and refusing.
2. **`as_srgb8_exact` exists for call sites that are structurally 8-bit** (thumbnail
   upload, the 8-bit export path). Its panic is a deliberate, documented invariant, and it
   is grep-able for audit.
3. **Accessors never convert.** A caller that needs another format calls `convert_to(...)`
   and that call is visible in code review and in profiles.
4. **Slices only, never per-pixel accessors.** Phase 1 measured this: the compositor binds
   the slice once per layer; an accessor call per pixel is a real cost in the hot loop.
5. **`as_srgb8_mut` is not a licence to assume 4 bytes per pixel.** Every caller computes
   its stride from `format().bytes_per_pixel()`, or from an explicit width it validated.

---

## 3. Computation (gate 3)

One implementation per operation, generic over a scalar, monomorphised:

```rust
pub trait Channel: Copy + 'static { /* to_f32, from_f32_clamped, MAX, BITS */ }

fn compose_rows<C: Channel, F>(...)            // one body, instantiated for u8 and u16
pub fn compose_layer_srgb8(...)                // thin wrapper: no turbofish at call sites
pub fn compose_layer_linear16(...)
```

Rules:

1. **Dispatch once per layer**, where the storage format is matched. Never per pixel, never
   per row. The 3.3× gain in v2.6.0 came from monomorphised per-mode loops; a `match`
   inside the pixel loop would spend it.
2. **Concrete public wrappers per format.** Call sites must not carry turbofish or type
   inference puzzles; the format-specific name also documents which path is taken.
3. **The `u8` instantiation must generate today's code.** The acceptance check is benchmark
   parity on the 8-bit path plus the oracle — not "it looks generic so it should be fine".
4. **Filters are generic in the same way**, but migrated one at a time. A filter that only
   exists at one precision returns an error rather than silently converting.

---

## 4. Conversion boundaries and scratch ownership (gate 4)

This is the part most likely to make or break the performance gates, so it is specified as
rules rather than left to implementation taste.

### 4.1 Exactly two boundaries per pass

1. **storage → working**, at the start of a composite/filter pass.
2. **working → storage**, at write-back.

Not per operation, not per layer, not per pixel. A document with 4 layers converts 4 times
in and 4 times out per pass, and the intermediate results stay in working precision for the
whole pass.

### 4.2 Enumeration of quantisation points

Every place a value loses precision is listed here, with its rounding rule, so the oracle
can pin each one:

| Point | Rule |
| :--- | :--- |
| Composite write-back | `round(x).clamp(0, MAX)` — today's behaviour, unchanged for u8 |
| Filter write-back | each filter states its rule; the migration keeps today's rule per filter |
| Import (16-bit source) | `round(v * MAX / 65535)` — the documented lossy step |
| Export | per format; PNG/TIFF may stay 16-bit, others downconvert with the same rule |

### 4.3 Scratch ownership — and why it must be per-thread

A `PassScratch` is owned by the document, reused across passes, and capacity-preserving
(no allocate/free per frame). **It is not a single buffer:**

Rows are composed in parallel (v2.6.0 splits a layer's rows across up to 8 threads with
disjoint output slices). A single shared scratch buffer would either serialise the
compositor or need locking inside the hot loop. Therefore:

- scratch is split into **one slice per row band**, mirroring the existing row ownership;
- each thread writes only its own band, so results stay independent of scheduling and the
  parallel-vs-serial oracle keeps holding;
- the scratch is sized once per pass from the band size, not per row.

Rules: no allocation in the hot path, no `thread_local!`, no hidden arena. If an operation
needs scratch, it takes a `&mut Scratch` parameter, so the cost is visible at the call site.

---

## 5. Persistence (gate 5)

### 5.1 AUR v3 layout

- **Document-level format tag** in the header (`0 = Srgb8`, `1 = Linear16`, …). One tag, not
  one per layer.
- **Per-layer field reserved**: a per-layer `format` byte whose value `0` means "inherit the
  document format". Written as `0` always, in the first implementation.
- This reserves the shape without building the feature: a future mixed-format editor does
  not need another format version, and readers that do not understand mixed formats see
  `0` on every layer and cannot be confused.

### 5.2 Migration rules

| Source | Behaviour |
| :--- | :--- |
| AUR v1 | Loads as Srgb8 (format did not exist) |
| AUR v2 | Loads as Srgb8 (masks added, format still implicit) |
| AUR v3 | Loads the tag; a layer tag of `0` inherits |
| Unknown format tag | **Hard error** — never a silent conversion, which would corrupt artwork without telling anyone |

Saving always writes the document's format and `0` for every layer, so files round-trip
with their precision intact and mixed-format writers have room later.

### 5.3 Why mixed-format documents are out of scope

Per-layer depths cost a UI tag on every row, defined behaviour for every pair of depths
inside one file, a conversion at every operation boundary instead of at two edges, and
filters that behave differently depending on the layer they run on. None of that is needed
to deliver the actual benefit (precision).

The realistic pressure is **import**: a 16-bit image into an 8-bit document. That is better
answered explicitly than by supporting the matrix:

- **Promote the document** on import when it is empty or has a single untouched layer, or
- **Convert on import** and tell the person the source depth was reduced.

Recommendation: promote only for a pristine document; otherwise convert with a visible
notice. The reserved per-layer byte in §5.1 means this can be revisited additively.

---

## 6. Performance prototype (gate 6) - measured

An **isolated** prototype, not an integration:

- No UI, no `Document` state, no file format. A benchmark in the shape of
  `crates/aurelia-render/examples/bench_composite.rs`, carrying a candidate implementation
  of §3 + §4 (generic channels, per-band scratch) next to the current monomorphised code.
- **Workloads that must all be measured**, because they stress different things: **opaque**
  (pure arithmetic), **sparse** (the transparent-pixel skip that dominates real artwork),
  **masked** (extra read per pixel), **clipped** (an extra base-alpha read per row).
- Method: interleaved rounds, median reported, best shown for spread, machine and compiler
  printed — the v2.6.1 harness, unchanged.

**Target definition (decided):** the 16-bit target is measured against the **generic u8 path**,
not the mixed production fixture - both formats then use equivalent computation paths, while
the mixed fixture is dominated by an 8-bit fast path a 16-bit document cannot have.

**Deferred, not rejected:** a 16-bit fast path stays a documented future option; building one
now would add a second arithmetic model before the general 16-bit path is proven.

Acceptance, against the numbers shipped in v2.6.0/v2.6.1:

| Workload | Requirement |
| :--- | :--- |
| Opaque general | ≥ 60 MPix/s (the T4 gate) |
| Sparse / masked / clipped | no regression against the current path on the same machine in the same run |
| Plain stack (fast path) | unchanged — the fast path is not part of this change |

**Kill criterion, stated up front:** if the candidate model cannot reach T4 on opaque, the
storage change does not ship in that form. The likely failure is the double-width buffer
halving a memory-bound compositor; the prototype exists to find that out for a day's work
instead of after a migration.

---

## 7. Correctness (gate 7)

The oracle gate is a release requirement, not a nice-to-have:

- 8-bit storage maps to 16-bit as `v * 257`, which is exact and lands on the 16-bit lattice.
  Because `65535 = 255 × 257`, `(257v)/65535` and `v/255` are the same real value, so f32
  blend maths is bit-identical, and `(v16 + 128) / 257` recovers the byte exactly.
- Therefore the existing **702-combination oracle** runs unchanged in 16-bit mode and is
  compared **after downconversion, byte for byte**. It is not relaxed, not re-baselined, and
  not replaced by "close enough".
- The integer fast path is the one exception: its rounding differs at 16-bit, so it needs
  either an explicit ±1 LSB tolerance with a written argument, or a rewrite to the same
  rounding model. Either way the decision is recorded, not implied.
- Parallel-vs-serial and determinism tests apply to the new path exactly as they do today.

---

## 8. Migration order (after approval)

1. `LayerBuffer` enum with `Srgb8` only; `Layer` holds it; accessors move to the buffer with
   `Layer` delegating. **Gate:** all 80 tests and the 702 oracle unchanged.
2. `Channel` generic compositor; the `u8` instantiation must be benchmark-parity with today.
   **Gate:** prototype numbers from §6 reproduced in the real compositor.
   **Done for `u8`** - it did not merely reach parity: it beat the old general path by
   14-57% with the integer fast path untouched. See section 10.
3. `Linear16` variant + conversion + per-band scratch; compositor dispatches once per layer.
   **Gate:** oracle via downconversion; T4 held.
4. AUR v3 write/read with the document tag and the reserved per-layer byte. **Gate:** v1/v2
   files still load; v3 round-trips; unknown tags error.
5. Filters one at a time, each with its own before/after oracle comparison.
6. Import policy implementation (§5.3). **Gate:** a 16-bit import is either promoted or
   converted — and the notice is shown.

Steps 1–2 are safe independently of the 16-bit decision; step 3 is the point of no return.

---

## 9. Prototype results (gate 6) - measured

`crates/aurelia-render/examples/proto_pixel_format.rs` implements the ?3 + ?4 model in
isolation and was run on an Intel Alder Lake (12 logical cores), rustc 1.93.1, release
profile, 1920x1080, 4 layers, 20 iterations x 5 interleaved rounds, medians.

### Correctness of the model, proven not assumed

The prototype asserts, before measuring anything, that:

- the `u8` instantiation reproduces `Document::composite_layers` **byte for byte**; and
- a `v * 257` `u16` document downconverts to the **same bytes**.

Both hold on all five fixtures (opaque, sparse, masked, clipped, and the mixed bench
fixture). That is the `v * 257` lattice gate demonstrated empirically rather than argued.

### Throughput (MPix/s, median of 5 interleaved rounds)

| Workload | production u8 | candidate u8 | candidate u16 |
| :--- | ---: | ---: | ---: |
| opaque (bench shape: 3 fast + 1 general) | 74.6 | 43.1 | 43.5 |
| opaque, all layers general | 24.6 | 37.8 | 41.3 |
| sparse 10%, all layers general | 168.7 | 285.7 | 175.5 |
| masked, all layers general | 35.1 | 47.6 | 46.3 |
| clipped, all layers general | 35.7 | 47.7 | 44.2 |

### Gate 6 verdict: **T4 fails, and not for the expected reason**

- **T4 on the bench fixture: 43.5 MPix/s against a 60 MPix/s gate - FAIL.**
- **The storage width is not the cause.** The bench fixture lets three layers take the
  8-bit integer fast path; a 16-bit document cannot have that fast path under decision 5,
  so every layer pays general-path cost. The *8-bit* all-general figure (24.6 MPix/s) is the
  same ceiling or lower, and the candidate u16 (41.3) beats it.
- **Width matters only where the loop is bandwidth-bound.** Opaque is compute-bound and
  pays +9% for double width (37.8 -> 41.3, within noise). Sparse is bandwidth-bound - it
  does almost no arithmetic per pixel because transparency is skipped - and pays **-39%**
  (285.7 -> 175.5). Sparse is the realistic artwork case, so this is the number to respect.
- **The generic model is faster than today's general path on every all-general workload**
  (37.8 vs 24.6 opaque, 285.7 vs 168.7 sparse, 47.6 vs 35.1 masked, 47.7 vs 35.7 clipped).

### Consequences for this proposal

1. **Gates 1-5 stand, but the target for a 16-bit document cannot be T4 as defined.** T4 was
   set on a mixed fixture dominated by the 8-bit fast path. Either the 16-bit target is
   defined against the general path, or a 16-bit fast path is built (decision 5 deferred
   that as a second project). This is a decision for review, not a number to lower quietly.
2. **?4.1 is corrected by this measurement.** "Intermediate results stay in working
   precision for the whole pass" would change output - it is precisely the per-layer
   quantisation that makes the oracle byte-identical. The first implementation therefore
   quantises per layer, and a whole-pass f32 buffer is a *separate* proposal that would
   need its own oracle re-baselining. Consequently the scratch question in ?4.3 shrinks:
   there is no document-wide f32 image, only per-band conversion buffers.
3. **Step 2 of the migration is worth doing on its own.** The generic model beats the
   current general path on every workload measured, independent of any 16-bit decision.

## 10. Step 2 landed: the generic model is in production for u8

The model from section 3 is now the compositor's general path, `u8` only:

- `BlendMode::composite_working` is the **single** implementation of the blend maths, in
  working precision. `composite_pixel` is a thin 8-bit storage wrapper over it, and the
  per-mode `compose_*` functions call it with a literal mode, so dispatch stays out of the
  pixel loop. One arithmetic implementation, not one per storage width.
- `Channel` (in `aurelia-core::pixel`) has exactly one implementation, `u8`, with a test
  asserting the storage <-> working conversions round-trip exactly for all 256 values.
- `compose_rows` is generic over the channel; `compose_layer` stays 8-bit because the source
  slice comes from `as_srgb8()`.
- **The 8-bit integer fast path is untouched** - the diff does not go near it.

Correctness: 55 core tests pass, including the **702-combination byte-identical oracle**
across 13 blend modes, the 234 clipping corners, parallel-vs-serial and determinism.

Measured on the same harness as the v2.6.1 baseline (medians of 5 interleaved rounds,
1920x1080, 4 layers):

| Workload | v2.6.1 | now | change |
| :--- | ---: | ---: | ---: |
| General, opaque | 79.8 | **91.3** | +14% |
| General, 50% coverage | 95.9 | **122.1** | +27% |
| General, 10% coverage | 157.3 | **191.9** | +22% |
| General, 10% + mask | 154.9 | **177.7** | +15% |
| General, clipping | 56.6 | **68.3** | +21% |
| General, clipping 10% | 137.1 | **215.1** | +57% |
| All 13 modes | 64.3 - 79.9 | **80.9 - 94.1** | every mode up |
| Plain stack (fast path) | 141.6 | 133.4 | unchanged code, within noise |
| With layer mask | 127.2 | 129.0 | unchanged code |

T4 (>= 60 MPix/s) holds at 91.3, and the slowest mode (Luminosity) rose from 64.3 to 80.9.

## Open questions for review

1. **`Option` vs panic** in `as_srgb8` (§2 rule 1) — is `as_srgb8_exact` acceptable, or
   should every call site be forced to handle the format explicitly?
2. **Where does `dimensions()` live** — the buffer, or the `Layer` (which already has
   `width`/`height`)? Duplication invites disagreement between them.
3. **Per-layer format byte in v3** (§5.1) — reserve it now as `0`-always, or leave it out
   until mixed formats are a real proposal? Reserving costs one byte per layer and buys
   forward compatibility.
4. **Import policy** (§5.3) — promote vs convert, and what the notice says.
5. **Does the fast path get a 16-bit variant at all**, or does a `Linear16` document simply
   take the general path? The fast path's value is 8-bit integer maths; a 16-bit version may
   not be worth its maintenance.


