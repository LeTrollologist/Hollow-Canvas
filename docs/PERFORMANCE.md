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

## Reproduce

```powershell
cargo run --release -p aurelia-render --example bench_composite
cargo run --release -p aurelia-render --example bench_brush
```

Run on the same machine and power profile; laptop power saving, background load and debug builds
change results substantially.

## What is not claimed

- “60+ FPS” is a target for typical editing, not a guarantee for every canvas/layer combination.
- “Zero allocation” applies to selected composite/brush passes, not the whole application.
- Large documents are memory-bound; see [MEMORY_AND_UNDO.md](MEMORY_AND_UNDO.md).
