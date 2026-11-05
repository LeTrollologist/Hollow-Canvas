# 16-bit presentation: spike decision

**Status:** deferred with a review gate. Not adopted in v3.4.x.
**Decision date:** 2026-11-05
**Review at:** next major-version planning (v3.5); revisit earlier only if a 10-bit/HDR output
proposal exists.

## The question

Can the viewport present an RGBA16 document without downconverting it to 8 bits for display?

## What the current path actually is

```text
Document (Srgb8 or Srgb16)
  -> compositor (u8 fast path, or generic working-precision path)
  -> 8-bit RGBA framebuffer (`Vec<u32>`)
  -> GDI SetDIBitsToDevice
  -> Windows desktop surface
```

The viewport is **not** an egui texture. It is a software framebuffer blitted by GDI. The
Navigator is an egui texture, but it is a 320 px preview, not the editing surface.

## Findings

1. **GDI is 8 bits per channel.** `SetDIBitsToDevice` consumes a 32bpp BGRA DIB; the visual
   result is an 8-bit presentation. A 16-bit source is downconverted before the blit.
2. **egui 0.28 textures are RGBA8.** `ColorImage` and the managed texture path are 8-bit. A
   16-bit texture would need a custom paint callback and a GPU backend, not another
   `load_texture` call.
3. **Aurelia has no GPU host.** There is no `wgpu`, `egui-wgpu` or DXGI swapchain in the
   dependency graph. Adding one is not a texture-format change; it replaces the presentation
   host that the Win32 window, input path and GDI blit are built around.
4. **The desktop is still 8-bit unless 10-bit/HDR is requested.** Even a perfect RGBA16
   texture is quantised by an 8-bit desktop surface. Visible benefit requires a 10-bit swapchain
   and a color-managed output path, not 16-bit storage alone.
5. **The user-visible problem is banding and edge quality, not missing 16-bit file storage.**
   Those are addressed by 10-bit/HDR presentation, dithering at the 8-bit boundary, and the
   bilinear edge fix - each with its own visual baseline.

## Decision

- **Do not fork egui.**
- **Do not add a GPU presentation host in v3.4.x.**
- **Defer 16-bit presentation** until all of the following exist:
  1. P1 stabilization and performance work is complete.
  2. A 10-bit/HDR output proposal names the swapchain format, color space, fallback behaviour
     and the machines it can actually be tested on.
  3. The color-management work (ICC profile, working-space conversion) has a baseline.
  4. A measured banding case demonstrates a difference that 8-bit presentation cannot show.
- Revisit at the v3.5 planning gate. A later custom texture/renderer layer is allowed only as a
  measured spike that keeps the existing viewport tests green and does not replace the UI toolkit.

## Why this is a real decision, not a punt

The bounded experiment ("upload RGBA16 and look at it") cannot be run meaningfully through the
current GDI path: the framebuffer, the blit and the desktop surface are all 8-bit. The first
honest prototype is a GPU swapchain prototype with a 10-bit format, which is a host-level
project, not a viewport patch. Spending that now would delay the viewport regression work and
the unsigned-installer problem for a benefit the current output cannot display.
