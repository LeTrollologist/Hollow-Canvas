use crate::document::Document;
use crate::layer::Layer;

pub trait Command: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &'static str;
    /// Approximate retained heap footprint in bytes, used by the undo stack's memory budget.
    fn cost(&self) -> usize {
        0
    }
    fn undo(&mut self, doc: &mut Document);
    fn redo(&mut self, doc: &mut Document);
}

/// Whole-buffer snapshot of one layer.
///
/// **Not the path ordinary edits take any more** - `LayerPixelsDeltaCommand` replaced it,
/// because retaining two full buffers per step bounds undo depth by canvas size: at 1080p a
/// step is 16.6 MB, so the 512 MiB default budget keeps roughly 30 steps whatever
/// `max_history` says. This type is kept as the reference the delta is measured against, and
/// as a fallback for callers that genuinely need whole-buffer capture.
#[derive(Debug)]
pub struct LayerPixelsSnapshotCommand {
    pub layer_id: u64,
    pub description: &'static str,
    pub before_pixels: Vec<u8>,
    pub after_pixels: Vec<u8>,
}

impl Command for LayerPixelsSnapshotCommand {
    fn name(&self) -> &'static str {
        self.description
    }

    fn cost(&self) -> usize {
        self.before_pixels.len() + self.after_pixels.len()
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.get_layer_mut(self.layer_id) {
            if layer.storage_len() == self.before_pixels.len() {
                // Storage bytes, not 8-bit channels: the snapshot is format-agnostic like the
                // delta command, so a 16-bit layer restores exactly.
                layer.write_storage_bytes(0, &self.before_pixels);
            }
        }
    }

    fn redo(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.get_layer_mut(self.layer_id) {
            if layer.storage_len() == self.after_pixels.len() {
                layer.write_storage_bytes(0, &self.after_pixels);
            }
        }
    }
}

/// One contiguous byte range that an edit changed, with both of its states.
///
/// Stored as bytes rather than pixels so a delta is exact for any buffer layout: there is no
/// assumption here about channels, strides or colour encoding.
#[derive(Debug)]
struct DeltaSpan {
    offset: usize,
    before: Vec<u8>,
    after: Vec<u8>,
}

/// The changed parts of one layer buffer.
#[derive(Debug)]
enum PixelDelta {
    /// Only these spans differ; every other byte is identical.
    Spans(Vec<DeltaSpan>),
    /// The buffers differ in length - a canvas resize, not an ordinary edit. Keeping both
    /// whole buffers is the only way to restore a length change exactly.
    Whole { before: Vec<u8>, after: Vec<u8> },
}

/// Changed runs closer together than this are stored as one span, unchanged bytes and all.
///
/// A brush stroke is contiguous, so this rarely matters; it exists so a pathological edit
/// (every other pixel) cannot explode into thousands of tiny spans with per-span overhead.
const SPAN_MERGE_GAP: usize = 64;

impl PixelDelta {
    fn between(before: Vec<u8>, after: Vec<u8>) -> Self {
        if before.len() != after.len() {
            return PixelDelta::Whole { before, after };
        }
        let len = before.len();
        let mut spans: Vec<DeltaSpan> = Vec::new();
        let mut index = 0usize;
        while index < len {
            if before[index] == after[index] {
                index += 1;
                continue;
            }
            let start = index;
            let mut end = index + 1;
            let mut gap = 0usize;
            while index < len {
                if before[index] != after[index] {
                    end = index + 1;
                    gap = 0;
                } else {
                    gap += 1;
                    if gap > SPAN_MERGE_GAP {
                        break;
                    }
                }
                index += 1;
            }
            spans.push(DeltaSpan {
                offset: start,
                before: before[start..end].to_vec(),
                after: after[start..end].to_vec(),
            });
            index = end;
        }
        PixelDelta::Spans(spans)
    }

    /// Replays the delta into `layer` at the storage-byte level.
    ///
    /// Deliberately format-agnostic: it asks the layer to write storage bytes, so a delta
    /// works the same whether the layer holds 8-bit or 16-bit channels and never interprets
    /// a pixel itself.
    fn apply(&self, layer: &mut Layer, use_after: bool) {
        match self {
            PixelDelta::Spans(spans) => {
                for span in spans {
                    let source = if use_after { &span.after } else { &span.before };
                    layer.write_storage_bytes(span.offset, source);
                }
            }
            PixelDelta::Whole { before, after } => {
                let source = if use_after { after } else { before };
                if layer.storage_len() == source.len() {
                    layer.write_storage_bytes(0, source);
                }
            }
        }
    }

