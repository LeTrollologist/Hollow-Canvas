# Project format (AUR)

AUR is Aurelia's binary project format. Saving always writes **v3**; the reader accepts v1 and v2
for compatibility and legacy Hollow Canvas `.hcv` v2–v5.

## Container

```text
magic            4 bytes   "AUR\x01" / "AUR\x02" / "AUR\x03"
width, height    u32 LE each
active_layer_id  u64 LE
background       u8
is_transparent   u8
theme            u8
document_format  u8        v3 only: 0 = sRGB8, 1 = sRGB16
layer_count      u32 LE
```

Each layer then carries: id, UTF-8 name, kind, adjustment/text config, parent id, flags,
opacity, blend mode, offsets, an optional v3 format tag, `comp_len`, a raw-deflate payload, and
an optional mask section. The preview thumbnail is a PNG appended after the layers and is never
required for loading.

## Versions

| Version | Adds | Reader behavior |
| :--- | :--- | :--- |
| v1 | Initial `.aur` container (HCV v5 body layout) | read-only |
| v2 | Per-layer 8-bit masks | read-only |
| v3 | Per-document and per-layer pixel-format tags; 16-bit layers | written |

## The document format tag

The `document_format` byte is **the document's storage format for new layers**, not a summary of
what the layers happen to hold:

- The writer emits `doc.pixel_format`. It used to derive the byte from "does any layer hold
  16-bit channels", which meant deleting the last 16-bit layer silently turned a 16-bit document
  into an 8-bit one, and the next new layer was created 8-bit.
- A layer's own tag may still differ from the document's (`0` = inherit, `1` = sRGB8,
  `2` = sRGB16). Mixed documents are therefore representable, loadable, and save unchanged; the
  editor simply does not create them.
- An unknown document tag is a hard error, never a fallback.

## Limits and validation

- `width * height * 8` must be ≤ 2 GiB; zero dimensions are rejected.
- A layer's deflate output is bounded to the exact expected payload (+1 byte probe); a
  high-ratio stream is rejected before it can allocate the expansion.
- Truncated or malformed files return `CorruptData`; the reader does not guess a format.
- 16-bit layers round-trip byte-exactly, and native 16-bit compositing output is available
  through `Document::composite_layers_srgb16` (#15).

## Round-trip policy

`load → save → load` must preserve layer order, names, masks, blend modes, offsets, adjustment
settings and 8/16-bit channel data. Tests under `crates/aurelia-io/src/tests.rs` enforce this for
the supported versions; a permanent fixture corpus for historical files is listed as follow-up
work in [RELEASE_PROCESS.md](../RELEASE_PROCESS.md).
