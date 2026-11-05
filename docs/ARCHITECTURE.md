# Architecture

Aurelia is a Windows-native desktop application. Rust owns the domain model, rasterization and
UI logic; a thin Win32/GDI host owns the window, message loop and framebuffer blit.

## Crates

| Crate | Responsibility | Platform |
| :--- | :--- | :--- |
| `aurelia-core` | Document/layer model, brush and stroke rasterization, selection, history, blend modes, color | portable |
| `aurelia-render` | CPU compositor, software framebuffer, viewport transforms, overlays | portable |
| `aurelia-io` | AUR v3 serialization, legacy HCV reading, PNG/JPEG/WebP/GIF export | portable |
| `aurelia-ui` | egui panels, dialogs, tools, state and preferences | portable |
| `aurelia-presence` | Optional Discord Rich Presence over local named-pipe IPC | Windows |
| `aurelia-app` | Win32 message loop, GDI blit, stylus messages, CLI, autosave ticker | Windows |

## Frame path

1. The Win32 host pumps `GetMessageW` and forwards input to `aurelia-app`.
2. `aurelia-ui` lays out panels and tools through egui.
3. `aurelia-render` composites the visible canvas into a software RGBA framebuffer.
4. The host blits that framebuffer with `SetDIBitsToDevice`; egui draws its panels on top.

There is no GPU rendering path. The renderer is deterministic and CPU-bound by design.

## Native boundary

Safe Rust covers the core, renderer, I/O and UI logic. The following are outside Rust's
memory-safety guarantees and are reviewed as `unsafe`/FFI boundaries:

- Win32, GDI, COM and shell calls in `aurelia-app` and `rfd`;
- the MSVC C runtime linked by the official Windows build;
- third-party decoders used for imported images.

See [SECURITY_MODEL.md](SECURITY_MODEL.md) for the threat model.

## Release tooling

`tools/release` is Python and is not part of the shipped binary. It never runs in CI; it is a
local orchestration layer over git, the build, signing and the GitHub API.
