# Memory and undo

## Layer buffers

Each raster layer stores `width × height` pixels:

- 8-bit RGBA: 4 bytes/pixel
- 16-bit RGBA: 8 bytes/pixel

A document holds every layer buffer plus the current composite, the navigator thumbnail, undo
patches and the reference image. Canvas size and layer count dominate memory use.

The AUR format rejects any layer whose `width × height × 8` exceeds 2 GiB, but that is a format
cap, not a promise that a document of that size will run comfortably.

## Undo

Undo is **bounded**: history records pixel deltas/patches for editing operations and evicts the
oldest entries when the budget is reached. It is not an unlimited document snapshot stack.
Flattening, large filters and many-layer composites can each create sizable transient buffers.

## Autosave and recovery

- Autosave writes snapshots to `%LOCALAPPDATA%\Aurelia\autosave`.
- The interval is configurable via preferences; the default is 120 seconds.
- A crash leaves the snapshot, but there is no recovery browser in 3.1.0 — open the `.aur` file
  directly from that folder.

A recovery manager (browse/restore/delete snapshots) is a high-priority roadmap item.

## Practical guidance

For very large documents, close other projects, reduce undo depth, and flatten completed layers.
Measure with the benchmark commands in [PERFORMANCE.md](PERFORMANCE.md) rather than assuming a
fixed canvas limit.