    fn retained_bytes(&self) -> usize {
        match self {
            PixelDelta::Spans(spans) => spans
                .iter()
                .map(|span| span.before.len() + span.after.len() + std::mem::size_of::<DeltaSpan>())
                .sum(),
            PixelDelta::Whole { before, after } => before.len() + after.len(),
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, PixelDelta::Spans(spans) if spans.is_empty())
    }
}

/// A brush edit to a layer mask (#35).
///
/// Masks are one byte per pixel, so a whole-buffer snapshot is cheap enough to be the
/// documented unit of undo: there are no colour channels to diff and the mask is never
/// reinterpreted. `enabled` travels with the pixels because undo must restore the state the
/// user saw, not just the bytes.
#[derive(Debug)]
pub struct MaskPixelsCommand {
    pub layer_id: u64,
    pub description: &'static str,
    pub before_pixels: Vec<u8>,
    pub after_pixels: Vec<u8>,
    pub before_enabled: bool,
    pub after_enabled: bool,
}

impl Command for MaskPixelsCommand {
    fn name(&self) -> &'static str {
        self.description
    }

    fn cost(&self) -> usize {
        self.before_pixels.len() + self.after_pixels.len()
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some(mask) = doc
            .get_layer_mut(self.layer_id)
            .and_then(|layer| layer.mask.as_mut())
        {
            if mask.pixels.len() == self.before_pixels.len() {
                mask.pixels.copy_from_slice(&self.before_pixels);
                mask.enabled = self.before_enabled;
            }
        }
    }

    fn redo(&mut self, doc: &mut Document) {
        if let Some(mask) = doc
            .get_layer_mut(self.layer_id)
            .and_then(|layer| layer.mask.as_mut())
        {
            if mask.pixels.len() == self.after_pixels.len() {
                mask.pixels.copy_from_slice(&self.after_pixels);
                mask.enabled = self.after_enabled;
            }
        }
    }
}

/// A layer edit stored as the regions that actually changed.///
/// This is the command ordinary edits push. The buffers are consumed at construction and
/// dropped: what survives is the diff, whose size follows the edit rather than the canvas.
/// Undo and redo replay the stored spans, so the result is byte-identical to the buffers the
/// edit started and ended with.
#[derive(Debug)]
pub struct LayerPixelsDeltaCommand {
    pub layer_id: u64,
    pub description: &'static str,
    delta: PixelDelta,
}

impl LayerPixelsDeltaCommand {
    /// Builds the command from the buffer as it was and as it is now.
    ///
    /// An edit that changed nothing retains nothing - `is_empty` reports that, so callers can
    /// skip pushing a no-op step.
    pub fn new(layer_id: u64, description: &'static str, before: Vec<u8>, after: Vec<u8>) -> Self {
        Self {
            layer_id,
            description,
            delta: PixelDelta::between(before, after),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.delta.is_empty()
    }
}

impl Command for LayerPixelsDeltaCommand {
    fn name(&self) -> &'static str {
        self.description
    }

    fn cost(&self) -> usize {
        self.delta.retained_bytes()
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.get_layer_mut(self.layer_id) {
            self.delta.apply(layer, false);
        }
    }

    fn redo(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.get_layer_mut(self.layer_id) {
            self.delta.apply(layer, true);
        }
    }
}

#[derive(Debug)]
pub struct AddLayerCommand {
    pub layer: Layer,
    pub insert_index: usize,
}

impl Command for AddLayerCommand {
    fn name(&self) -> &'static str {
        "Add Layer"
    }

    fn undo(&mut self, doc: &mut Document) {
        doc.delete_layer(self.layer.id);
    }

    fn redo(&mut self, doc: &mut Document) {
        doc.layers.insert(self.insert_index.min(doc.layers.len()), self.layer.clone());
        doc.active_layer_id = self.layer.id;
    }
}

#[derive(Debug)]
pub struct DeleteLayerCommand {
    pub layer: Layer,
    pub previous_index: usize,
}

impl Command for DeleteLayerCommand {
    fn name(&self) -> &'static str {
        "Delete Layer"
    }

    fn undo(&mut self, doc: &mut Document) {
        doc.layers.insert(self.previous_index.min(doc.layers.len()), self.layer.clone());
        doc.active_layer_id = self.layer.id;
    }

    fn redo(&mut self, doc: &mut Document) {
        doc.delete_layer(self.layer.id);
    }
}

/// Moves one layer to a new position in the document stack. The IDs and pixels are
/// unchanged; only ordering moves, so undo/redo is an exact index swap.
#[derive(Debug)]
pub struct MoveLayerCommand {
    pub layer_id: u64,
    pub from_index: usize,
    pub to_index: usize,
}

impl Command for MoveLayerCommand {
    fn name(&self) -> &'static str {
        "Reorder Layer"
    }

    fn undo(&mut self, doc: &mut Document) {
        move_layer_to(doc, self.layer_id, self.from_index);
    }

    fn redo(&mut self, doc: &mut Document) {
        move_layer_to(doc, self.layer_id, self.to_index);
    }
}

