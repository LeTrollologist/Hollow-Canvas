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

- Working data follows the document: 8-bit documents composite on the integer fast path, while
  16-bit documents use the generic working-precision path and are downconverted only at the
  8-bit presentation boundary.
- There is **no ICC/Display P3 color management** yet: profiles are not loaded, embedded or
  converted, and untagged images are treated as sRGB.
- Color management is the highest-priority rendering gap; see `ROADMAP.md`.

## Overlays and UI

Rulers, grids, symmetry guides, transform gizmos and perspective guides are drawn as overlays;
they do not modify the composited pixels. The Navigator thumbnail is a downscaled copy of the
same composite, not a second document render.

## Limits

- Compositing is CPU-bound by canvas area and layer count.
- Very large canvases are limited by available memory and the 2 GiB per-layer format cap.
- 16-bit compositing, editing and transforms are native; only the viewport presentation is
  8-bit, because `egui` 0.28 textures are 8-bit.
