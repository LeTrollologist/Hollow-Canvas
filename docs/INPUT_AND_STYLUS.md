# Input and stylus

## Pointer path

- The Win32 host receives `WM_POINTER*` messages for pen and touch, with a mouse fallback.
- Pressure is sampled per pointer event and passed into the stroke rasterizer.
- Tilt is sampled where the device reports it; tilt-aware width is opt-in (0 = off).
- Pan/zoom and transform handles consume pointer input before the canvas.

## DPI and coordinates

The process opts into per-monitor V2 DPI awareness. egui works in points; canvas geometry is in
physical pixels. Conversion happens at the `pixels_per_point` boundary, so strokes and rulers
stay aligned at 100–200 % scaling.

## Current tablet boundary

- **WM_POINTER** is the supported path on Windows 10/11.
- **WinTab is not implemented.** Devices that only expose WinTab will behave like a mouse.
- Pressure/tilt mapping is device-dependent; there is no per-device calibration UI.
- Pen cancellation and disconnect behavior is handled as a stroke end, but broad hardware
  testing (Wacom, Huion, XP-Pen, Surface Pen) is outstanding.

## Keyboard

Shortcuts are defined in `aurelia-ui` and documented in the README. User-defined remapping and
conflict detection are not yet implemented.
