# Rendering

Aurelia uses a deterministic CPU renderer. There is no GPU/OpenGL path.

## Pipeline

1. Layers are composited in document order with the active blend mode and opacity.
2. Adjustment layers run as filter passes over the accumulated result.
3. Layer masks scale alpha during compositing; they do not mutate pixel data.
4. The visible canvas is transformed (pan/zoom) into an RGBA framebuffer.
5. `aurelia-app` blits the framebuffer with `SetDIBitsToDevice`; egui draws panels and overlays on
   top.

Performance-sensitive paths avoid per-frame allocations where practical, but “zero-allocation”
is an implementation goal for specific passes, not a global guarantee.

## Color

- Working data is 8-bit sRGB for the composite path; 16-bit layers are stored and round-tripped.
- There is **no ICC/Display P3 color management** in 3.1.0: profiles are not loaded, embedded or
  converted, and untagged images are treated as sRGB.
- Color management is the highest-priority rendering gap; see `ROADMAP.md`.

## Overlays and UI

Rulers, grids, symmetry guides, transform gizmos and perspective guides are drawn as overlays;
they do not modify the composited pixels. The Navigator thumbnail is a downscaled copy of the
same composite, not a second document render.

## Limits

- Compositing is CPU-bound by canvas area and layer count.
- Very large canvases are limited by available memory and the 2 GiB per-layer format cap.
- Native 16-bit compositing is not yet implemented (#15); 16-bit data is preserved on save/load.
