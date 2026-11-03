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

