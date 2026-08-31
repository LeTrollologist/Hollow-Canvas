# Hollow Canvas · Graphics Studio

<div align="center">

<img src="assets/banner.svg" alt="Hollow Canvas Banner" width="100%" />

**A modern, high-performance, local-first digital painting and graphics studio built with 100% pure Rust.**

[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE.md)
[![Platform](https://img.shields.io/badge/platform-Windows-lightgrey.svg)](https://github.com/LeTrollologist/Hollow-Canvas)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

</div>

---

## 🌟 Overview

**Hollow Canvas** is a lightweight, studio-grade digital illustration and raster graphics workbench engineered from the ground up in Rust. Designed for artists, concept designers, and technical illustrators who demand absolute precision, low latency, and deterministic offline reliability.

### Key Highlights

- ⚡ **High-Frequency Painting Engine**: Cubic Catmull-Rom spline interpolation eliminates angular chord artifacts, ensuring fluid and continuous curves at any stroke speed.
- 🎨 **Versatile Toolset**: Includes Brush, Pencil, Watercolor (wet edge/falloff simulation), Chalk (grain dispersion), Spray (stochastic scatter), Smudge (directional velocity smear), Clone Stamp, Vector Shapes, Flood Fill with Reference-layer line art detection, and Marquee selection.
- 📐 **Symmetry Engine**: Real-time Horizontal, Vertical, Quad, and multi-segment Radial/Mandala symmetry.
- 🛡️ **Zero-Allocation Rendering**: Highly optimized software rendering pipeline with reusable composite buffers for silky smooth 60+ FPS viewport navigation and drawing.
- 🔒 **Local-First & Private**: Completely offline, zero telemetries, no trackers, and local project serialization via binary compressed archives (`.hcv`).

---

## 🏗️ Architecture

The workspace is organized into four modular, decoupled Rust crates:

```text
Hollow Canvas/
├── crates/
│   ├── hollow-core/      # Core data models, blend modes, symmetry, layer stack, and rasterizers
│   ├── hollow-render/    # Software raster renderer, texture caching, and UI primitive composition
│   ├── hollow-io/        # Binary HCV project serialization, PNG export, and file parsing
│   ├── hollow-ui/        # Egui-based dock panels, tools shelf, color palettes, and modals
│   └── hollow-app/       # Native Win32 desktop application orchestrator and message loop
```

### Module Responsibilities

1. **`hollow-core`**:
   - **Blend Modes**: Normal, Multiply, Screen, Overlay, Darken, Lighten, Color Dodge, Color Burn, Hard Light, Soft Light, Difference, Exclusion.
   - **Rasterizer**: High-precision `blend_stamp` rasterizer, Catmull-Rom spline curve generator, Bresenham shape algorithms, and flood fill with tolerance matching and reference layer awareness.
   - **History**: Linear and non-destructive undo/redo command stack.

2. **`hollow-render`**:
   - Software framebuffer rendering with zero heap allocation per-frame.
   - Nearest-neighbor and bilinear canvas viewport scaling with pan/zoom math.

3. **`hollow-io`**:
   - Non-blocking `.hcv` binary project saving and loading with header checksums.
   - PNG export with transparency channels.

4. **`hollow-ui`**:
   - Modular themeable interface (Deep Mist, Moonlit, Ember Glow).
   - Interactive docks: Tools, Symmetry, Brush Dynamics, Layers, Color History & Palette, Canvas Settings, Reference Image Viewer, and Help dialog.

5. **`hollow-app`**:
   - Low-latency native Win32 message loop processing mouse, tablet, and keyboard input.

---

## 🚀 Installation & Getting Started

Hollow Canvas can be installed as a portable native binary, via the **VPack Archiver** ecosystem, or compiled directly from source.

### Option 1: Install with VPack (Recommended)

[**VPack Archiver**](https://github.com/LeTrollologist/vpack-archiver) is the high-performance universal archive manager for `.vpack` packages.

1. Download [`hollow-canvas-v0.1.0-windows-x86_64.vpack`](https://github.com/LeTrollologist/Hollow-Canvas/releases/latest) from the latest release.
2. Extract the package with `vpack`:
   ```bash
   # Extract all files
   vpack extract hollow-canvas-v0.1.0-windows-x86_64.vpack

   # Or extract to a custom directory
   vpack extract hollow-canvas-v0.1.0-windows-x86_64.vpack -o ./HollowCanvas/
   ```
3. *(Optional)* Verify CRC-32 integrity:
   ```bash
   vpack test hollow-canvas-v0.1.0-windows-x86_64.vpack
   ```
4. Run `hollow-canvas.exe`.

---

### Option 2: Install with Native Portable Zip

No additional archive tools required — works with standard Windows extraction:

1. Download [`hollow-canvas-v0.1.0-windows-x86_64.zip`](https://github.com/LeTrollologist/Hollow-Canvas/releases/latest) from the latest release.
2. Extract the zip file using PowerShell or Windows Explorer:
   ```powershell
   Expand-Archive -Path .\hollow-canvas-v0.1.0-windows-x86_64.zip -DestinationPath .\HollowCanvas
   ```
3. Double-click `HollowCanvas\hollow-canvas.exe` to launch immediately.

---

### Option 3: Compile from Source (Pure Rust)

If you prefer building from source with full compiler optimizations:

**Prerequisites:**
- [Rust Toolchain](https://www.rust-lang.org/tools/install) (1.75+ recommended)
- Windows 10 / 11 (64-bit)

1. Clone the repository:
   ```bash
   git clone https://github.com/LeTrollologist/Hollow-Canvas.git
   cd Hollow-Canvas
   ```

2. Run the test suite:
   ```bash
   cargo test --workspace
   ```

3. Build and launch:
   ```bash
   cargo run --release -p hollow-app
   ```

---

## ⌨️ Keyboard Shortcuts & Controls

| Action | Shortcut |
| :--- | :--- |
| **Brush Tool** | <kbd>B</kbd> |
| **Eraser Tool** | <kbd>E</kbd> |
| **Flood Fill** | <kbd>G</kbd> |
| **Eyedropper (Pick)** | <kbd>I</kbd> |
| **Marquee Select** | <kbd>M</kbd> |
| **Move / Pan View** | <kbd>V</kbd> (Drag) |
| **Translate Layer Content** | <kbd>Ctrl</kbd> + <kbd>V</kbd> (Drag) |
| **Viewport Pan** | <kbd>Space</kbd> + Drag or Middle Mouse Drag |
| **Viewport Zoom** | <kbd>Mouse Wheel</kbd> |
| **Swap Primary/Secondary Color** | <kbd>X</kbd> |
| **Set Clone Source** | <kbd>Alt</kbd> + Click (with Clone tool) |
| **Commit Polygon / Crop** | <kbd>Enter</kbd> |
| **Cancel Polygon / Selection** | <kbd>Esc</kbd> or <kbd>Ctrl</kbd> + <kbd>D</kbd> |
| **Undo** | <kbd>Ctrl</kbd> + <kbd>Z</kbd> |
| **Redo** | <kbd>Ctrl</kbd> + <kbd>Y</kbd> or <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Z</kbd> |
| **Save Project** | <kbd>Ctrl</kbd> + <kbd>S</kbd> |
| **Export PNG** | <kbd>Ctrl</kbd> + <kbd>E</kbd> |
| **Open Project** | <kbd>Ctrl</kbd> + <kbd>O</kbd> |

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit issues, feature requests, or pull requests.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## 📄 License

This project is licensed under the terms of the **GNU General Public License v3.0** (GPL-3.0). See [LICENSE.md](LICENSE.md) for details.

---

<div align="center">
  <sub>Crafted with passion by <a href="https://github.com/LeTrollologist">LeTrollologist</a>.</sub>
</div>