fn move_layer_to(doc: &mut Document, layer_id: u64, target: usize) {
    let Some(current) = doc.layers.iter().position(|layer| layer.id == layer_id) else {
        return;
    };
    let target = target.min(doc.layers.len().saturating_sub(1));
    if current == target {
        return;
    }
    let layer = doc.layers.remove(current);
    doc.layers.insert(target, layer);
}

#[derive(Debug)]
pub struct TranslateLayerCommand {
    pub layer_id: u64,
    pub before_offset: (i32, i32),
    pub after_offset: (i32, i32),
}

impl Command for TranslateLayerCommand {
    fn name(&self) -> &'static str {
        "Move Layer"
    }

    fn undo(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.get_layer_mut(self.layer_id) {
            layer.offset_x = self.before_offset.0;
            layer.offset_y = self.before_offset.1;
        }
    }

    fn redo(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.get_layer_mut(self.layer_id) {
            layer.offset_x = self.after_offset.0;
            layer.offset_y = self.after_offset.1;
        }
    }
}

#[derive(Debug)]
pub struct FullDocumentSnapshotCommand {
    pub description: &'static str,
    pub before_layers: Vec<Layer>,
    pub after_layers: Vec<Layer>,
    pub before_active_id: u64,
    pub after_active_id: u64,
}

impl Command for FullDocumentSnapshotCommand {
    fn name(&self) -> &'static str {
        self.description
    }

    fn cost(&self) -> usize {
        // `storage_len`, not the 8-bit view: this runs on push, and a 16-bit layer would make
        // `as_srgb8_exact()` panic before the step was ever recorded.
        let sum = |layers: &[Layer]| -> usize { layers.iter().map(|l| l.storage_len()).sum() };
        sum(&self.before_layers) + sum(&self.after_layers)
    }

    fn undo(&mut self, doc: &mut Document) {
        doc.layers = self.before_layers.clone();
        doc.active_layer_id = self.before_active_id;
    }

    fn redo(&mut self, doc: &mut Document) {
        doc.layers = self.after_layers.clone();
        doc.active_layer_id = self.after_active_id;
    }
}

/// One retained step, as the read-only History panel sees it.
///
/// The label is the same string undo/redo report in the status bar, so the panel and the
/// command that produced it cannot disagree. `bytes` is the command's own accounted cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryEntry {
    pub label: &'static str,
    pub bytes: usize,
}

pub struct HistoryStack {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    max_history: usize,
    max_bytes: usize,
    undo_bytes: usize,
    redo_bytes: usize,
    generation: u64,
}

impl HistoryStack {
    /// Default retained-memory ceiling for the undo history (512 MiB).
    pub const DEFAULT_MAX_BYTES: usize = 512 * 1024 * 1024;

    pub fn new(max_history: usize) -> Self {
        Self::with_budget(max_history, Self::DEFAULT_MAX_BYTES)
    }

