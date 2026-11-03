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

## Gate 3 - Production architecture

Only after gates 1 and 2 pass:

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
