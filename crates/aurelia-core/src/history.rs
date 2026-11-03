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
            if layer.as_srgb8().len() == self.before_pixels.len() {
                layer.as_srgb8_mut().copy_from_slice(&self.before_pixels);
            }
        }
    }

    fn redo(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.get_layer_mut(self.layer_id) {
            if layer.as_srgb8().len() == self.after_pixels.len() {
                layer.as_srgb8_mut().copy_from_slice(&self.after_pixels);
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

    fn apply(&self, pixels: &mut [u8], use_after: bool) {
        match self {
            PixelDelta::Spans(spans) => {
                for span in spans {
                    let source = if use_after { &span.after } else { &span.before };
                    if span.offset + source.len() <= pixels.len() {
                        pixels[span.offset..span.offset + source.len()].copy_from_slice(source);
                    }
                }
            }
            PixelDelta::Whole { before, after } => {
                let source = if use_after { after } else { before };
                if pixels.len() == source.len() {
                    pixels.copy_from_slice(source);
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

/// A layer edit stored as the regions that actually changed.
///
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
            self.delta.apply(layer.as_srgb8_mut(), false);
        }
    }

    fn redo(&mut self, doc: &mut Document) {
        if let Some(layer) = doc.get_layer_mut(self.layer_id) {
            self.delta.apply(layer.as_srgb8_mut(), true);
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
        let sum = |layers: &[Layer]| -> usize { layers.iter().map(|l| l.as_srgb8().len()).sum() };
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

pub struct HistoryStack {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
    max_history: usize,
    max_bytes: usize,
    undo_bytes: usize,
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
            generation: 0,
        }
    }

    pub fn push(&mut self, cmd: Box<dyn Command>) {
        self.generation = self.generation.saturating_add(1);
        self.undo_bytes = self.undo_bytes.saturating_add(cmd.cost());
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
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
        self.undo_bytes = self.undo_bytes.saturating_sub(cmd.cost());
        cmd.undo(doc);
        let name = cmd.name();
        self.redo_stack.push(cmd);
        Some(name)
    }

    pub fn redo(&mut self, doc: &mut Document) -> Option<&'static str> {
        let mut cmd = self.redo_stack.pop()?;
        self.generation = self.generation.saturating_add(1);
        self.undo_bytes = self.undo_bytes.saturating_add(cmd.cost());
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

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
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
        self.generation = self.generation.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, HistoryStack, LayerPixelsDeltaCommand, LayerPixelsSnapshotCommand};
    use crate::document::Document;

    fn pixels_of(doc: &Document, layer_id: u64) -> Vec<u8> {
        doc.get_layer(layer_id)
            .expect("layer")
            .as_srgb8()
            .to_vec()
    }

    /// Paints a solid square - the shape of an ordinary brush dab.
    fn paint_square(doc: &mut Document, layer_id: u64, x: u32, y: u32, size: u32, value: u8) {
        let width = doc.width as usize;
        if let Some(layer) = doc.get_layer_mut(layer_id) {
            for dy in 0..size as usize {
                for dx in 0..size as usize {
                    let at = ((y as usize + dy) * width + (x as usize + dx)) * 4;
                    let pixels = layer.as_srgb8_mut();
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
            let pixels = layer.as_srgb8_mut();
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
}
