# Compatibility

## Platform

| Item | Supported |
| :--- | :--- |
| OS | Windows 10 / 11, x86-64 |
| Display | 100–200 % scaling, per-monitor V2 DPI |
| GPU | Not required; software renderer |
| Linux/macOS | Not supported in Windows-targeted releases |

## Formats

| Format | Read | Write | Notes |
| :--- | :--- | :--- | :--- |
| AUR v3 | ✅ | ✅ | current format |
| AUR v2 | ✅ | — | masks supported; saves migrate to v3 |
| AUR v1 / HCV v2–v5 | ✅ | — | legacy compatibility |
| PNG / JPEG / WebP | ✅ | ✅ | 8-bit export |
| GIF / sprite sheet / frame sequence | ✅ (import) | ✅ | animation timeline export |
| ICC / Display P3 | ❌ | ❌ | no color-management workflow yet |

## Application features

| Area | Limits |
| :--- | :--- |
| Layers | raster, groups, clipping masks, adjustment layers, text |
| Pixel formats | native 8-bit and 16-bit sRGB editing, compositing and transforms; the viewport presents an 8-bit image, and mixed-format files load but are not created |
| Animation | timeline + GIF/sprite/frame export; no video container |
| Reference images | single dock; not persisted as a library |
| Workspace | panel toggles persisted; no named workspace presets |
| Brushes | built-in presets; no import/export library |
| Discord presence | opt-in, generic activity, optional project name, no artwork previews |
| Accessibility | keyboard shortcuts; screen-reader and contrast work outstanding |

## Installer and updates

The installer is per-user, requires no administrator rights, and writes to
`%LOCALAPPDATA%\Programs\Aurelia`. There is no auto-update; releases are installed manually.
The installer is not Authenticode-signed.
