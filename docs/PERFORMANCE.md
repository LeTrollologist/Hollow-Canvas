# Performance

Aurelia is CPU-rendered; there is no GPU fallback. Numbers are machine-dependent and should be
reproduced locally before they are quoted as guarantees.

## Recorded baseline

From the development machine used for the 3.0.x/3.1.0 work:

| Workload | Throughput |
| :--- | :--- |
| 4-layer 1080p composite | ~113 MPix/s |
| Same, with layer masks | ~92 MPix/s |

These are composite throughput figures, not end-to-end frame rates. UI frame timing also
includes egui layout, panel painting and the GDI blit.

## 16-bit completion baseline

Measured at `checkpoint/v3.4.0-16bit-complete` (`bfa6709`) on Intel64 Family 6 Model 154
Stepping 3, rustc 1.93.1, release profile, five paired rounds. This is the stabilization
baseline, not a new target:

| Workload | Throughput |
| :--- | ---: |
| General-path composite, 4-layer 1080p, opaque | **105.4 MPix/s** |
| Slowest blend mode (Luminosity) | 82.5 MPix/s |
| Plain stack (8-bit fast path) | 163.7 MPix/s |
| Masked stack | 133.2 MPix/s |
| Brush, `Srgb8` | 1208 operations/s |
| Brush, `Srgb16` | 1263 operations/s (+5%) |
| Gate 2 sparse ratio (`Srgb16` / u8) | **0.67** (u8 175.8, `Srgb16` 113.8) |

The older ~113 / ~92 MPix/s figures are the fast-path composite baseline; the 105.4 general-path
number is the like-for-like path a 16-bit document uses. The gate-2 budget for sparse is 60% of
u8, so 67% passes with limited headroom — sparse is the first performance item in the
stabilization phase.

## Reproduce

```powershell
cargo run --release -p aurelia-render --example bench_composite
cargo run --release -p aurelia-render --example bench_brush
cargo test -p aurelia-core gate_2 --release -- --ignored --nocapture
```

Run on the same machine and power profile; laptop power saving, background load and debug builds
change results substantially.

## What is not claimed

- “60+ FPS” is a target for typical editing, not a guarantee for every canvas/layer combination.
- “Zero allocation” applies to selected composite/brush passes, not the whole application.
- Large documents are memory-bound; see [MEMORY_AND_UNDO.md](MEMORY_AND_UNDO.md).