    /// History bounded by both step count and retained bytes.
    ///
    /// Full-layer RGBA snapshots are tens of megabytes each, so a step-count-only
    /// cap can balloon the process into gigabytes. Oldest steps are evicted first;
    /// the newest step is always kept so a single large edit stays undoable.
    pub fn with_budget(max_history: usize, max_bytes: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: max_history.max(4),
            max_bytes: max_bytes.max(1),
            undo_bytes: 0,
            redo_bytes: 0,
            generation: 0,
        }
    }

    pub fn push(&mut self, cmd: Box<dyn Command>) {
        self.generation = self.generation.saturating_add(1);
        self.undo_bytes = self.undo_bytes.saturating_add(cmd.cost());
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
        self.redo_bytes = 0;
        self.evict_to_budget();
    }

    /// Drops the oldest steps until both the count and the byte budget fit.
    ///
    /// The newest step is always kept, so a single edit larger than the whole budget stays
    /// undoable rather than being silently discarded.
    fn evict_to_budget(&mut self) {
        while self.undo_stack.len() > 1
            && (self.undo_stack.len() > self.max_history || self.undo_bytes > self.max_bytes)
        {
            let dropped = self.undo_stack.remove(0);
            self.undo_bytes = self.undo_bytes.saturating_sub(dropped.cost());
        }
    }

    pub fn undo(&mut self, doc: &mut Document) -> Option<&'static str> {
        let mut cmd = self.undo_stack.pop()?;
        self.generation = self.generation.saturating_add(1);
        let cost = cmd.cost();
        self.undo_bytes = self.undo_bytes.saturating_sub(cost);
        cmd.undo(doc);
        let name = cmd.name();
        self.redo_bytes = self.redo_bytes.saturating_add(cost);
        self.redo_stack.push(cmd);
        Some(name)
    }

    pub fn redo(&mut self, doc: &mut Document) -> Option<&'static str> {
        let mut cmd = self.redo_stack.pop()?;
        self.generation = self.generation.saturating_add(1);
        let cost = cmd.cost();
        self.redo_bytes = self.redo_bytes.saturating_sub(cost);
        self.undo_bytes = self.undo_bytes.saturating_add(cost);
        cmd.redo(doc);
        let name = cmd.name();
        self.undo_stack.push(cmd);
        Some(name)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Steps currently retained in the undo stack.
    pub fn len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.undo_stack.is_empty()
    }

    /// Approximate bytes retained by undoable commands.
    pub fn retained_bytes(&self) -> usize {
        self.undo_bytes
    }

    /// Bytes retained by steps that are currently redoable.
    pub fn redo_retained_bytes(&self) -> usize {
        self.redo_bytes
    }

    /// Total bytes retained by the history, undoable and redoable alike.
    pub fn total_retained_bytes(&self) -> usize {
        self.undo_bytes.saturating_add(self.redo_bytes)
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn max_history(&self) -> usize {
        self.max_history
    }

    /// Changes the retained-memory ceiling and evicts immediately if the new budget is
    /// smaller. Used by the Preferences dialog; the newest step always survives.
    pub fn set_max_bytes(&mut self, max_bytes: usize) {
        self.max_bytes = max_bytes.max(1);
        self.evict_to_budget();
    }

    /// Changes the step-count ceiling and evicts immediately if the new limit is smaller.
    pub fn set_max_history(&mut self, max_history: usize) {
        self.max_history = max_history.max(1);
        self.evict_to_budget();
    }

    /// Retained undoable steps, oldest first. Read-only: the panel never mutates history.
    pub fn undo_entries(&self) -> Vec<HistoryEntry> {
        self.undo_stack
            .iter()
            .map(|cmd| HistoryEntry {
                label: cmd.name(),
                bytes: cmd.cost(),
            })
            .collect()
    }

    /// Retained redoable steps, nearest redo first (the order redo would replay them in).
    pub fn redo_entries(&self) -> Vec<HistoryEntry> {
        self.redo_stack
            .iter()
            .rev()
            .map(|cmd| HistoryEntry {
                label: cmd.name(),
                bytes: cmd.cost(),
            })
            .collect()
    }

    /// Monotonic edit counter. Every push/undo/redo bumps it, so callers can
    /// compare against a "saved" snapshot to know whether the document is dirty.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.undo_bytes = 0;
        self.redo_bytes = 0;
        self.generation = self.generation.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, FullDocumentSnapshotCommand, HistoryStack, LayerPixelsDeltaCommand, LayerPixelsSnapshotCommand};
    use crate::document::Document;

    fn pixels_of(doc: &Document, layer_id: u64) -> Vec<u8> {
        doc.get_layer(layer_id)
            .expect("layer")
            .as_srgb8_exact()
            .to_vec()
    }

    /// Paints a solid square - the shape of an ordinary brush dab.
    fn paint_square(doc: &mut Document, layer_id: u64, x: u32, y: u32, size: u32, value: u8) {
        let width = doc.width as usize;
        if let Some(layer) = doc.get_layer_mut(layer_id) {
            for dy in 0..size as usize {
                for dx in 0..size as usize {
                    let at = ((y as usize + dy) * width + (x as usize + dx)) * 4;
                    let pixels = layer.as_srgb8_exact_mut();
                    if at + 3 < pixels.len() {
                        pixels[at] = value;
                        pixels[at + 1] = value;
                        pixels[at + 2] = value;
                        pixels[at + 3] = 255;
                    }
                }
            }
        }
    }

    /// A delta must restore exactly the bytes the edit started and ended with - not
    /// approximately, and not just the pixels the edit was supposed to touch.
    #[test]
    fn delta_replays_undo_and_redo_exactly() {
        let mut doc = Document::new(16, 16);
        let layer_id = doc.active_layer_id;
        let before = pixels_of(&doc, layer_id);

        paint_square(&mut doc, layer_id, 3, 4, 5, 91);
        let after = pixels_of(&doc, layer_id);
        assert_ne!(before, after, "the fixture must actually change pixels");

        let mut command = LayerPixelsDeltaCommand::new(layer_id, "Dab", before.clone(), after.clone());
        command.undo(&mut doc);
        assert_eq!(pixels_of(&doc, layer_id), before, "undo must restore every byte");
        command.redo(&mut doc);
        assert_eq!(pixels_of(&doc, layer_id), after, "redo must restore every byte");
        assert!(command.cost() < before.len(), "a small dab must not cost a whole buffer");
    }

    /// Scattered edits, a length change, and an edit that changes nothing - the three cases
    /// the diff has to get right.
    #[test]
    fn deltas_handle_scattered_edits_and_no_ops() {
        let mut doc = Document::new(32, 32);
        let layer_id = doc.active_layer_id;
        let start = pixels_of(&doc, layer_id);

        // Every fourth pixel, so the diff cannot assume contiguity.
        if let Some(layer) = doc.get_layer_mut(layer_id) {
            let pixels = layer.as_srgb8_exact_mut();
            for index in (0..pixels.len() / 4).step_by(4) {
                pixels[index * 4] = 200;
            }
        }
        let scattered = pixels_of(&doc, layer_id);
        let mut command = LayerPixelsDeltaCommand::new(layer_id, "Scatter", start.clone(), scattered.clone());
        command.undo(&mut doc);
        assert_eq!(pixels_of(&doc, layer_id), start);
        command.redo(&mut doc);
        assert_eq!(pixels_of(&doc, layer_id), scattered);

        // No-op: retains nothing, so a caller can skip it.
        let noop = LayerPixelsDeltaCommand::new(layer_id, "Noop", scattered.clone(), scattered.clone());
        assert!(noop.is_empty());
        assert_eq!(noop.cost(), 0);

        // Different lengths (a resize) fall back to whole buffers and still restore exactly.
        let mut resized = LayerPixelsDeltaCommand::new(layer_id, "Resize", scattered.clone(), scattered[..100].to_vec());
        assert!(resized.cost() > 0);
        resized.undo(&mut doc);
        assert_eq!(pixels_of(&doc, layer_id), scattered);
    }

    /// The milestone's gate: a long editing session must undo and redo to byte-identical
    /// state, including after undoing only part of it and redoing some of that.
    #[test]
    fn long_edit_session_undoes_and_redoes_to_identical_bytes() {
        const W: u32 = 256;
        const H: u32 = 256;
        const STEPS: usize = 300;

        let mut doc = Document::new(W, H);
        let layer_id = doc.active_layer_id;
        let initial = pixels_of(&doc, layer_id);
        let mut stack = HistoryStack::with_budget(1024, usize::MAX);
        let mut states = Vec::with_capacity(STEPS);

        for step in 0..STEPS {
            let before = pixels_of(&doc, layer_id);
            let x = (step as u32 * 7) % (W - 8);
            let y = (step as u32 * 11) % (H - 8);
            paint_square(&mut doc, layer_id, x, y, 8, (step % 251) as u8);
            let after = pixels_of(&doc, layer_id);
            stack.push(Box::new(LayerPixelsDeltaCommand::new(
                layer_id,
                "Stroke",
                before,
                after.clone(),
            )));
            states.push(after);
        }
        let final_state = states.last().cloned().expect("states");
        assert_eq!(stack.len(), STEPS);

        while stack.can_undo() {
            stack.undo(&mut doc);
        }
        assert_eq!(pixels_of(&doc, layer_id), initial, "undoing everything must return to the start");

        while stack.can_redo() {
            stack.redo(&mut doc);
        }
        assert_eq!(pixels_of(&doc, layer_id), final_state, "redoing everything must return to the end");

        // Partial and interleaved: undo 50, redo 20, and the state must land on the same
        // intermediate buffer a straight session would have produced at that point.
        for _ in 0..50 {
            stack.undo(&mut doc);
        }
        for _ in 0..20 {
            stack.redo(&mut doc);
        }
        assert_eq!(pixels_of(&doc, layer_id), states[STEPS - 50 + 20 - 1]);
    }

    /// The point of the change: retained history follows the edit, not the canvas.
    #[test]
    fn delta_history_retains_a_fraction_of_full_snapshots() {
        const W: u32 = 256;
        const H: u32 = 256;
        const STEPS: usize = 200;

        let mut delta_stack = HistoryStack::with_budget(1024, usize::MAX);
        let mut snapshot_stack = HistoryStack::with_budget(1024, usize::MAX);
        let mut delta_doc = Document::new(W, H);
        let mut snapshot_doc = Document::new(W, H);
        let layer_id = delta_doc.active_layer_id;

        for step in 0..STEPS {
            let x = (step as u32 * 7) % (W - 8);
            let y = (step as u32 * 11) % (H - 8);
            let value = (step % 251) as u8;

            let before = pixels_of(&delta_doc, layer_id);
            paint_square(&mut delta_doc, layer_id, x, y, 8, value);
            let after = pixels_of(&delta_doc, layer_id);
            delta_stack.push(Box::new(LayerPixelsDeltaCommand::new(layer_id, "Stroke", before.clone(), after.clone())));

            let before = pixels_of(&snapshot_doc, layer_id);
            paint_square(&mut snapshot_doc, layer_id, x, y, 8, value);
            let after = pixels_of(&snapshot_doc, layer_id);
            snapshot_stack.push(Box::new(LayerPixelsSnapshotCommand {
                layer_id,
                description: "Stroke",
                before_pixels: before,
                after_pixels: after,
            }));
        }

        let delta_bytes = delta_stack.retained_bytes();
        let snapshot_bytes = snapshot_stack.retained_bytes();
        println!(
            "history retained after {STEPS} strokes on {W}x{H}: delta {:.2} MB, snapshots {:.2} MB ({:.0}x)",
            delta_bytes as f64 / (1024.0 * 1024.0),
            snapshot_bytes as f64 / (1024.0 * 1024.0),
            snapshot_bytes as f64 / delta_bytes as f64
        );
        assert!(
            delta_bytes * 40 < snapshot_bytes,
            "expected delta history to be at least 40x smaller, got {delta_bytes} vs {snapshot_bytes}"
        );
    }

    /// Constraint for gate 3: history must stay storage-format agnostic.
    ///
    /// The delta stores byte spans and never parses them, so a 16-bit buffer - or anything
    /// else - needs no change here. This restores a buffer of values that are not eight-bit
    /// pixels at all and checks them exactly, which fails if the diff ever starts assuming
    /// 4-byte pixels, a stride, or a channel layout.
    #[test]
    fn deltas_do_not_assume_eight_bit_pixels() {
        let mut doc = Document::new(4, 4);
        let layer_id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(layer_id) {
            for (index, byte) in layer.as_srgb8_exact_mut().iter_mut().enumerate() {
                *byte = (index * 37 % 251) as u8;
            }
        }
        let before = pixels_of(&doc, layer_id);
        if let Some(layer) = doc.get_layer_mut(layer_id) {
            for (index, byte) in layer.as_srgb8_exact_mut().iter_mut().enumerate() {
                *byte = (index * 91 + 13 % 241) as u8;
            }
        }
        let after = pixels_of(&doc, layer_id);

        let mut command = LayerPixelsDeltaCommand::new(layer_id, "Opaque bytes", before.clone(), after.clone());
        command.undo(&mut doc);
        assert_eq!(pixels_of(&doc, layer_id), before, "undo must be byte-exact");
        command.redo(&mut doc);
        assert_eq!(pixels_of(&doc, layer_id), after, "redo must be byte-exact");
    }

    /// The stronger form of the format-agnostic constraint: the same delta machinery must
    /// round-trip a **16-bit** layer exactly, without knowing it is 16-bit.
    #[test]
    fn deltas_round_trip_a_sixteen_bit_layer() {
        use crate::layer::LayerBuffer;

        let mut doc = Document::new(8, 8);
        let layer_id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(layer_id) {
            let channels: Vec<u16> = (0..(8 * 8 * 4) as u16).map(|i| i.wrapping_mul(257)).collect();
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }
        let read_all = |doc: &Document| -> Vec<u8> {
            let layer = doc.get_layer(layer_id).expect("layer");
            let mut bytes = vec![0u8; layer.storage_len()];
            assert!(layer.read_storage_bytes(0, &mut bytes));
            bytes
        };

        let before = read_all(&doc);
        if let Some(channels) = doc.get_layer_mut(layer_id).and_then(|l| l.as_srgb16_mut()) {
            for (index, channel) in channels.iter_mut().enumerate().step_by(3) {
                *channel = channel.wrapping_add(1000 + index as u16);
            }
        }
        let after = read_all(&doc);
        assert_ne!(before, after, "the fixture must change channels");

        let mut command = LayerPixelsDeltaCommand::new(layer_id, "16-bit edit", before.clone(), after.clone());
        command.undo(&mut doc);
        assert_eq!(read_all(&doc), before, "undo of a 16-bit layer must be byte-exact");
        command.redo(&mut doc);
        assert_eq!(read_all(&doc), after, "redo of a 16-bit layer must be byte-exact");
    }

    /// The whole-buffer snapshot commands are the fallback the delta does not cover. They used
    /// to read `as_srgb8_exact()`, which panics on 16-bit storage - and `cost()` runs on every
    /// push, so a structural edit on a 16-bit document would have panicked before being
    /// recorded. Both now work in storage bytes.
    #[test]
    fn snapshots_measure_and_restore_sixteen_bit_layers() {
        use crate::layer::LayerBuffer;

        let mut doc = Document::new(4, 4);
        let layer_id = doc.active_layer_id;
        if let Some(layer) = doc.get_layer_mut(layer_id) {
            let channels: Vec<u16> = (0..(4 * 4 * 4u16))
                .map(|index| index.wrapping_mul(4099).wrapping_add(7))
                .collect();
            *layer.buffer_mut() = LayerBuffer::Srgb16(channels);
        }

        let before_bytes = doc.get_layer(layer_id).expect("layer").storage_bytes();
        let before_layers = doc.layers.clone();

        if let Some(channels) = doc.get_layer_mut(layer_id).and_then(|l| l.as_srgb16_mut()) {
            for channel in channels.iter_mut().step_by(5) {
                *channel = channel.wrapping_add(999);
            }
        }
        let after_bytes = doc.get_layer(layer_id).expect("layer").storage_bytes();
        let after_layers = doc.layers.clone();
        assert_ne!(before_bytes, after_bytes, "the fixture must change channels");

        let expected_cost = before_layers.iter().map(|l| l.storage_len()).sum::<usize>()
            + after_layers.iter().map(|l| l.storage_len()).sum::<usize>();
        assert_eq!(
            expected_cost,
            before_bytes.len() + after_bytes.len(),
            "with one layer, the document cost is its two storage buffers"
        );

        let mut snapshot = FullDocumentSnapshotCommand {
            description: "16-bit structural edit",
            before_layers,
            after_layers,
            before_active_id: layer_id,
            after_active_id: layer_id,
        };
        assert_eq!(snapshot.cost(), expected_cost, "cost must count storage bytes");

        snapshot.undo(&mut doc);
        assert_eq!(
            doc.get_layer(layer_id).expect("layer").storage_bytes(),
            before_bytes,
            "undo of a document snapshot must be byte-exact for a 16-bit layer"
        );
        snapshot.redo(&mut doc);
        assert_eq!(
            doc.get_layer(layer_id).expect("layer").storage_bytes(),
            after_bytes,
            "redo of a document snapshot must be byte-exact for a 16-bit layer"
        );

        // The single-layer snapshot follows the same rule.
        let mut single = LayerPixelsSnapshotCommand {
            layer_id,
            description: "16-bit layer snapshot",
            before_pixels: before_bytes.clone(),
            after_pixels: after_bytes.clone(),
        };
        single.undo(&mut doc);
        assert_eq!(doc.get_layer(layer_id).expect("layer").storage_bytes(), before_bytes);
        single.redo(&mut doc);
        assert_eq!(doc.get_layer(layer_id).expect("layer").storage_bytes(), after_bytes);
    }

    /// The History panel reads labels and costs through the read-only API only. Entry order is
    /// the order the user thinks in: oldest undo first, nearest redo first, and the byte totals
    /// must follow commands as they cross between the two stacks.
    #[test]
    fn history_entries_report_order_labels_and_memory() {
        let mut doc = Document::new(16, 16);
        let layer_id = doc.active_layer_id;
        let mut stack = HistoryStack::with_budget(64, usize::MAX);

        let mut expected_bytes = 0usize;
        for step in 0..3u8 {
            let before = pixels_of(&doc, layer_id);
            paint_square(&mut doc, layer_id, step as u32 * 4, 0, 3, 40 + step);
            let after = pixels_of(&doc, layer_id);
            let command = LayerPixelsDeltaCommand::new(
                layer_id,
                if step == 0 { "Stroke" } else { "Fill" },
                before,
                after,
            );
            expected_bytes += command.cost();
            stack.push(Box::new(command));
        }

        let entries = stack.undo_entries();
        assert_eq!(entries.iter().map(|e| e.label).collect::<Vec<_>>(), vec!["Stroke", "Fill", "Fill"]);
        assert_eq!(stack.total_retained_bytes(), expected_bytes, "undo bytes account every step");
        assert!(entries.iter().all(|e| e.bytes > 0));

        // Undo moves a step to the redo stack and the totals must not double-count it.
        stack.undo(&mut doc);
        let undo_bytes = expected_bytes - entries[2].bytes;
        assert_eq!(stack.retained_bytes(), undo_bytes);
        assert_eq!(stack.redo_retained_bytes(), entries[2].bytes);
        assert_eq!(stack.total_retained_bytes(), expected_bytes);
        assert_eq!(stack.redo_entries()[0].label, "Fill");
        assert_eq!(stack.redo_entries().len(), 1);

        // A push clears redo, and its memory with it.
        let before = pixels_of(&doc, layer_id);
        paint_square(&mut doc, layer_id, 1, 1, 2, 200);
        let after = pixels_of(&doc, layer_id);
        stack.push(Box::new(LayerPixelsDeltaCommand::new(layer_id, "Dab", before, after)));
        assert_eq!(stack.redo_retained_bytes(), 0);
        assert!(stack.redo_entries().is_empty());

        // A smaller budget evicts from the oldest end immediately, never the newest step.
        stack.set_max_history(2);
        assert_eq!(stack.undo_entries().len(), 2);
        assert_eq!(stack.undo_entries().last().map(|e| e.label), Some("Dab"));
        assert_eq!(
            stack.retained_bytes(),
            stack.undo_entries().iter().map(|e| e.bytes).sum::<usize>(),
            "eviction must subtract exactly what it dropped"
        );

        stack.set_max_bytes(1);
        assert_eq!(stack.undo_entries().len(), 1, "one oversized step always survives");
        stack.clear();
        assert_eq!(stack.total_retained_bytes(), 0);
        assert!(stack.undo_entries().is_empty() && stack.redo_entries().is_empty());
    }

    /// The panel must not be able to mutate history: reading it twice is stable and does not
    /// change the generation counter undo/redo and dirty tracking rely on.
    #[test]
    fn history_inspection_is_side_effect_free() {
        let mut doc = Document::new(8, 8);
        let layer_id = doc.active_layer_id;
        let before = pixels_of(&doc, layer_id);
        paint_square(&mut doc, layer_id, 0, 0, 2, 77);
        let after = pixels_of(&doc, layer_id);
        let mut stack = HistoryStack::new(16);
        stack.push(Box::new(LayerPixelsDeltaCommand::new(layer_id, "Dab", before, after)));
        stack.undo(&mut doc);

        let generation = stack.generation();
        let total = stack.total_retained_bytes();
        let a = stack.undo_entries();
        let b = stack.undo_entries();
        let c = stack.redo_entries();
        assert_eq!(a, b);
        assert!(a.is_empty() && c.len() == 1);
        assert_eq!(stack.generation(), generation);
        assert_eq!(stack.total_retained_bytes(), total);
    }
}

