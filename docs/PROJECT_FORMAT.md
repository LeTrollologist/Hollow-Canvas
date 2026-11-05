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

## Limits and validation

- `width * height * 8` must be ≤ 2 GiB; zero dimensions are rejected.
- A layer's deflate output is bounded to the exact expected payload (+1 byte probe); a
  high-ratio stream is rejected before it can allocate the expansion.
- Truncated or malformed files return `CorruptData`; the reader does not guess a format.
- 16-bit layers round-trip byte-exactly. Native 16-bit compositing is tracked separately (#15).

## Round-trip policy

`load → save → load` must preserve layer order, names, masks, blend modes, offsets, adjustment
settings and 8/16-bit channel data. Tests under `crates/aurelia-io/src/tests.rs` enforce this for
the supported versions; a permanent fixture corpus for historical files is listed as follow-up
work in [RELEASE_PROCESS.md](../RELEASE_PROCESS.md).
