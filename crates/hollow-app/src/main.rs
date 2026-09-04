use glam::Vec2;
use hollow_core::brush::{BrushPoint, ToolType};
use hollow_core::color::Color;
use hollow_core::history::LayerPixelsSnapshotCommand;
use hollow_core::rasterizer::StrokeRasterizer;
use hollow_core::selection::SelectionMask;
use hollow_io::export::{export_flat_image, ExportFormat};
use hollow_io::project::{load_project_file, save_project_file};
use hollow_ui::{
    configure_hollow_style, render_ui, AppState, PendingFileAction,
};


pub struct HollowCanvasDesktopApp {
    pub state: AppState,
    pub stroke_points: Vec<Vec2>,
    pub stabilizer_window: Vec<Vec2>,
    pub active_snapshot_taken: bool,
    pub stroke_dirty: bool,
    pub before_stroke_pixels: Vec<u8>,
    pub clone_source_pos: Option<Vec2>,
    pub shape_start_pos: Option<Vec2>,
    pub gradient_start_pos: Option<Vec2>,
    pub is_panning: bool,
}

impl HollowCanvasDesktopApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (w, h) = (1920, 1080);
        let state = AppState::new(w, h);
        Self {
            state,
            stroke_points: Vec::new(),
            stabilizer_window: Vec::new(),
            active_snapshot_taken: false,
            stroke_dirty: false,
            before_stroke_pixels: Vec::new(),
            clone_source_pos: None,
            shape_start_pos: None,
            gradient_start_pos: None,
            is_panning: false,
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.modifiers.command || i.modifiers.ctrl {
                if i.key_pressed(egui::Key::Z) {
                    if i.modifiers.shift {
                        if let Some(desc) = self.state.history.redo(&mut self.state.document) {
                            self.state.set_status(format!("Redo: {}", desc));
                            self.state.canvas_dirty = true;
                        }
                    } else {
                        if let Some(desc) = self.state.history.undo(&mut self.state.document) {
                            self.state.set_status(format!("Undo: {}", desc));
                            self.state.canvas_dirty = true;
                        }
                    }
                } else if i.key_pressed(egui::Key::Y) {
                    if let Some(desc) = self.state.history.redo(&mut self.state.document) {
                        self.state.set_status(format!("Redo: {}", desc));
                        self.state.canvas_dirty = true;
                    }
                } else if i.key_pressed(egui::Key::N) {
                    self.state.show_new_canvas_dialog = true;
                } else if i.key_pressed(egui::Key::O) {
                    self.state.pending_file_action = Some(PendingFileAction::OpenProject);
                } else if i.key_pressed(egui::Key::S) {
                    self.state.pending_file_action = Some(PendingFileAction::SaveProject);
                } else if i.key_pressed(egui::Key::E) {
                    self.state.pending_file_action = Some(PendingFileAction::ExportPng);
                } else if i.key_pressed(egui::Key::T) {
                    self.state.brush.tool = ToolType::Transform;
                    self.state.begin_transform_session();
                } else if i.key_pressed(egui::Key::A) {
                    self.state.selection = Some(SelectionMask::select_all(self.state.document.width, self.state.document.height));
                    self.state.set_status("Selected All");
                } else if i.key_pressed(egui::Key::D) {
                    self.state.selection = None;
                    self.state.set_status("Deselected");
                } else if i.key_pressed(egui::Key::I) {
                    if let Some(mask) = &mut self.state.selection {
                        mask.invert();
                        self.state.set_status("Selection inverted");
                    }
                }
            } else {
                if i.key_pressed(egui::Key::X) {
                    self.state.swap_colors();
                } else if i.key_pressed(egui::Key::B) {
                    self.state.brush.tool = ToolType::Brush;
                    self.state.set_status("Tool: Brush (B)");
                } else if i.key_pressed(egui::Key::E) {
                    self.state.brush.tool = ToolType::Eraser;
                    self.state.set_status("Tool: Eraser (E)");
                } else if i.key_pressed(egui::Key::I) {
                    self.state.brush.tool = ToolType::Eyedropper;
                    self.state.set_status("Tool: Eyedropper (I)");
                } else if i.key_pressed(egui::Key::G) {
                    self.state.brush.tool = ToolType::Gradient;
                    self.state.set_status("Tool: Gradient (G)");
                } else if i.key_pressed(egui::Key::W) {
                    self.state.brush.tool = ToolType::Wand;
                    self.state.set_status("Tool: Magic Wand (W)");
                } else if i.key_pressed(egui::Key::M) {
                    self.state.brush.tool = ToolType::Marquee;
                    self.state.set_status("Tool: Marquee (M)");
                } else if i.key_pressed(egui::Key::L) {
                    self.state.brush.tool = ToolType::Lasso;
                    self.state.set_status("Tool: Lasso (L)");
                } else if i.key_pressed(egui::Key::H) {
                    self.state.brush.tool = ToolType::Move;
                    self.state.set_status("Tool: Hand Move (H)");
                } else if i.key_pressed(egui::Key::OpenBracket) {
                    self.state.brush.size = (self.state.brush.size - 4.0).max(1.0);
                    self.state.set_status(format!("Brush Size: {:.1} px", self.state.brush.size));
                } else if i.key_pressed(egui::Key::CloseBracket) {
                    self.state.brush.size = (self.state.brush.size + 4.0).min(300.0);
                    self.state.set_status(format!("Brush Size: {:.1} px", self.state.brush.size));
                } else if i.key_pressed(egui::Key::Tab) {
                    self.state.show_ui_panels = !self.state.show_ui_panels;
                } else if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                    if self.state.selection.is_some() {
                        self.state.clear_selection_active_layer();
                        self.state.canvas_dirty = true;
                    }
                } else if i.key_pressed(egui::Key::Escape) {
                    if self.state.transform_session.is_active {
                        self.state.cancel_transform_session();
                    } else if self.state.selection.is_some() {
                        self.state.selection = None;
                        self.state.set_status("Deselected");
                    }
                } else if i.key_pressed(egui::Key::Enter) {
                    if self.state.transform_session.is_active {
                        self.state.commit_transform_session();
                        self.state.canvas_dirty = true;
                    }
                }
            }
        });
    }

    fn handle_file_actions(&mut self) {
        if let Some(action) = self.state.pending_file_action.take() {
            match action {
                PendingFileAction::SaveProject => {
                    if let Some(path) = hollow_ui::save_project_dialog() {
                        match save_project_file(&self.state.document, &path) {
                            Ok(_) => self.state.set_status(format!("Saved project: {}", path.display())),
                            Err(e) => self.state.set_status(format!("Save error: {}", e)),
                        }
                    }
                }
                PendingFileAction::OpenProject => {
                    if let Some(path) = hollow_ui::open_project_dialog() {
                        match load_project_file(&path) {
                            Ok(doc) => {
                                self.state = AppState::from_document(doc);
                                self.state.canvas_dirty = true;
                                self.state.set_status(format!("Loaded project: {}", path.display()));
                            }
                            Err(e) => self.state.set_status(format!("Open error: {}", e)),
                        }
                    }
                }
                PendingFileAction::ExportPng => {
                    if let Some(path) = hollow_ui::export_png_dialog() {
                        match export_flat_image(&self.state.document, &path, ExportFormat::Png, false) {
                            Ok(_) => self.state.set_status(format!("Exported PNG: {}", path.display())),
                            Err(e) => self.state.set_status(format!("Export error: {}", e)),
                        }
                    }
                }
                PendingFileAction::OpenReferenceImage => {
                    if let Some(path) = hollow_ui::open_image_dialog() {
                        if let Ok(img) = image::open(&path) {
                            let rgba = img.to_rgba8();
                            let (w, h) = rgba.dimensions();
                            self.state.reference_image = Some((w, h, rgba.into_raw()));
                            self.state.ref_texture = None;
                            self.state.set_status(format!("Loaded reference: {}", path.display()));
                        }
                    }
                }
                PendingFileAction::NewCanvas(w, h, bg_mode) => {
                    let mut doc = hollow_core::document::Document::new(w, h);
                    match bg_mode {
                        1 => {
                            doc.background_value = 255;
                            doc.is_transparent = false;
                        }
                        2 => {
                            doc.is_transparent = true;
                        }
                        _ => {
                            doc.background_value = 20;
                            doc.is_transparent = false;
                        }
                    }
                    self.state = AppState::from_document(doc);
                    self.state.canvas_dirty = true;
                    self.state.set_status(format!("Created new canvas: {} × {} px", w, h));
                }
            }
        }
    }

    fn handle_canvas_pointer_events(&mut self, ctx: &egui::Context) {
        let vp_rect = self.state.viewport_rect;
        if vp_rect.width() <= 0.0 || vp_rect.height() <= 0.0 {
            return;
        }

        let center = vp_rect.center() + egui::vec2(self.state.pan.x, self.state.pan.y);
        let doc_w = self.state.document.width as f32;
        let doc_h = self.state.document.height as f32;
        let can_w = doc_w * self.state.zoom;
        let can_h = doc_h * self.state.zoom;
        let canvas_rect = egui::Rect::from_center_size(center, egui::vec2(can_w, can_h));

        ctx.input(|i| {
            let pointer = &i.pointer;
            let hover_pos = pointer.hover_pos().or(pointer.latest_pos());

            // ── A. PAN & ZOOM GESTURES ──
            if let Some(pos) = hover_pos {
                if vp_rect.contains(pos) {
                    let scroll_y = i.raw_scroll_delta.y;
                    if scroll_y.abs() > 0.001 {
                        let factor = if scroll_y > 0.0 { 1.12 } else { 1.0 / 1.12 };
                        let old_zoom = self.state.zoom;
                        let new_zoom = (old_zoom * factor).clamp(0.05, 32.0);
                        let offset = pos - center;
                        let new_center = pos - offset * (new_zoom / old_zoom);
                        let d = new_center - vp_rect.center();
                        self.state.pan = glam::Vec2::new(d.x, d.y);
                        self.state.zoom = new_zoom;
                    }
                }
            }

            let space_down = i.key_down(egui::Key::Space);
            let middle_down = pointer.middle_down();
            let primary_down = pointer.primary_down();

            if middle_down || (space_down && primary_down) {
                let d = pointer.delta();
                self.state.pan += glam::Vec2::new(d.x, d.y);
                self.is_panning = true;
                return;
            } else {
                self.is_panning = false;
            }

            // ── B. CANVAS POINTER INTERACTION & TOOL PIPELINE ──
            if let Some(pos) = hover_pos {
                let inside_vp = vp_rect.contains(pos);
                if !inside_vp && !self.active_snapshot_taken {
                    return;
                }

                let canvas_x = (pos.x - canvas_rect.min.x) / self.state.zoom;
                let canvas_y = (pos.y - canvas_rect.min.y) / self.state.zoom;
                let raw_pos = Vec2::new(canvas_x, canvas_y);
                self.state.cursor_canvas_pos = raw_pos;

                if pointer.primary_pressed() && inside_vp {
                    self.on_pointer_press(raw_pos, i.modifiers.alt, i.modifiers.shift);
                }

                if pointer.primary_down() && self.active_snapshot_taken {
                    let pressure = 1.0;
                    self.on_pointer_drag(raw_pos, pressure);
                }

                if pointer.primary_released() {
                    self.on_pointer_release(raw_pos);
                }
            }
        });
    }

    fn on_pointer_press(&mut self, raw_pos: Vec2, _alt_held: bool, shift_held: bool) {
        match self.state.brush.tool {
            ToolType::Move => {
                // Hand move mode
            }
            ToolType::Eyedropper => {
                let ix = raw_pos.x.floor() as i32;
                let iy = raw_pos.y.floor() as i32;
                if ix >= 0 && ix < self.state.document.width as i32 && iy >= 0 && iy < self.state.document.height as i32 {
                    let idx = (iy as usize * self.state.document.width as usize + ix as usize) * 4;
                    if idx + 4 <= self.state.canvas_composite_buffer.len() {
                        let r = self.state.canvas_composite_buffer[idx];
                        let g = self.state.canvas_composite_buffer[idx + 1];
                        let b = self.state.canvas_composite_buffer[idx + 2];
                        let a = self.state.canvas_composite_buffer[idx + 3];
                        let c = Color::from_rgba8(r, g, b, a);
                        if self.state.color_slot_is_secondary {
                            self.state.brush.secondary_color = c;
                        } else {
                            self.state.brush.primary_color = c;
                            self.state.push_color_history(c);
                        }
                        self.state.set_status(format!("Sampled #{:02X}{:02X}{:02X}", r, g, b));
                    }
                }
            }
            ToolType::Fill => {
                let px = raw_pos.x.floor().max(0.0) as u32;
                let py = raw_pos.y.floor().max(0.0) as u32;
                if let Some(layer) = self.state.document.active_layer() {
                    let before = layer.pixels.clone();
                    let color = self.state.brush.primary_color;
                    let tol = self.state.wand_tolerance;
                    let sel_ref = self.state.selection.as_ref();
                    StrokeRasterizer::flood_fill(&mut self.state.document, px, py, color, sel_ref, tol);
                    if let Some(l) = self.state.document.active_layer() {
                        if before != l.pixels {
                            self.state.history.push(Box::new(LayerPixelsSnapshotCommand {
                                layer_id: l.id,
                                description: "Flood Fill",
                                before_pixels: before,
                                after_pixels: l.pixels.clone(),
                            }));
                            self.state.canvas_dirty = true;
                        }
                    }
                }
            }
            ToolType::Wand => {
                let px = raw_pos.x.floor().max(0.0) as u32;
                let py = raw_pos.y.floor().max(0.0) as u32;
                let mask = StrokeRasterizer::rasterize_magic_wand(
                    &self.state.document,
                    px,
                    py,
                    self.state.wand_tolerance,
                    self.state.wand_contiguous,
                    self.state.wand_sample_all_layers,
                );
                if shift_held {
                    if let Some(sel) = &mut self.state.selection {
                        sel.union(&mask);
                    } else {
                        self.state.selection = Some(mask);
                    }
                } else {
                    self.state.selection = Some(mask);
                }
                self.state.set_status("Magic Wand Selection updated");
            }
            ToolType::Gradient => {
                self.gradient_start_pos = Some(raw_pos);
            }
            ToolType::Line | ToolType::Rect | ToolType::Ellipse | ToolType::Marquee => {
                self.shape_start_pos = Some(raw_pos);
            }
            ToolType::Lasso => {
                self.state.lasso_points = vec![raw_pos];
                self.active_snapshot_taken = true;
            }
            ToolType::Transform => {
                // Transform sessions handle their own gizmo interaction
            }
            _ => {
                // Freehand brushes
                if let Some(layer) = self.state.document.active_layer() {
                    self.before_stroke_pixels = layer.pixels.clone();
                    self.active_snapshot_taken = true;
                    self.stroke_dirty = false;
                    self.stroke_points.clear();
                    self.stabilizer_window.clear();
                    self.state.is_painting = true;
                    self.on_pointer_drag(raw_pos, 1.0);
                }
            }
        }
    }

    fn on_pointer_drag(&mut self, raw_pos: Vec2, pressure: f32) {
        if self.state.brush.tool == ToolType::Lasso {
            if let Some(last) = self.state.lasso_points.last() {
                if last.distance(raw_pos) > 2.0 {
                    self.state.lasso_points.push(raw_pos);
                }
            }
            return;
        }

        let s_level = self.state.brush.stabilization_level;
        let smoothed_pos = if s_level > 0 {
            self.stabilizer_window.push(raw_pos);
            let window_size = (s_level as usize * 2) + 2;
            if self.stabilizer_window.len() > window_size {
                self.stabilizer_window.remove(0);
            }
            let mut sum = Vec2::ZERO;
            let mut weight_sum = 0.0;
            for (i, &pt) in self.stabilizer_window.iter().enumerate() {
                let w = (i + 1) as f32;
                sum += pt * w;
                weight_sum += w;
            }
            sum / weight_sum
        } else {
            raw_pos
        };

        let final_pos = if self.state.perspective.p_type != hollow_core::perspective::PerspectiveType::None && self.state.perspective.snap_enabled {
            if let Some(&start_pt) = self.stroke_points.first() {
                let (snapped, _) = self.state.perspective.constrain_stroke_point(start_pt, smoothed_pos, None);
                snapped
            } else {
                smoothed_pos
            }
        } else {
            smoothed_pos
        };

        if self.stroke_points.is_empty() {
            self.stroke_points.push(final_pos);
            let pt = BrushPoint::new(final_pos, pressure);
            let sel_ref = self.state.selection.as_ref();
            StrokeRasterizer::paint_dot(&mut self.state.document, pt, &self.state.brush, &self.state.symmetry, sel_ref);
            self.stroke_dirty = true;
            self.state.canvas_dirty = true;
        } else {
            let last_pt = *self.stroke_points.last().unwrap();
            if last_pt.distance(final_pos) >= 0.5 {
                let p0 = BrushPoint::new(last_pt, pressure);
                let p1 = BrushPoint::new(final_pos, pressure);
                let sel_ref = self.state.selection.as_ref();
                StrokeRasterizer::paint_segment(&mut self.state.document, p0, p1, &self.state.brush, &self.state.symmetry, sel_ref);
                self.stroke_points.push(final_pos);
                self.stroke_dirty = true;
                self.state.canvas_dirty = true;
            }
        }
    }

    fn on_pointer_release(&mut self, raw_pos: Vec2) {
        if let Some(start) = self.shape_start_pos.take() {
            if let Some(layer) = self.state.document.active_layer() {
                let before = layer.pixels.clone();
                let sel_ref = self.state.selection.as_ref();
                match self.state.brush.tool {
                    ToolType::Line => {
                        StrokeRasterizer::paint_segment(
                            &mut self.state.document,
                            BrushPoint::new(start, 1.0),
                            BrushPoint::new(raw_pos, 1.0),
                            &self.state.brush,
                            &self.state.symmetry,
                            sel_ref,
                        );
                    }
                    ToolType::Rect => {
                        StrokeRasterizer::rasterize_rect(
                            &mut self.state.document,
                            start,
                            raw_pos,
                            &self.state.brush,
                            &self.state.symmetry,
                            sel_ref,
                        );
                    }
                    ToolType::Ellipse => {
                        StrokeRasterizer::rasterize_ellipse(
                            &mut self.state.document,
                            start,
                            raw_pos,
                            &self.state.brush,
                            &self.state.symmetry,
                            sel_ref,
                        );
                    }
                    ToolType::Marquee => {
                        let min = Vec2::new(start.x.min(raw_pos.x), start.y.min(raw_pos.y));
                        let max = Vec2::new(start.x.max(raw_pos.x), start.y.max(raw_pos.y));
                        let mask = SelectionMask::from_rect(self.state.document.width, self.state.document.height, min, max);
                        self.state.selection = Some(mask);
                    }
                    _ => {}
                }
                if let Some(l) = self.state.document.active_layer() {
                    if before != l.pixels {
                        self.state.history.push(Box::new(LayerPixelsSnapshotCommand {
                            layer_id: l.id,
                            description: self.state.brush.tool.label(),
                            before_pixels: before,
                            after_pixels: l.pixels.clone(),
                        }));
                        self.state.canvas_dirty = true;
                    }
                }
            }
        }

        if let Some(start) = self.gradient_start_pos.take() {
            if let Some(layer) = self.state.document.active_layer() {
                let before = layer.pixels.clone();
                let sel_ref = self.state.selection.as_ref();
                StrokeRasterizer::rasterize_gradient(&mut self.state.document, start, raw_pos, &self.state.brush, sel_ref);
                if let Some(l) = self.state.document.active_layer() {
                    self.state.history.push(Box::new(LayerPixelsSnapshotCommand {
                        layer_id: l.id,
                        description: "Gradient Fill",
                        before_pixels: before,
                        after_pixels: l.pixels.clone(),
                    }));
                    self.state.canvas_dirty = true;
                }
            }
        }

        if !self.state.lasso_points.is_empty() {
            let mask = SelectionMask::from_polygon(self.state.document.width, self.state.document.height, &self.state.lasso_points);
            self.state.selection = Some(mask);
            self.state.lasso_points.clear();
            self.state.set_status("Lasso selection created");
        }

        if self.active_snapshot_taken {
            if self.stroke_dirty {
                if let Some(layer) = self.state.document.active_layer() {
                    let cmd = Box::new(LayerPixelsSnapshotCommand {
                        layer_id: layer.id,
                        description: self.state.brush.tool.label(),
                        before_pixels: std::mem::take(&mut self.before_stroke_pixels),
                        after_pixels: layer.pixels.clone(),
                    });
                    self.state.history.push(cmd);
                }
            }
            self.active_snapshot_taken = false;
            self.stroke_dirty = false;
            self.stroke_points.clear();
            self.stabilizer_window.clear();
            self.state.is_painting = false;
            self.state.canvas_dirty = true;
        }
    }
}

impl eframe::App for HollowCanvasDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        configure_hollow_style(ctx, self.state.theme_accent_color());
        self.handle_shortcuts(ctx);
        self.handle_file_actions();
        render_ui(ctx, &mut self.state);
        self.handle_canvas_pointer_events(ctx);

        if self.state.timeline.is_playing || self.state.is_painting || self.active_snapshot_taken {
            ctx.request_repaint();
        }
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("Hollow Canvas Studio v1.0.0-alpha.1"),
        ..Default::default()
    };
    eframe::run_native(
        "Hollow Canvas Studio",
        native_options,
        Box::new(|cc| {
            Ok(Box::new(HollowCanvasDesktopApp::new(cc)))
        }),
    )
}
