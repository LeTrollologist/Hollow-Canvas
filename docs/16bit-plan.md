# 16-bit storage: implementation plan

**Status:** plan for review on issue #11. Three gates, in order. No implementation until
gate 1 is specified *and* reviewed, and gate 3 does not start until gates 1 and 2 pass.

---

## 0. The name: `Srgb16`, not `Linear16`

Naming the variant `Linear16` would fuse two concepts this project deliberately keeps apart
(proposal §0): **storage precision** and **working colour space**. They are different
decisions with different consequences:

| | 16-bit **sRGB** (gamma-encoded) | 16-bit **linear-light** |
| :--- | :--- | :--- |
| What changes | Storage precision only | Storage precision *and* how values combine |
| Blend results | **Unchanged** | Changed by definition |
| Byte-identical oracle | Preserved via the `v*257` lattice | **Impossible** - needs re-baselining |
| Fidelity gained | 256x more levels: gradients, feathering, repeated filters, no banding | Physically correct light mixing |
| Risk | Low: a storage swap behind a proven oracle | High: every blend mode's output moves |
| Scope | This plan | A separate proposal |

**Recommendation: implement `Srgb16` first; keep `Linear16` as the name for a future
linear-light working space.** Reasoning:

1. The byte-identical requirement (gate 1) is only satisfiable by the *encoded* variant. A
   linear-light storage change cannot pass the oracle it would inherit, which means the
   safety net disappears exactly when the change is largest.
2. The fidelity problem that motivates this work is **banding** - 256 levels is too few for
   gradients, feathering and repeated filtering. That is a storage-precision problem, and
   `Srgb16` solves it.
3. Correct light mixing is a genuinely different feature: it changes what a Multiply or
   Screen *means*. It deserves its own decision, its own oracle re-baselining, and its own
   user-visible explanation (existing artwork will look different).
4. The generic model landed in v2.7.0 already computes in f32, so when linear-light is
   wanted the working colour space can change **without another storage change**.

Keeping the name `Srgb16` makes the constraint visible in every `match` arm. `Linear16`
would encode a live assumption that we have linear compositing, which we would not.

---

## Gate 1 - Fidelity and correctness

### 1.1 Definition

`Srgb16`: channel `u16`, **gamma-encoded sRGB**, same encoding as today, alpha linear (as
today), RGBA order, row-major, `len == width * height * 4` channels. Eight-bit content
promoted to it is stored as `v * 257`.

### 1.2 Rounding at every boundary

Every place precision changes is specified here, so each can be pinned by a test:

| Boundary | Rule |
| :--- | :--- |
| u8 -> u16 promotion | `v * 257` - exact for all 256 values, lands on the lattice |
| u16 -> u8 downconvert | `(v + 128) / 257` - exact for lattice values, `+-1 LSB` otherwise (documented) |
| Composite write-back | `round(x.clamp(0.0, 1.0) * 65535)` - same shape as the u8 rule, rescaled |
| Filter write-back | per filter, recorded individually as each is migrated |
| Import (16-bit source) | identity; 8-bit source promoted by `v * 257` |
| Import (other depths) | `round(v * 65535 / source_max)` - the only lossy import step, and it is visible |
| Export, 16-bit formats | identity (PNG, TIFF) |
| Export, 8-bit formats | the standard downconvert above |

### 1.3 Oracle
The 702-combination oracle runs with fixtures promoted `v * 257`, results downconverted,
compared against today's 8-bit output. The oracle is not relaxed, re-baselined or
replaced.

**Measured, and it is exact in one case and bounded in the other:**

| Comparison | Result |
| :--- | :--- |
| Single layer, 13 modes x offsets x mask states x coverages | **byte-identical** |
| Multi-layer stack, same matrix | worst channel drift **1 LSB**, never more |

The reason is arithmetic, not convenience, and it belongs in the plan rather than in a
tolerance nobody reads: the 8-bit chain quantises to 256 levels *between* layers, while the
16-bit chain keeps 65536. A later layer is therefore fed a slightly different input and can
round to the neighbouring byte. With no inter-layer quantisation - a single layer - the
result is exact.