#[cfg(test)]
mod mask_history_tests {
    use super::{Command, MaskPixelsCommand};
    use crate::document::Document;
    use crate::layer::LayerMask;

    #[test]
    fn mask_undo_restores_pixels_and_enabled_state() {
        let mut doc = Document::new(8, 8);
        let layer_id = doc.active_layer_id;
        doc.add_layer_mask(layer_id, None);
        let before = doc
            .get_layer(layer_id)
            .and_then(|layer| layer.mask.as_ref())
            .map(|mask| mask.pixels.clone())
            .expect("mask");

        if let Some(mask) = doc.get_layer_mut(layer_id).and_then(|l| l.mask.as_mut()) {
            mask.paint_dab(glam::Vec2::new(4.0, 4.0), 3.0, 1.0, 1.0, 0, None);
            mask.enabled = false;
        }
        let after = doc
            .get_layer(layer_id)
            .and_then(|layer| layer.mask.as_ref())
            .map(|mask| mask.pixels.clone())
            .expect("mask");

        let mut command = MaskPixelsCommand {
            layer_id,
            description: "Mask stroke",
            before_pixels: before.clone(),
            after_pixels: after.clone(),
            before_enabled: true,
            after_enabled: false,
        };
        assert_eq!(command.cost(), before.len() + after.len());
        assert_ne!(before, after, "the fixture must change the mask");

        command.undo(&mut doc);
        let restored = doc.get_layer(layer_id).and_then(|l| l.mask.as_ref()).expect("mask");
        assert_eq!(restored.pixels, before);
        assert!(restored.enabled, "disabled state is part of the undone state");

        command.redo(&mut doc);
        let reapplied = doc.get_layer(layer_id).and_then(|l| l.mask.as_ref()).expect("mask");
        assert_eq!(reapplied.pixels, after);
        assert!(!reapplied.enabled);

        // A missing mask is not resurrected by undo, and must not panic.
        doc.remove_layer_mask(layer_id);
        command.undo(&mut doc);
        assert!(doc.get_layer(layer_id).and_then(|l| l.mask.as_ref()).is_none());
    }

    /// LayerMask is the mask type the command guards against; a size change (for example after
    /// a canvas resize) must be ignored rather than panicking on a slice length mismatch.
    #[test]
    fn mask_undo_ignores_a_size_mismatch() {
        let mut doc = Document::new(8, 8);
        let layer_id = doc.active_layer_id;
        doc.add_layer_mask(layer_id, None);
        let mut command = MaskPixelsCommand {
            layer_id,
            description: "Mask stroke",
            before_pixels: vec![0u8; 4],
            after_pixels: vec![255u8; 4],
            before_enabled: true,
            after_enabled: true,
        };
        command.undo(&mut doc);
        let mask = doc.get_layer(layer_id).and_then(|l| l.mask.as_ref()).expect("mask");
        assert_eq!(mask.pixels.len(), 64, "the mismatched snapshot must not resize the mask");
        assert!(mask.is_all_visible());
    }
}
