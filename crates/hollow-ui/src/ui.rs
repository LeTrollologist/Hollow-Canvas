use crate::state::{AppState, PendingFileAction};
use egui::{Align, Color32, Layout, RichText, ScrollArea, Vec2};
use hollow_core::blend::BlendMode;
use hollow_core::brush::{ShapeFillMode, ToolType};
use hollow_core::color::{Color, ThemeMode, DEFAULT_PALETTE};
use hollow_core::symmetry::SymmetryMode;

pub fn render_ui(ctx: &egui::Context, state: &mut AppState) {
    let accent = state.document.theme.accent_color();
    let [ar, ag, ab, _] = accent.to_rgba8();
    let accent_c32 = Color32::from_rgb(ar, ag, ab);
    let accent_dim_c32 = Color32::from_rgba_unmultiplied(ar, ag, ab, 50);

    // ── TOP HEADER ──
    egui::TopBottomPanel::top("header_panel")
        .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(5, 8, 20, 230)).inner_margin(8.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Hollow Canvas").size(15.0).strong().color(Color32::from_rgb(205, 213, 237)));
                ui.label(RichText::new("Graphics Studio").size(10.0).color(Color32::from_rgb(92, 106, 133)));

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add_space(20.0);
                    let tool_label = format!(
                        "{} · {}px · {}%",
                        state.brush.tool.label(),
                        state.brush.size as u32,
                        (state.brush.opacity * 100.0).round() as u32
                    );
                    egui::Frame::none()
                        .fill(accent_dim_c32)
                        .stroke(egui::Stroke::new(1.0_f32, accent_c32))
                        .rounding(2.0)
                        .inner_margin(egui::vec2(10.0, 3.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(tool_label).size(11.0).color(accent_c32));
                        });
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Safe non-blocking file actions dispatched to main loop
                    if ui.button(RichText::new("💾 Save").color(accent_c32)).clicked() {
                        state.pending_file_action = Some(PendingFileAction::SaveProject);
                    }

                    if ui.button("Export PNG").clicked() {
                        state.pending_file_action = Some(PendingFileAction::ExportPng);
                    }

                    if ui.button("Open Project").clicked() {
                        state.pending_file_action = Some(PendingFileAction::OpenProject);
                    }

                    // Reference image dock button
                    let ref_label = if state.show_ref_window { "🖼 Ref [ON]" } else { "🖼 Ref" };
                    if ui.button(ref_label).clicked() {
                        state.show_ref_window = !state.show_ref_window;
                    }

                    if ui.button("Help (?)").clicked() {
                        state.show_help = !state.show_help;
                    }
                });
            });
        });

    // ── BOTTOM STATUS BAR ──
    egui::TopBottomPanel::bottom("status_bar")
        .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(3, 5, 12, 240)).inner_margin(4.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").size(8.0).color(accent_c32));
                ui.label(RichText::new(&state.status_message).size(10.0).color(Color32::from_rgb(150, 160, 185)));

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{}×{}", state.document.width, state.document.height))
                            .size(10.0)
                            .color(Color32::from_rgb(120, 130, 155)),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            state.cursor_canvas_pos.x.round() as i32,
                            state.cursor_canvas_pos.y.round() as i32
                        ))
                        .size(10.0)
                        .color(Color32::from_rgb(120, 130, 155)),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!("{}%", (state.zoom * 100.0).round() as i32))
                            .size(10.0)
                            .color(accent_c32),
                    );
                });
            });
        });

    // ── LEFT SIDEBAR (TOOLS & BRUSH PROPERTIES) ──
    egui::SidePanel::left("left_tools_panel")
        .default_width(200.0)
        .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(5, 8, 20, 230)).inner_margin(10.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.label(RichText::new("TOOLS").size(9.0).strong().color(Color32::from_rgb(92, 106, 133)));

                let tools = [
                    (ToolType::Brush, "✦ Brush"),
                    (ToolType::Pencil, "✏ Pencil"),
                    (ToolType::Watercolor, "💧 Water"),
                    (ToolType::Chalk, "▧ Chalk"),
                    (ToolType::Spray, "✧ Spray"),
                    (ToolType::Smudge, "〰 Smudge"),
                    (ToolType::Clone, "⎘ Clone"),
                    (ToolType::Line, "╱ Line"),
                    (ToolType::Fill, "◈ Fill"),
                    (ToolType::Eraser, "◻ Eraser"),
                    (ToolType::Rect, "▭ Rect"),
                    (ToolType::Ellipse, "○ Oval"),
                    (ToolType::Polygon, "⬠ Poly"),
                    (ToolType::Move, "✥ Move"),
                    (ToolType::Marquee, "▢ Select"),
                    (ToolType::Crop, "⛶ Crop"),
                    (ToolType::Eyedropper, "🔍 Pick"),
                ];

                egui::Grid::new("tools_grid").num_columns(2).spacing([4.0, 4.0]).show(ui, |ui| {
                    for (i, (t, label)) in tools.iter().enumerate() {
                        let selected = state.brush.tool == *t;
                        let btn = egui::Button::new(
                            RichText::new(*label).size(11.0).color(if selected { accent_c32 } else { Color32::from_rgb(180, 190, 215) })
                        ).min_size(Vec2::new(84.0, 24.0));

                        if ui.add(btn).clicked() {
                            state.brush.tool = *t;
                            state.set_status(format!("Tool: {}", t.label()));
                        }
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });

                ui.separator();
                ui.label(RichText::new("SYMMETRY").size(9.0).strong().color(Color32::from_rgb(92, 106, 133)));
                ui.horizontal(|ui| {
                    let modes = [
                        (SymmetryMode::None, "Ø"),
                        (SymmetryMode::Horizontal, "◫"),
                        (SymmetryMode::Vertical, "⊟"),
                        (SymmetryMode::Quad, "⊞"),
                    ];
                    for (m, icon) in modes {
                        let selected = state.symmetry.mode == m;
                        if ui.selectable_label(selected, icon).clicked() {
                            state.symmetry.mode = m;
                            state.set_status(format!("Symmetry: {}", m.label()));
                        }
                    }
                });

                ui.add(egui::Slider::new(&mut state.symmetry.mandala_segments, 0..=24).text("Mandala"));

                ui.separator();
                ui.label(RichText::new("BRUSH DYNAMICS").size(9.0).strong().color(Color32::from_rgb(92, 106, 133)));
                ui.add(egui::Slider::new(&mut state.brush.size, 1.0..=160.0).text("Size"));
                ui.add(egui::Slider::new(&mut state.brush.opacity, 0.05..=1.0).text("Opacity"));
                ui.add(egui::Slider::new(&mut state.brush.smoothing, 0.0..=0.95).text("Smoothing"));
                ui.add(egui::Slider::new(&mut state.brush.hardness, 0.05..=1.0).text("Hardness"));
                ui.add(egui::Slider::new(&mut state.brush.spacing, 0.05..=1.0).text("Spacing"));

                // Tool-specific properties
                match state.brush.tool {
                    ToolType::Watercolor => {
                        ui.add(egui::Slider::new(&mut state.brush.watercolor_wetness, 0.1..=1.0).text("Wetness"));
                    }
                    ToolType::Chalk => {
                        ui.add(egui::Slider::new(&mut state.brush.chalk_grain, 0.1..=1.0).text("Grain"));
                    }
                    ToolType::Spray => {
                        ui.add(egui::Slider::new(&mut state.brush.spray_density, 0.1..=2.0).text("Density"));
                    }
                    ToolType::Smudge => {
                        ui.add(egui::Slider::new(&mut state.brush.smudge_strength, 0.1..=1.0).text("Strength"));
                    }
                    ToolType::Clone => {
                        ui.checkbox(&mut state.brush.clone_aligned, "Aligned Clone");
                        if let Some(src) = state.brush.clone_source {
                            ui.label(RichText::new(format!("Source: ({}, {})", src.x as i32, src.y as i32)).size(10.0).color(accent_c32));
                        } else {
                            ui.label(RichText::new("Alt+Click canvas to set Source").size(10.0).color(Color32::from_rgb(220, 180, 80)));
                        }
                    }
                    ToolType::Line | ToolType::Rect | ToolType::Ellipse | ToolType::Polygon => {
                        ui.horizontal(|ui| {
                            ui.label("Mode:");
                            ui.selectable_value(&mut state.brush.shape_fill_mode, ShapeFillMode::Stroke, "Stroke");
                            ui.selectable_value(&mut state.brush.shape_fill_mode, ShapeFillMode::Fill, "Fill");
                            ui.selectable_value(&mut state.brush.shape_fill_mode, ShapeFillMode::Both, "Both");
                        });
                    }
                    ToolType::Marquee => {
                        if state.selection.as_ref().map_or(false, |s| s.has_selection()) {
                            if ui.button("✕ Deselect (Ctrl+D)").clicked() {
                                state.selection = None;
                                state.set_status("Deselected");
                            }
                        } else {
                            ui.label(RichText::new("Drag on canvas to select area").size(10.0).color(Color32::from_rgb(140, 150, 180)));
                        }
                    }
                    ToolType::Crop => {
                        if state.crop_box.is_some() {
                            ui.horizontal(|ui| {
                                if ui.button("✓ Apply Crop").clicked() {
                                    if let Some((p0, p1)) = state.crop_box {
                                        let min_x = p0.x.min(p1.x).max(0.0) as u32;
                                        let min_y = p0.y.min(p1.y).max(0.0) as u32;
                                        let max_x = p0.x.max(p1.x).min(state.document.width as f32) as u32;
                                        let max_y = p0.y.max(p1.y).min(state.document.height as f32) as u32;
                                        let new_w = (max_x - min_x).max(1);
                                        let new_h = (max_y - min_y).max(1);
                                        state.document.resize(new_w, new_h);
                                        state.crop_box = None;
                                        state.set_status(format!("Cropped to {}×{}", new_w, new_h));
                                    }
                                }
                                if ui.button("✕ Cancel").clicked() {
                                    state.crop_box = None;
                                }
                            });
                        } else {
                            ui.label(RichText::new("Drag to define crop box").size(10.0).color(Color32::from_rgb(140, 150, 180)));
                        }
                    }
                    _ => {}
                }

                ui.checkbox(&mut state.brush.eraser_to_background, "Erase to background");

                ui.separator();
                ui.label(RichText::new("ACTIONS").size(9.0).strong().color(Color32::from_rgb(92, 106, 133)));
                ui.horizontal(|ui| {
                    if ui.button("↩ Undo").clicked() {
                        if let Some(name) = state.history.undo(&mut state.document) {
                            state.set_status(format!("Undo: {}", name));
                        }
                    }
                    if ui.button("↪ Redo").clicked() {
                        if let Some(name) = state.history.redo(&mut state.document) {
                            state.set_status(format!("Redo: {}", name));
                        }
                    }
                });

                ui.horizontal(|ui| {
                    if ui.button("⇋ Flip H").clicked() {
                        state.document.flip(true);
                        state.set_status("Flipped horizontally");
                    }
                    if ui.button("⇅ Flip V").clicked() {
                        state.document.flip(false);
                        state.set_status("Flipped vertically");
                    }
                });

                ui.horizontal(|ui| {
                    if ui.button("⟲ Rot -90°").clicked() {
                        state.document.rotate_90(false);
                        state.set_status("Rotated -90°");
                    }
                    if ui.button("⟳ Rot +90°").clicked() {
                        state.document.rotate_90(true);
                        state.set_status("Rotated +90°");
                    }
                });

                if ui.button(RichText::new("✕ Clear Canvas").color(Color32::from_rgb(230, 110, 110))).clicked() {
                    for layer in &mut state.document.layers {
                        layer.clear();
                    }
                    state.set_status("Canvas cleared");
                }
            });
        });

    // ── RIGHT SIDEBAR (LAYERS & COLOR PICKER & THEMES) ──
    egui::SidePanel::right("right_inspector_panel")
        .default_width(230.0)
        .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(5, 8, 20, 230)).inner_margin(10.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                // ── LAYERS ──
                ui.label(RichText::new("LAYERS").size(9.0).strong().color(Color32::from_rgb(92, 106, 133)));

                let mut layer_to_move = None;
                let active_id = state.document.active_layer_id;

                for layer in state.document.layers.iter_mut().rev() {
                    let is_active = layer.id == active_id;
                    let frame_color = if is_active {
                        accent_dim_c32
                    } else {
                        Color32::from_rgba_unmultiplied(255, 255, 255, 6)
                    };

                    egui::Frame::none()
                        .fill(frame_color)
                        .stroke(egui::Stroke::new(1.0_f32, if is_active { accent_c32 } else { Color32::from_rgba_unmultiplied(168, 159, 216, 25) }))
                        .rounding(4.0)
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let vis_icon = if layer.visible { "👁" } else { "◌" };
                                if ui.small_button(vis_icon).clicked() {
                                    layer.visible = !layer.visible;
                                }

                                let lock_icon = if layer.locked { "🔒" } else { "🔓" };
                                if ui.small_button(lock_icon).clicked() {
                                    layer.locked = !layer.locked;
                                }

                                // Reference layer toggle
                                let ref_icon = if layer.is_reference { "📌 Ref" } else { "Ref" };
                                let ref_color = if layer.is_reference { accent_c32 } else { Color32::from_rgb(100, 110, 130) };
                                if ui.button(RichText::new(ref_icon).size(10.0).color(ref_color)).clicked() {
                                    layer.is_reference = !layer.is_reference;
                                }

                                let name_label = RichText::new(&layer.name)
                                    .size(11.0)
                                    .color(if is_active { accent_c32 } else { Color32::from_rgb(205, 213, 237) });

                                if ui.selectable_label(is_active, name_label).clicked() {
                                    state.document.active_layer_id = layer.id;
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Op:").size(9.0));
                                ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).show_value(true));
                            });

                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_source(format!("blend_{}", layer.id))
                                    .selected_text(layer.blend_mode.label())
                                    .show_ui(ui, |ui| {
                                        for mode in BlendMode::ALL {
                                            ui.selectable_value(&mut layer.blend_mode, *mode, mode.label());
                                        }
                                    });

                                if ui.small_button("▲").clicked() {
                                    layer_to_move = Some((layer.id, 1));
                                }
                                if ui.small_button("▼").clicked() {
                                    layer_to_move = Some((layer.id, -1));
                                }
                            });

                            if layer.offset_x != 0 || layer.offset_y != 0 {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("Offset: {}, {}", layer.offset_x, layer.offset_y)).size(9.0).color(Color32::from_rgb(255, 180, 100)));
                                    if ui.button("⟲ Reset Pos").clicked() {
                                        layer.offset_x = 0;
                                        layer.offset_y = 0;
                                    }
                                });
                            }
                        });
                    ui.add_space(3.0);
                }

                if let Some((id, delta)) = layer_to_move {
                    state.document.move_layer(id, delta);
                }

                ui.horizontal(|ui| {
                    if ui.button("＋ New").clicked() {
                        state.document.add_layer(None);
                    }
                    if ui.button("Duplicate").clicked() {
                        state.document.duplicate_active_layer();
                    }
                    if ui.button("🗑 Delete").clicked() {
                        state.document.delete_layer(state.document.active_layer_id);
                    }
                });

                ui.horizontal(|ui| {
                    if ui.button("⏬ Merge Down").clicked() {
                        state.document.merge_layer_down();
                    }
                    if ui.button("Merge Visible").clicked() {
                        state.document.merge_visible_layers();
                    }
                });

                ui.separator();
                // ── PALETTE & COLORS ──
                ui.label(RichText::new("COLOURS").size(9.0).strong().color(Color32::from_rgb(92, 106, 133)));

                ui.horizontal(|ui| {
                    let [pr, pg, pb, _] = state.brush.primary_color.to_rgba8();
                    let [sr, sg, sb, _] = state.brush.secondary_color.to_rgba8();

                    let prim_stroke = if !state.color_slot_is_secondary { 2.0_f32 } else { 0.5_f32 };
                    let prim_btn = egui::Button::new("")
                        .fill(Color32::from_rgb(pr, pg, pb))
                        .stroke(egui::Stroke::new(prim_stroke, accent_c32))
                        .min_size(Vec2::new(42.0, 32.0));

                    if ui.add(prim_btn).clicked() {
                        state.color_slot_is_secondary = false;
                    }

                    let sec_stroke = if state.color_slot_is_secondary { 2.0_f32 } else { 0.5_f32 };
                    let sec_btn = egui::Button::new("")
                        .fill(Color32::from_rgb(sr, sg, sb))
                        .stroke(egui::Stroke::new(sec_stroke, accent_c32))
                        .min_size(Vec2::new(42.0, 32.0));

                    if ui.add(sec_btn).clicked() {
                        state.color_slot_is_secondary = true;
                    }

                    if ui.button("⇄ Swap (X)").clicked() {
                        state.swap_colors();
                    }
                });

                // Color palette chips
                egui::Grid::new("palette_grid").num_columns(6).spacing([3.0, 3.0]).show(ui, |ui| {
                    for (i, &hex) in DEFAULT_PALETTE.iter().enumerate() {
                        if let Some(c) = Color::from_hex(hex) {
                            let [r, g, b, _] = c.to_rgba8();
                            let chip = egui::Button::new("")
                                .fill(Color32::from_rgb(r, g, b))
                                .min_size(Vec2::new(26.0, 20.0));
                            if ui.add(chip).clicked() {
                                if state.color_slot_is_secondary {
                                    state.brush.secondary_color = c;
                                } else {
                                    state.brush.primary_color = c;
                                }
                                state.push_color_history(c);
                            }
                        }
                        if i % 6 == 5 {
                            ui.end_row();
                        }
                    }
                });

                // RGB custom edit
                let active_color = if state.color_slot_is_secondary {
                    &mut state.brush.secondary_color
                } else {
                    &mut state.brush.primary_color
                };

                let mut rgba_arr = [active_color.r, active_color.g, active_color.b];
                if ui.color_edit_button_rgb(&mut rgba_arr).changed() {
                    active_color.r = rgba_arr[0];
                    active_color.g = rgba_arr[1];
                    active_color.b = rgba_arr[2];
                }

                ui.separator();
                // ── THEMES ──
                ui.label(RichText::new("THEME").size(9.0).strong().color(Color32::from_rgb(92, 106, 133)));
                ui.horizontal(|ui| {
                    if ui.selectable_label(state.document.theme == ThemeMode::DeepMist, "Deep Mist").clicked() {
                        state.document.theme = ThemeMode::DeepMist;
                    }
                    if ui.selectable_label(state.document.theme == ThemeMode::Moonlit, "Moonlit").clicked() {
                        state.document.theme = ThemeMode::Moonlit;
                    }
                    if ui.selectable_label(state.document.theme == ThemeMode::EmberGlow, "Ember Glow").clicked() {
                        state.document.theme = ThemeMode::EmberGlow;
                    }
                });

                ui.separator();
                // ── CANVAS SETTINGS ──
                ui.label(RichText::new("CANVAS").size(9.0).strong().color(Color32::from_rgb(92, 106, 133)));
                ui.checkbox(&mut state.document.is_transparent, "Transparent Background");

                ui.add(egui::Slider::new(&mut state.document.background_value, 0..=255).text("BG Brightness"));

                // Dynamic viewport centering
                let screen_size = ctx.screen_rect().size();
                if ui.button("🎯 Center & Reset View").clicked() {
                    state.reset_view_centered(screen_size.x, screen_size.y);
                }
            });
        });

    // ── FLOATING REFERENCE IMAGE WINDOW ──
    let mut show_ref = state.show_ref_window;
    if show_ref {
        egui::Window::new("🖼 Reference Image")
            .open(&mut show_ref)
            .default_size([280.0, 320.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("📁 Load Image...").clicked() {
                        state.pending_file_action = Some(PendingFileAction::OpenReferenceImage);
                    }
                    if state.reference_image.is_some() && ui.button("✕ Clear").clicked() {
                        state.reference_image = None;
                    }
                });

                if let Some((w, h, _)) = &state.reference_image {
                    ui.label(RichText::new(format!("Reference: {}×{}px", w, h)).size(10.0).color(accent_c32));
                    ui.label("Reference loaded. Use Eyedropper on canvas or dock to pick colors.");
                } else {
                    ui.label("No reference image loaded. Click 'Load Image...' to open an image file.");
                }
            });
        state.show_ref_window = show_ref;
    }

    // ── HELP DIALOG ──
    let mut show_help = state.show_help;
    if show_help {
        let mut should_close = false;
        egui::Window::new("Shortcuts & Studio Guide")
            .open(&mut show_help)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading("Hollow Canvas Quick Reference");
                ui.separator();
                ui.label("• Space + Drag: Pan the canvas smoothly");
                ui.label("• Mouse Wheel: Smooth Zoom in/out");
                ui.label("• B: Brush | E: Eraser | G: Flood Fill | I: Eyedropper | M: Select | V: Move");
                ui.label("• Alt + Click (Clone Tool): Set clone source coordinate");
                ui.label("• 📌 Ref Layer: Set a layer as Reference to flood fill with line-art detection");
                ui.label("• X: Swap Primary and Secondary Colors");
                ui.label("• Ctrl+Z: Undo | Ctrl+Y (or Ctrl+Shift+Z): Redo");
                ui.label("• Ctrl+S: Save Project | Ctrl+E: Export Flat PNG | Ctrl+O: Open Project");
                ui.label("• Symmetry: Toggle Horizontal, Vertical, Quad, or Mandala");
                ui.add_space(10.0);
                if ui.button("Close").clicked() {
                    should_close = true;
                }
            });
        state.show_help = if should_close { false } else { show_help };
    }
}