Two consequences for gate 3:

1. Opening an 8-bit stack as 16-bit and saving it back is **not** guaranteed lossless at the
   last bit. If that is unacceptable, the answer is to quantise the 16-bit result back to
   the 8-bit lattice after each layer when the document originated as 8-bit - a per-document
   flag, not a global rule - and that is a decision for gate 3.
2. There is no 16-bit fast path, so layers that would have used the 8-bit integer path have\n   no 16-bit equivalent. Fixtures for this oracle must therefore force the general path on
   both sides, or they compare two algorithms rather than two storage widths.

The 702-combination oracle runs with fixtures promoted `v * 257`, results downconverted,
compared **byte for byte** against today's 8-bit oracle. It must be identical. The oracle is
not relaxed, re-baselined or replaced.

### 1.4 Tests for values u8 cannot represent

The oracle cannot prove the *added* precision, only that nothing was lost. New tests must
prove the point of the change:

- a half-step gradient (values `k * 257 + 128`) that must survive a composite unchanged;
- a 1024-level ramp stored and read back with all levels distinguishable;
- a feathered mask edge at 16 bits, checked for monotonicity with no duplicated steps;
- a repeated filter chain (blur -> sharpen -> blur, N times) where the 8-bit result bands
  and the 16-bit result does not - asserted by counting distinct values, not by eye.

### 1.5 Acceptance

All of the above green, **no 8-bit output changed**, determinism and parallel-vs-serial
holding on the new path.

---

## Gate 2 - Performance and memory

Baseline is the **generic u8 path** from v2.7.0 (decision A), never the mixed fixture.

**Gate definition (approved):** throughput is a **paired ratio of `Srgb16` to `u8`**, measured
over five interleaved rounds with both widths sampled inside the same round, reporting the
median and the spread. Absolute MPix/s is recorded as context only - on a machine that swings
40% between runs of identical code, an absolute threshold is a coin toss rather than a gate.
The sparse limit stays a **relative** one: `Srgb16` must reach at least 60% of u8 throughput.

| Requirement | Target |
| :--- | :--- |
| Opaque general compositing | **>= 60 MPix/s** |
| Masked, clipped | measured and recorded |
| Sparse regression budget | **<= 40%** against the generic u8 path, with an absolute floor of **>= 120 MPix/s** |
| Memory | measured, including undo history |
| u16 fast path | **not built** (decision B) - a documented future option |

Why a 40% sparse budget rather than zero: the prototype measured -39% (285.7 -> 175.5
MPix/s), and that is close to irreducible. Sparse compositing is bandwidth-bound - skipping
transparent pixels leaves almost no arithmetic per pixel - so doubling bytes per channel is
the whole cost. The floor matters more than the percentage: 175 MPix/s is still ~3x the
opaque gate, so the workload stays comfortably real-time.

Memory must be measured, not assumed, and specifically **undo history**: snapshots store
before/after full-layer buffers, so a 4-layer 1080p document with 20 history steps goes
from roughly 0.66 GB to 1.3 GB. If that is judged unacceptable, the answer is a history
redesign (delta or compressed snapshots) *decided at this gate*, not discovered after
gate 3.

**Acceptance:** numbers recorded in the plan; if sparse throughput or history memory falls
outside the budget, the plan changes here - before any production architecture is written.

---

### Sub-step 3: AUR v3 - done

AUR v3 adds a **document format tag** and a **per-layer format byte**: `0` inherits the
`document's format, `1` is `Srgb8`, `2` is `Srgb16`. An ordinary document writes zeros
throughout, which is exactly what the reserved byte was designed to do. Mixed-format
documents are expressible in the file but still not created by the editor - the format
preserves them so that decision stays additive rather than becoming another version.

One writer now takes a `version` parameter, so old formats are produced by the real writer
in their real layout (v2 added masks, v3 added tags) instead of by transforming today's
bytes into something that only looks old.

**Requirements, each with a test:**

| Requirement | Test |
| :--- | :--- |
| 16-bit round-trip, every channel exact | `sixteen_bit_layers_round_trip_every_channel` |
| Mixed-format fidelity, per layer | `mixed_format_documents_round_trip_each_layer_format` |
| v1/v2 fixtures unchanged, layers load as `Srgb8` | `v1_and_v2_files_load_as_eight_bit` |
| Unknown document tag rejected | `malformed_format_tags_and_payloads_are_rejected` |
| Unsupported layer tag rejected | same |
| Truncated 16-bit payload rejected | same |
| Invalid dimensions rejected before allocating | same |

The rejection cases matter more than they look: each one is a place a lenient reader would
have fallen back to 8-bit and silently corrupted someone's document. Malformed input is an
error, never an interpretation.

Writing a 16-bit layer into a v1/v2 file returns `UnsupportedFormat` rather than flattening
it, so a downgrade can never lose precision quietly.

## Gate 2 results - passed, provisionally

`cargo test -p aurelia-core gate_2 -- --ignored --nocapture` on a quiet machine, five paired
interleaved rounds: both widths measured inside the same round, so a slow patch of machine
hits both and cancels in the ratio.

| Workload | u8 MPix/s | Srgb16 | ratio (median) | ratio spread |
| :--- | ---: | ---: | ---: | :--- |
| opaque | 31.9 | 32.2 | **0.97** | 0.91 .. 1.08 |
| sparse 10% | 156.3 | 140.8 | **0.88** | 0.60 .. 0.95 |
| masked | 30.2 | 33.4 | **1.06** | 0.98 .. 1.24 |
| clipped | 31.1 | 34.1 | **1.04** | 0.92 .. 1.13 |

- **Sparse: 88% of the u8 baseline against a 60% budget - pass.**
- Opaque, masked and clipped are all within noise of parity; two of the three are *faster*
  at 16 bits, which is expected: the loop is compute-bound there, and the wider channel does
  not add work per pixel.

### What was wrong with the earlier measurements

Both earlier results were artefacts, and both are kept in the record rather than quietly
re-run:

1. **-78% sparse** came from promoting the whole document inside every composite - it
   measured the conversion, not the compositing. A real 16-bit document holds 16-bit buffers.
2. **-39% sparse** came from the prototype's own loop, which was slower than the production
   one, so it overstated the width penalty. In production code the penalty is 12%.
3. **The +/-40% single-sample swing** came from taking one unpaired sample per case. Pairing
   inside the round removed it: the spread is now 0.91-1.08 on opaque instead of -33%..+12%.

### Memory (arithmetic, asserted)

| | u8 | Srgb16 |
| :--- | ---: | ---: |
| One 1080p layer | 7.9 MB | 15.8 MB |
| 4-layer document | 31.6 MB | 63.3 MB |
| History, per-stroke x20 | 316 MB | 633 MB |
| History, full-document x20 | 1.27 GB | **2.53 GB** |

### History: decided, and it changes the gate-2 conclusion

**Approved:** delta-based per-stroke snapshots become the primary history mechanism. No
full-document snapshots for ordinary operations; bounded compressed checkpoints where they
earn their keep; **exact undo/redo behaviour preserved**. That removes the 2.53 GB figure
from the 16-bit cost: history is then bounded by the size of what changed, not by the size
of the document, and it is a *better* design at 8 bits too.

This is production work in its own right, with its own gate: undoing and redoing a long edit
session must restore byte-identical state, and the existing tests must keep passing.

**Implemented, separately from any `LayerBuffer` work** (one change, one commit, so a
regression is attributable):

- `LayerPixelsDeltaCommand` stores the spans of a layer buffer that actually changed, with
  both states, and drops the full buffers at construction. Spans closer than 64 bytes are
  merged so a pathological edit cannot fragment into thousands of tiny ones; a length change
  (canvas resize) falls back to whole-buffer capture, which is the only exact option there.
- All ten call sites that captured whole layers - brush strokes, filters, fill, clear,
  stroke-selection, invert, grayscale, sepia - now push the delta command. Full-document
  snapshots remain for structural operations only.
- Exactness is tested, not assumed: `delta_replays_undo_and_redo_exactly`,
  `deltas_handle_scattered_edits_and_no_ops` (non-contiguous edits, a no-op that costs zero,
  and a length change), and `long_edit_session_undoes_and_redoes_to_identical_bytes` - 300
  strokes that must undo to the starting buffer, redo to the final buffer, and land on the
  exact intermediate buffer after a partial undo-then-redo.

**Measured** (`delta_history_retains_a_fraction_of_full_snapshots`, 256x256, 200 strokes):
retained history **0.18 MB against 100.00 MB - 546x smaller**. The arithmetic that matters
more is at 1080p: a snapshot step is 16.6 MB, so the 512 MiB default budget retains roughly
**30** undo steps however large `max_history` is; a delta step for an ordinary dab is under a
kilobyte, so the budget stops bounding undo depth at all.


## Gate 3 - Production architecture

Only after gates 1 and 2 pass. **Three constraints were set before implementation began:**

1. **History stays storage-format agnostic.** It was moved to byte deltas first, as a
   standalone milestone, so it stores spans without parsing them - no pixel size, stride or
   channel assumption. `deltas_do_not_assume_eight_bit_pixels` pins that.
2. **Existing u8 behaviour and the 702-case oracle stay protected.** None of this work may
   relax the oracle; the tests that compare against the pre-rewrite reference are the gate,
   not a convenience.
3. **AUR v1/v2 loading keeps working after AUR v3 exists.** A v3 writer must not make an old
   file unreadable, and the existing v1 test pins the oldest path.

### Sub-step 1: `LayerBuffer` - done

`Layer { buffer: LayerBuffer }` with a single variant, `Srgb8(Vec<u8>)`, replacing the bare
`Vec<u8>`. Every caller already went through `as_srgb8()`/`as_srgb8_mut()`, so this is a
change inside one file plus the enum: **90 tests pass unchanged, including the 702-case
oracle**, and no output moved.

What the type deliberately does not carry: no width or height (those belong to the layer and
describe the image, not the allocation), and no colour space (that belongs to the compositor).
Keeping those out is what lets the representation change without silently changing meaning.

The next sub-step adds `Srgb16` alongside it, which is the point where accessors become
fallible and conversion becomes explicit.

### Sub-step 2: `Srgb16` and fallible accessors - done

`LayerBuffer::Srgb16(Vec<u16>)` exists, gamma-encoded sRGB at 16 bits, and the accessors are
now the approved shape:

| Accessor | Behaviour |
| :--- | :--- |
| `as_srgb8() -> Option<&[u8]>` | `None` for other representations - the caller decides |
| `as_srgb8_mut() -> Option<&mut [u8]>` | same |
| `as_srgb8_exact()` / `as_srgb8_exact_mut()` | returns a view or **panics**; never converts, never allocates |
| `as_srgb16()` / `as_srgb16_mut()` | the 16-bit view |
| `read_storage_bytes` / `write_storage_bytes` | format-agnostic byte access, little-endian per channel |

The invariant behind `as_srgb8_exact()` is stated once, in its documentation, rather than
repeated 170 times: **nothing in the application creates a non-`Srgb8` layer yet** - new
layers, loaded projects and the fixture builders all produce `Srgb8`, and the only `Srgb16`
buffers in existence are the ones tests build deliberately. When AUR v3 can produce 16-bit
documents, every one of those call sites must be revisited, which is exactly why they are
named rather than hidden behind a blanket accessor.

Three constraints were met with tests rather than promises:

- **History stays format-agnostic** - and it is now genuinely so: the delta writes storage
  bytes through `write_storage_bytes`, so it never interprets a pixel. `deltas_round_trip_a_sixteen_bit_layer` records and replays an edit on a 16-bit layer byte-exactly.
- **Compositing blends at 16 bits** - a document holding any 16-bit layer runs the generic
  working-precision path on **real `u16` channels** (`Cow::Borrowed` from the layer, never a
  converted copy), with exact `v * 257` promotion for 8-bit layers and one dispatch per layer.
  Only the *result* is downconverted, because `composite_layers` returns an 8-bit buffer; a
  16-bit output API arrives with AUR v3.
  `sixteen_bit_compositing_keeps_detail_below_the_eight_bit_lattice` proves the point that a
  staged-to-8-bit design would fail: two values half an 8-bit step apart must stay distinct
  through the blend, while the 8-bit output necessarily collapses them.

  The earlier version of this sub-step converted a 16-bit layer to 8-bit once per composite
  and called that a boundary. It was a compatibility bridge that silently discarded the
  precision gate 1 exists to measure - two 16-bit values a half-step apart became one byte
  *before* the blend saw them. It has been removed rather than kept as a fast path.
- **The oracle is untouched**: 92 tests pass, including the 702 combinations.


1. `LayerBuffer` with `Srgb8` + `Srgb16`, accessors as specified in proposal section 2.
2. Conversion boundaries and **per-layer quantisation** (proposal section 4, corrected:
   whole-pass working precision would break byte-identity).
3. `AUR v3`: document-level format tag, per-layer byte reserved as `0`, unknown tags error.
4. AUR v1/v2 keep loading exactly as they do today.
5. Filters and rasterizer migrated incrementally, each with its own before/after oracle
   comparison.

### Explicitly out of scope

- Mixed-format documents (representable in v3, not supported in the editor).
- Linear-light compositing - its own proposal.
- A u16 fast path.

---

## Open decision for this plan

**Confirm `Srgb16` as the target and linear-light as a separate future proposal.** If the
intent was in fact linear-light storage, gate 1 cannot hold (the oracle becomes impossible)
and the whole shape changes: it becomes a compositing-semantics change with a user-visible
output change, rather than a storage change behind a safety net.

---

## Step 4: making the rest of the app format-aware

Approved order, each slice its own commit and its own tests:

1. **Brush rasterization and basic painting** - a 16-bit document you cannot paint on is not
   usable, so this comes before filters.
2. Erasing and fill.
3. Geometric transforms and resampling.
4. Individual filters, simplest first.
5. Selection-based operations and the remaining rasterizer paths.

### Slice 1, scoped

The whole brush pipeline - `paint_dot`, `paint_segment`, `paint_spline` - funnels through
`StrokeRasterizer::blend_stamp`, which is ~270 lines with **11 pixel-write sites** and takes
`pixels: &mut [u8]`. Genericising that one function over `Channel` makes every brush path
format-aware in a single change, which is why it is slice 1 rather than five separate ones.

Approach:

- `blend_stamp<C: Channel + PartialEq>`, with the per-pixel blend routed through
  `composite_working` (already the single source of the blend maths) so `u8` and `u16` run
  the same arithmetic.
- Call sites dispatch **once per stamp**, not per pixel: `match layer.buffer_mut()` at the
  entry point, exactly as the compositor does.
- The 8-bit path must stay byte-identical: the 702-case oracle and the existing brush tests
  are the gate, plus a benchmark check that painting throughput has not moved.
- New test: paint a dab on an `Srgb16` layer and assert (a) it does not panic, (b) the
  painted pixel is 16-bit, (c) an off-lattice value under 50% opacity keeps sub-8-bit
  detail - the same standard the compositor test holds.

Risk worth naming: eleven write sites is eleven chances to convert to `u8` "just here". A
review should be able to see one conversion pattern, not eleven variants.

Until this slice lands, painting on a 16-bit layer **panics** rather than silently painting at
8-bit. A loud, documented failure is the right behaviour for an unimplemented path, and it is
better than the precision loss a quiet fallback would cause.
