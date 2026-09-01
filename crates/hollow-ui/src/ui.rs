use crate::icons::draw_tool_icon;
use crate::state::{ActiveFilterModal, AppState, CanvasPreset, PendingFileAction};
use egui::{Align, Color32, Layout, Rect, RichText, ScrollArea, Stroke, Vec2};
use hollow_core::brush::{EraserMode, GradientType, ShapeFillMode, ToolType};
use hollow_core::color::{Color, DEFAULT_PALETTE};
use hollow_core::symmetry::SymmetryMode;

/// Helper to render a custom studio tool button with a crisp vector-drawn icon
fn render_tool_button(
    ui: &mut egui::Ui,
    tool: ToolType,
    label: &str,
    is_selected: bool,
    accent_c32: Color32,
) -> bool {
    let size = Vec2::new(94.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let is_hovered = response.hovered();

        // Button background
        let bg_fill = if is_selected {
            Color32::from_rgba_unmultiplied(accent_c32.r(), accent_c32.g(), accent_c32.b(), 45)
        } else if is_hovered {
            Color32::from_rgba_unmultiplied(32, 44, 76, 220)
        } else {
            Color32::from_rgba_unmultiplied(20, 28, 48, 180)
        };

        // Button border
        let stroke = if is_selected {
            Stroke::new(1.5_f32, accent_c32)
        } else if is_hovered {
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(accent_c32.r(), accent_c32.g(), accent_c32.b(), 180))
        } else {
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(85, 105, 150, 45))
        };

        painter.rect(rect, 4.0, bg_fill, stroke);

        // Vector Icon Area (Left side)
        let icon_rect = Rect::from_min_size(rect.min + egui::vec2(4.0, 3.0), Vec2::new(20.0, 20.0));
        let icon_color = if is_selected {
            accent_c32
        } else if is_hovered {
            Color32::from_rgb(240, 245, 255)
        } else {
            Color32::from_rgb(180, 195, 225)
        };
        draw_tool_icon(painter, icon_rect, tool, icon_color, is_selected);

        // Text Label (Right side)
        let text_color = if is_selected {
            accent_c32
        } else if is_hovered {
            Color32::from_rgb(245, 248, 255)
        } else {
            Color32::from_rgb(195, 205, 228)
        };

        let text_pos = rect.min + egui::vec2(28.0, 5.0);
        painter.text(
            text_pos,
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(11.0),
            text_color,
        );
    }

    response.clicked()
}

pub fn render_ui(ctx: &egui::Context, state: &mut AppState) {
    let accent = state.document.theme.accent_color();
    let [ar, ag, ab, _] = accent.to_rgba8();
    let accent_c32 = Color32::from_rgb(ar, ag, ab);
    let accent_dim_c32 = Color32::from_rgba_unmultiplied(ar, ag, ab, 45);

    // Ensure reference texture is loaded if reference_image is present
    if state.ref_texture.is_none() {
        if let Some((w, h, raw_rgba)) = &state.reference_image {
            let color_image = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], raw_rgba);
            state.ref_texture = Some(ctx.load_texture("ref_image_texture", color_image, Default::default()));
        }
    }

    // ── 1. TOP STUDIO MENU BAR & HEADER ──
    if state.show_ui_panels {
        egui::TopBottomPanel::top("header_panel")
            .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(8, 12, 24, 252)).inner_margin(egui::Margin::symmetric(8.0, 3.5)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Left Brand & Version
                    ui.label(RichText::new("✦ HOLLOW CANVAS").size(12.5).strong().color(Color32::from_rgb(235, 242, 255)));
                    ui.label(RichText::new("v0.6.0").size(9.0).color(Color32::from_rgb(115, 130, 165)));

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);

                    // ── DROPDOWN MENUS ──
                    egui::menu::bar(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button("✦ New Canvas... (Ctrl+N)").clicked() {
                                state.show_new_canvas_dialog = true;
                                ui.close_menu();
                            }
                            if ui.button("📂 Open Project... (Ctrl+O)").clicked() {
                                state.pending_file_action = Some(PendingFileAction::OpenProject);
                                ui.close_menu();
                            }
                            if ui.button("💾 Save Project (Ctrl+S)").clicked() {
                                state.pending_file_action = Some(PendingFileAction::SaveProject);
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("📤 Export Flat PNG... (Ctrl+E)").clicked() {
                                state.pending_file_action = Some(PendingFileAction::ExportPng);
                                ui.close_menu();
                            }
                        });

                        ui.menu_button("Edit", |ui| {
                            if ui.button("↶ Undo (Ctrl+Z)").clicked() {
                                if let Some(desc) = state.history.undo(&mut state.document) {
                                    state.set_status(format!("Undo: {}", desc));
                                }
                                ui.close_menu();
                            }
                            if ui.button("↷ Redo (Ctrl+Y)").clicked() {
                                if let Some(desc) = state.history.redo(&mut state.document) {
                                    state.set_status(format!("Redo: {}", desc));
                                }
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("⇄ Swap Colors (X)").clicked() {
                                state.swap_colors();
                                ui.close_menu();
                            }
                        });

                        ui.menu_button("Select", |ui| {
                            if ui.button("Invert Selection (Ctrl+Shift+I)").clicked() {
                                if let Some(mask) = &mut state.selection {
                                    mask.invert();
                                    state.set_status("Selection inverted");
                                }
                                ui.close_menu();
                            }
                            if ui.button("✕ Deselect (Ctrl+D)").clicked() {
                                state.selection = None;
                                state.set_status("Deselected");
                                ui.close_menu();
                            }
                        });

                        ui.menu_button("Canvas", |ui| {
                            if ui.button("Resize / Scale Canvas...").clicked() {
                                state.show_resize_canvas_dialog = true;
                                state.resize_canvas_w = state.document.width;
                                state.resize_canvas_h = state.document.height;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Flip Horizontal").clicked() {
                                state.document.flip(true);
                                state.set_status("Flipped horizontally");
                                ui.close_menu();
                            }
                            if ui.button("Flip Vertical").clicked() {
                                state.document.flip(false);
                                state.set_status("Flipped vertically");
                                ui.close_menu();
                            }
                            if ui.button("Rotate 90° CW").clicked() {
                                state.document.rotate_90(true);
                                state.set_status("Rotated 90° CW");
                                ui.close_menu();
                            }
                            if ui.button("Rotate 180°").clicked() {
                                state.document.rotate_180();
                                state.set_status("Rotated 180°");
                                ui.close_menu();
                            }
                        });

                        ui.menu_button("Filter", |ui| {
                            ui.label(RichText::new("COLOR ADJUSTMENTS").size(8.5).strong().color(Color32::from_rgb(120, 135, 170)));
                            if ui.button("🎨 HSL Adjustments...").clicked() {
                                state.begin_filter_modal(ActiveFilterModal::Hsl);
                                ui.close_menu();
                            }
                            if ui.button("☀️ Brightness & Contrast...").clicked() {
                                state.begin_filter_modal(ActiveFilterModal::BrightnessContrast);
                                ui.close_menu();
                            }
                            if ui.button("⚖️ Color Balance...").clicked() {
                                state.begin_filter_modal(ActiveFilterModal::ColorBalance);
                                ui.close_menu();
                            }
                            if ui.button("🎚️ Posterize & Threshold...").clicked() {
                                state.begin_filter_modal(ActiveFilterModal::PosterizeThreshold);
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("🔄 Invert Colors (Ctrl+I)").clicked() {
                                let w = state.document.width;
                                let h = state.document.height;
                                let sel = state.selection.clone();
                                if let Some(layer) = state.document.active_layer_mut() {
                                    let before = layer.pixels.clone();
                                    hollow_core::filter::filter_invert(&mut layer.pixels, w, h, sel.as_ref());
                                    let cmd = Box::new(hollow_core::history::LayerPixelsSnapshotCommand {
                                        layer_id: layer.id,
                                        description: "Invert Colors",
                                        before_pixels: before,
                                        after_pixels: layer.pixels.clone(),
                                    });
                                    state.history.push(cmd);
                                    state.set_status("Inverted Colors");
                                }
                                ui.close_menu();
                            }
                            if ui.button("📷 Grayscale").clicked() {
                                let w = state.document.width;
                                let h = state.document.height;
                                let sel = state.selection.clone();
                                if let Some(layer) = state.document.active_layer_mut() {
                                    let before = layer.pixels.clone();
                                    hollow_core::filter::filter_grayscale(&mut layer.pixels, w, h, sel.as_ref());
                                    let cmd = Box::new(hollow_core::history::LayerPixelsSnapshotCommand {
                                        layer_id: layer.id,
                                        description: "Grayscale",
                                        before_pixels: before,
                                        after_pixels: layer.pixels.clone(),
                                    });
                                    state.history.push(cmd);
                                    state.set_status("Applied Grayscale");
                                }
                                ui.close_menu();
                            }
                            if ui.button("🎞️ Vintage Sepia").clicked() {
                                let w = state.document.width;
                                let h = state.document.height;
                                let sel = state.selection.clone();
                                if let Some(layer) = state.document.active_layer_mut() {
                                    let before = layer.pixels.clone();
                                    hollow_core::filter::filter_sepia(&mut layer.pixels, w, h, sel.as_ref());
                                    let cmd = Box::new(hollow_core::history::LayerPixelsSnapshotCommand {
                                        layer_id: layer.id,
                                        description: "Sepia",
                                        before_pixels: before,
                                        after_pixels: layer.pixels.clone(),
                                    });
                                    state.history.push(cmd);
                                    state.set_status("Applied Sepia Tone");
                                }
                                ui.close_menu();
                            }

                            ui.separator();
                            ui.label(RichText::new("ARTISTIC & BLUR FX").size(8.5).strong().color(Color32::from_rgb(120, 135, 170)));
                            if ui.button("💧 Gaussian Blur...").clicked() {
                                state.begin_filter_modal(ActiveFilterModal::GaussianBlur);
                                ui.close_menu();
                            }
                            if ui.button("🔪 Sharpen & Unsharp Mask...").clicked() {
                                state.begin_filter_modal(ActiveFilterModal::SharpenUnsharp);
                                ui.close_menu();
                            }
                            if ui.button("📺 Film Grain & Noise...").clicked() {
                                state.begin_filter_modal(ActiveFilterModal::FilmGrain);
                                ui.close_menu();
                            }
                            if ui.button("🔍 Vignette & Lens FX...").clicked() {
                                state.begin_filter_modal(ActiveFilterModal::VignetteChromatic);
                                ui.close_menu();
                            }
                        });

                        ui.menu_button("View", |ui| {
                            if ui.checkbox(&mut state.show_grid, "⊞ Toggle Grid (Ctrl+')").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.show_rulers, "📏 Toggle Rulers (Ctrl+R)").clicked() {
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.checkbox(&mut state.show_ui_panels, "👁 Studio Panels (Tab)").clicked() {
                                ui.close_menu();
                            }
                            if ui.button("Reset View (100%)").clicked() {
                                state.reset_view();
                                ui.close_menu();
                            }
                        });

                        ui.menu_button("Window", |ui| {
                            if ui.checkbox(&mut state.show_ref_window, "🖼 Reference Viewer").clicked() {
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("❓ Shortcuts & Help (?)").clicked() {
                                state.show_help = true;
                                ui.close_menu();
                            }
                            if ui.button("ℹ About Hollow Canvas").clicked() {
                                state.show_about_dialog = true;
                                ui.close_menu();
                            }
                        });
                    });

                    // Right-aligned ultra-compact quick action icon bar (Never Overflows)
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("ℹ").on_hover_text("About Hollow Canvas").clicked() {
                            state.show_about_dialog = true;
                        }

                        if ui.button("?").on_hover_text("Shortcuts & Help (?)").clicked() {
                            state.show_help = !state.show_help;
                        }

                        if ui.selectable_label(!state.show_ui_panels, "👁").on_hover_text("Toggle Full Canvas / Zen Mode (Tab)").clicked() {
                            state.show_ui_panels = !state.show_ui_panels;
                        }

                        let ref_label = if state.tracing_enabled {
                            "📐 Trace [ON]"
                        } else if state.show_ref_window {
                            "🖼 Ref [ON]"
                        } else {
                            "🖼 Ref"
                        };
                        if ui.selectable_label(state.show_ref_window || state.tracing_enabled, ref_label).on_hover_text("Reference & On-Canvas Tracing Paper Studio").clicked() {
                            state.show_ref_window = !state.show_ref_window;
                        }

                        let ruler_label = if state.show_rulers { "📏 [ON]" } else { "📏 Rulers" };
                        if ui.selectable_label(state.show_rulers, ruler_label).on_hover_text("Toggle Viewport Rulers (Ctrl+R)").clicked() {
                            state.show_rulers = !state.show_rulers;
                        }

                        let grid_label = if state.show_grid { "⊞ [ON]" } else { "⊞ Grid" };
                        if ui.selectable_label(state.show_grid, grid_label).on_hover_text("Toggle Canvas Grid (Ctrl+')").clicked() {
                            state.show_grid = !state.show_grid;
                        }

                        ui.separator();

                        // Centered Active Tool Badge in remaining space
                        let tool_label = format!("{} · {}px", state.brush.tool.label(), state.brush.size as u32);
                        egui::Frame::none()
                            .fill(accent_dim_c32)
                            .stroke(egui::Stroke::new(1.0_f32, accent_c32))
                            .rounding(4.0)
                            .inner_margin(egui::vec2(8.0, 2.5))
                            .show(ui, |ui| {
                                ui.label(RichText::new(tool_label).size(10.0).strong().color(accent_c32));
                            });
                    });
                });
            });
    } else {
        // Floating Restore Button when UI panels are hidden in Zen Mode
        egui::Area::new("zen_restore_area".into())
            .fixed_pos(egui::pos2(12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(8, 12, 24, 230))
                    .stroke(egui::Stroke::new(1.0_f32, accent_c32))
                    .rounding(5.0)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("👁 Show Studio Panels (Tab)").strong().color(accent_c32)).clicked() {
                                state.show_ui_panels = true;
                            }
                            if ui.selectable_label(state.show_grid, "⊞ Grid").clicked() {
                                state.show_grid = !state.show_grid;
                            }
                        });
                    });
            });
    }

    // ── 2. BOTTOM STATUS BAR ──
    if state.show_ui_panels {
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(5, 7, 15, 252)).inner_margin(4.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("●").size(8.0).color(accent_c32));
                    ui.label(RichText::new(&state.status_message).size(10.5).color(Color32::from_rgb(170, 182, 210)));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} × {} px", state.document.width, state.document.height))
                                .size(10.0)
                                .color(Color32::from_rgb(130, 142, 172)),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(format!(
                                "X: {}  Y: {}",
                                state.cursor_canvas_pos.x.round() as i32,
                                state.cursor_canvas_pos.y.round() as i32
                            ))
                            .size(10.0)
                            .color(Color32::from_rgb(130, 142, 172)),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(format!("Zoom: {}%", (state.zoom * 100.0).round() as i32))
                                .size(10.0)
                                .strong()
                                .color(accent_c32),
                        );
                    });
                });
            });
    }

    // ── 3. LEFT SIDEBAR (TOOLS & BRUSH PROPERTIES) ──
    if state.show_ui_panels {
        egui::SidePanel::left("left_tools_panel")
            .default_width(220.0)
            .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(10, 14, 26, 245)).inner_margin(8.0))
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new("STUDIO TOOLS").size(9.5).strong().color(Color32::from_rgb(110, 125, 158)));

                    let tools = [
                        (ToolType::Brush, "Brush"),
                        (ToolType::Pencil, "Pencil"),
                        (ToolType::Watercolor, "Water"),
                        (ToolType::Chalk, "Chalk"),
                        (ToolType::Spray, "Spray"),
                        (ToolType::Smudge, "Smudge"),
                        (ToolType::Gradient, "Gradient"),
                        (ToolType::Wand, "Wand"),
                        (ToolType::Eraser, "Eraser"),
                        (ToolType::Fill, "Fill"),
                        (ToolType::Line, "Line"),
                        (ToolType::Rect, "Rect"),
                        (ToolType::Ellipse, "Oval"),
                        (ToolType::Polygon, "Poly"),
                        (ToolType::Marquee, "Select"),
                        (ToolType::Move, "Move"),
                        (ToolType::Crop, "Crop"),
                        (ToolType::Eyedropper, "Pick"),
                    ];

                    egui::Grid::new("tools_grid").num_columns(2).spacing([6.0, 5.0]).show(ui, |ui| {
                        for (i, (t, label)) in tools.iter().enumerate() {
                            let is_selected = state.brush.tool == *t;
                            if render_tool_button(ui, *t, label, is_selected, accent_c32) {
                                state.brush.tool = *t;
                                state.set_status(format!("Tool: {}", t.label()));
                            }
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();

                    // Tool-specific properties
                    ui.label(RichText::new("TOOL PROPERTIES").size(9.5).strong().color(Color32::from_rgb(110, 125, 158)));

                    match state.brush.tool {
                        ToolType::Gradient => {
                            ui.horizontal(|ui| {
                                ui.label("Type:");
                                for &gt in GradientType::ALL {
                                    if ui.selectable_label(state.brush.gradient_type == gt, gt.label()).clicked() {
                                        state.brush.gradient_type = gt;
                                    }
                                }
                            });
                            ui.checkbox(&mut state.brush.gradient_dither, "Dither Smoothing");
                            ui.label(RichText::new("Drag on canvas: blends Primary to Secondary").size(9.0).color(Color32::from_rgb(130, 142, 172)));
                        }
                        ToolType::Wand => {
                            ui.add(egui::Slider::new(&mut state.wand_tolerance, 0..=128).text("Tolerance"));
                            ui.checkbox(&mut state.wand_contiguous, "Contiguous Region");
                            ui.checkbox(&mut state.wand_sample_all_layers, "Sample All Layers");
                            let mut do_invert = false;
                            let mut do_deselect = false;
                            if let Some(mask) = &state.selection {
                                if mask.has_selection() {
                                    ui.horizontal(|ui| {
                                        if ui.button("Invert").clicked() {
                                            do_invert = true;
                                        }
                                        if ui.button("Deselect").clicked() {
                                            do_deselect = true;
                                        }
                                    });
                                }
                            }
                            if do_invert {
                                if let Some(mask) = &mut state.selection {
                                    mask.invert();
                                    state.set_status("Selection inverted");
                                }
                            }
                            if do_deselect {
                                state.selection = None;
                                state.set_status("Deselected");
                            }
                        }
                        ToolType::Eraser => {
                            ui.horizontal(|ui| {
                                ui.label("Mode:");
                                for &em in EraserMode::ALL {
                                    if ui.selectable_label(state.brush.eraser_mode == em, em.label()).clicked() {
                                        state.brush.eraser_mode = em;
                                    }
                                }
                            });
                            if state.brush.eraser_mode == EraserMode::ColorErase {
                                ui.add(egui::Slider::new(&mut state.brush.color_erase_tolerance, 0..=128).text("Color Tol"));
                                ui.label(RichText::new("Erases pixels matching Secondary color").size(9.0).color(Color32::from_rgb(130, 142, 172)));
                            }
                        }
                        ToolType::Line | ToolType::Rect | ToolType::Ellipse | ToolType::Polygon => {
                            ui.horizontal(|ui| {
                                ui.label("Fill Mode:");
                                ui.selectable_value(&mut state.brush.shape_fill_mode, ShapeFillMode::Stroke, "Outline");
                                ui.selectable_value(&mut state.brush.shape_fill_mode, ShapeFillMode::Fill, "Fill");
                                ui.selectable_value(&mut state.brush.shape_fill_mode, ShapeFillMode::Both, "Both");
                            });
                            if state.brush.shape_fill_mode == ShapeFillMode::Both {
                                ui.label(RichText::new("Outline: Primary · Fill: Secondary").size(9.0).color(accent_c32));
                            }
                        }
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
                        ToolType::Fill => {
                            ui.add(egui::Slider::new(&mut state.wand_tolerance, 0..=128).text("Tolerance"));
                        }
                        ToolType::Marquee => {
                            let mut do_invert = false;
                            let mut do_deselect = false;
                            if let Some(mask) = &state.selection {
                                if mask.has_selection() {
                                    ui.horizontal(|ui| {
                                        if ui.button("Invert").clicked() {
                                            do_invert = true;
                                        }
                                        if ui.button("✕ Deselect").clicked() {
                                            do_deselect = true;
                                        }
                                    });
                                }
                            } else {
                                ui.label(RichText::new("Drag on canvas to select area").size(9.0).color(Color32::from_rgb(130, 142, 172)));
                            }
                            if do_invert {
                                if let Some(mask) = &mut state.selection {
                                    mask.invert();
                                }
                            }
                            if do_deselect {
                                state.selection = None;
                                state.set_status("Deselected");
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
                                            let w = max_x.saturating_sub(min_x);
                                            let h = max_y.saturating_sub(min_y);
                                            if w > 10 && h > 10 {
                                                state.document.resize_canvas(w, h, -(min_x as i32), -(min_y as i32));
                                                state.crop_box = None;
                                                state.brush.tool = ToolType::Brush;
                                                state.set_status(format!("Cropped to {}×{}", w, h));
                                            }
                                        }
                                    }
                                    if ui.button("Cancel").clicked() {
                                        state.crop_box = None;
                                    }
                                });
                            } else {
                                ui.label(RichText::new("Drag box to crop canvas").size(9.0).color(Color32::from_rgb(130, 142, 172)));
                            }
                        }
                        _ => {}
                    }

                    ui.add_space(6.0);
                    ui.label(RichText::new("BRUSH DYNAMICS").size(9.5).strong().color(Color32::from_rgb(110, 125, 158)));
                    ui.add(egui::Slider::new(&mut state.brush.size, 1.0..=180.0).text("Size"));
                    ui.add(egui::Slider::new(&mut state.brush.opacity, 0.02..=1.0).text("Opacity"));
                    ui.add(egui::Slider::new(&mut state.brush.smoothing, 0.0..=0.95).text("Smoothing"));
                    ui.add(egui::Slider::new(&mut state.brush.hardness, 0.05..=1.0).text("Hardness"));
                    ui.add(egui::Slider::new(&mut state.brush.spacing, 0.05..=1.0).text("Spacing"));

                    ui.add_space(8.0);
                    ui.separator();
                    ui.label(RichText::new("SYMMETRY & GRID").size(9.5).strong().color(Color32::from_rgb(110, 125, 158)));
                    ui.horizontal(|ui| {
                        let modes = [
                            (SymmetryMode::None, "None"),
                            (SymmetryMode::Horizontal, "Horiz"),
                            (SymmetryMode::Vertical, "Vert"),
                            (SymmetryMode::Quad, "Quad"),
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

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.show_grid, "Canvas Grid");
                    });
                    if state.show_grid {
                        ui.add(egui::Slider::new(&mut state.grid_size, 8..=128).text("Cell Size"));
                        ui.add(egui::Slider::new(&mut state.grid_opacity, 0.05..=0.8).text("Grid Alpha"));
                    }
                });
            });
    }

    // ── 4. RIGHT SIDEBAR (LAYERS, COLORS & CANVAS CONTROLS) ──
    if state.show_ui_panels {
        egui::SidePanel::right("right_layers_panel")
            .default_width(240.0)
            .frame(egui::Frame::none().fill(Color32::from_rgba_unmultiplied(10, 14, 26, 245)).inner_margin(8.0))
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    // ── PALETTE & COLOR PICKER ──
                    ui.label(RichText::new("COLOR PALETTE").size(9.5).strong().color(Color32::from_rgb(110, 125, 158)));

                    ui.horizontal(|ui| {
                        let [pr, pg, pb, _] = state.brush.primary_color.to_rgba8();
                        let [sr, sg, sb, _] = state.brush.secondary_color.to_rgba8();

                        let prim_btn = egui::Button::new("").min_size(Vec2::new(32.0, 32.0)).fill(Color32::from_rgb(pr, pg, pb));
                        if ui.add(prim_btn).on_hover_text("Primary Color").clicked() {
                            state.color_slot_is_secondary = false;
                        }

                        let sec_btn = egui::Button::new("").min_size(Vec2::new(32.0, 32.0)).fill(Color32::from_rgb(sr, sg, sb));
                        if ui.add(sec_btn).on_hover_text("Secondary Color").clicked() {
                            state.color_slot_is_secondary = true;
                        }

                        if ui.button("⇄ Swap (X)").clicked() {
                            state.swap_colors();
                        }
                    });

                    // Target color to edit
                    let current_color = if state.color_slot_is_secondary {
                        state.brush.secondary_color
                    } else {
                        state.brush.primary_color
                    };

                    let mut col_array = [current_color.r, current_color.g, current_color.b];
                    if ui.color_edit_button_rgb(&mut col_array).changed() {
                        let new_col = Color::new(col_array[0], col_array[1], col_array[2], 1.0);
                        if state.color_slot_is_secondary {
                            state.brush.secondary_color = new_col;
                        } else {
                            state.brush.primary_color = new_col;
                        }
                        state.push_color_history(new_col);
                    }

                    // Swatches
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for &hex in DEFAULT_PALETTE {
                            if let Some(c) = Color::from_hex(hex) {
                                let [r, g, b, _] = c.to_rgba8();
                                let btn = egui::Button::new("").min_size(Vec2::new(14.0, 14.0)).fill(Color32::from_rgb(r, g, b));
                                if ui.add(btn).clicked() {
                                    if state.color_slot_is_secondary {
                                        state.brush.secondary_color = c;
                                    } else {
                                        state.brush.primary_color = c;
                                    }
                                    state.push_color_history(c);
                                }
                            }
                        }
                    });

                    ui.add_space(10.0);
                    ui.separator();

                    // ── LAYERS PANEL ──
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("LAYERS").size(9.5).strong().color(Color32::from_rgb(110, 125, 158)));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("➕ Add Layer").clicked() {
                                let id = state.document.add_layer(None);
                                state.set_status(format!("Added Layer {}", id));
                            }
                        });
                    });

                    let active_id = state.document.active_layer_id;
                    let layer_count = state.document.layers.len();

                    // Render layers in reverse order (top to bottom)
                    for i in (0..layer_count).rev() {
                        let is_active = state.document.layers[i].id == active_id;
                        let layer_id = state.document.layers[i].id;
                        let is_ref = state.document.layers[i].is_reference;
                        let is_clip = state.document.layers[i].clipping_mask;

                        egui::Frame::none()
                            .fill(if is_active { Color32::from_rgba_unmultiplied(ar, ag, ab, 42) } else { Color32::from_rgba_unmultiplied(18, 25, 46, 180) })
                            .stroke(egui::Stroke::new(1.0_f32, if is_active { accent_c32 } else { Color32::from_rgb(36, 48, 78) }))
                            .rounding(4.0)
                            .inner_margin(5.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let vis_icon = if state.document.layers[i].visible { "👁" } else { "⊘" };
                                    if ui.button(vis_icon).clicked() {
                                        state.document.layers[i].visible = !state.document.layers[i].visible;
                                    }

                                    if is_clip {
                                        ui.label(RichText::new("⮑").size(11.0).color(accent_c32));
                                    }
                                    if is_ref {
                                        ui.label(RichText::new("⭐").size(10.0));
                                    }

                                    // Editable Layer Name
                                    let name_edit = egui::TextEdit::singleline(&mut state.document.layers[i].name)
                                        .desired_width(85.0);
                                    let response = ui.add(name_edit);
                                    if response.clicked() || response.gained_focus() {
                                        state.document.active_layer_id = layer_id;
                                    }

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.button("🗑").on_hover_text("Delete Layer").clicked() {
                                            state.document.delete_layer(layer_id);
                                        }
                                        if ui.button("⎘").on_hover_text("Duplicate Layer").clicked() {
                                            state.document.duplicate_active_layer();
                                        }
                                    });
                                });

                                if is_active {
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Slider::new(&mut state.document.layers[i].opacity, 0.0..=1.0).text("Opacity"));
                                    });
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut state.document.layers[i].alpha_locked, "🔒α Lock");
                                        ui.checkbox(&mut state.document.layers[i].clipping_mask, "⮑ Clip");
                                        ui.checkbox(&mut state.document.layers[i].locked, "🔒");
                                    });
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut state.document.layers[i].is_reference, "Ref Boundary");
                                    });
                                }
                            });
                        ui.add_space(3.0);
                    }

                    ui.add_space(8.0);
                    ui.separator();

                    // ── REAL-TIME CANVAS OPERATIONS ──
                    ui.label(RichText::new("CANVAS OPERATIONS").size(9.5).strong().color(Color32::from_rgb(110, 125, 158)));

                    ui.horizontal(|ui| {
                        if ui.button("Resize / Scale...").clicked() {
                            state.show_resize_canvas_dialog = true;
                            state.resize_canvas_w = state.document.width;
                            state.resize_canvas_h = state.document.height;
                        }
                        if ui.button("Reset View (100%)").clicked() {
                            state.reset_view();
                        }
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Flip H").clicked() {
                            state.document.flip(true);
                            state.set_status("Flipped horizontally");
                        }
                        if ui.button("Flip V").clicked() {
                            state.document.flip(false);
                            state.set_status("Flipped vertically");
                        }
                        if ui.button("⟳ 90°").clicked() {
                            state.document.rotate_90(true);
                            state.set_status("Rotated 90° CW");
                        }
                        if ui.button("180°").clicked() {
                            state.document.rotate_180();
                            state.set_status("Rotated 180°");
                        }
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.document.is_transparent, "Transparent Canvas");
                    });
                    if !state.document.is_transparent {
                        ui.add(egui::Slider::new(&mut state.document.background_value, 0..=255).text("BG Lum"));
                    }
                });
            });
    }

    // ── 5. NEW CANVAS MODAL DIALOG ──
    if state.show_new_canvas_dialog {
        egui::Window::new("✦ Create New Canvas")
            .fixed_size(Vec2::new(480.0, 360.0))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(RichText::new("Choose a preset or configure custom canvas dimensions:").size(11.0).color(Color32::from_rgb(170, 182, 210)));
                ui.add_space(6.0);

                ui.label(RichText::new("PRESETS").size(9.5).strong().color(accent_c32));
                ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                    for (idx, preset) in CanvasPreset::ALL.iter().enumerate() {
                        let text = format!("{}: {} ({}×{})", preset.category, preset.name, preset.width, preset.height);
                        if ui.selectable_label(state.new_canvas_preset_idx == idx, text).clicked() {
                            state.new_canvas_preset_idx = idx;
                            state.new_canvas_w = preset.width;
                            state.new_canvas_h = preset.height;
                            state.new_canvas_aspect_ratio = preset.width as f32 / preset.height as f32;
                        }
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.label(RichText::new("CUSTOM DIMENSIONS").size(9.5).strong().color(accent_c32));

                ui.horizontal(|ui| {
                    ui.label("Width:");
                    let mut w_str = state.new_canvas_w.to_string();
                    if ui.add(egui::TextEdit::singleline(&mut w_str).desired_width(60.0)).changed() {
                        if let Ok(w) = w_str.parse::<u32>() {
                            state.new_canvas_w = w.clamp(16, 8192);
                            if state.new_canvas_lock_aspect && state.new_canvas_aspect_ratio > 0.0 {
                                state.new_canvas_h = ((w as f32) / state.new_canvas_aspect_ratio).round().clamp(16.0, 8192.0) as u32;
                            }
                        }
                    }
                    ui.label("px");

                    ui.add_space(10.0);
                    ui.label("Height:");
                    let mut h_str = state.new_canvas_h.to_string();
                    if ui.add(egui::TextEdit::singleline(&mut h_str).desired_width(60.0)).changed() {
                        if let Ok(h) = h_str.parse::<u32>() {
                            state.new_canvas_h = h.clamp(16, 8192);
                            if state.new_canvas_lock_aspect && state.new_canvas_aspect_ratio > 0.0 {
                                state.new_canvas_w = ((h as f32) * state.new_canvas_aspect_ratio).round().clamp(16.0, 8192.0) as u32;
                            }
                        }
                    }
                    ui.label("px");

                    ui.add_space(10.0);
                    ui.checkbox(&mut state.new_canvas_lock_aspect, "Lock Ratio 🔒");
                    if ui.button("⇄ Swap W/H").clicked() {
                        let tmp = state.new_canvas_w;
                        state.new_canvas_w = state.new_canvas_h;
                        state.new_canvas_h = tmp;
                        state.new_canvas_aspect_ratio = state.new_canvas_w as f32 / state.new_canvas_h as f32;
                    }
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Background:");
                    ui.selectable_value(&mut state.new_canvas_bg_mode, 0, "Dark Studio");
                    ui.selectable_value(&mut state.new_canvas_bg_mode, 1, "Pure White");
                    ui.selectable_value(&mut state.new_canvas_bg_mode, 2, "Transparent");
                });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("✦ Create Canvas").strong().color(accent_c32)).clicked() {
                        state.pending_file_action = Some(PendingFileAction::NewCanvas(
                            state.new_canvas_w,
                            state.new_canvas_h,
                            state.new_canvas_bg_mode,
                        ));
                        state.show_new_canvas_dialog = false;
                    }

                    if ui.button("Cancel").clicked() {
                        state.show_new_canvas_dialog = false;
                    }
                });
            });
    }

    // ── 6. RESIZE / SCALE CANVAS MODAL ──
    if state.show_resize_canvas_dialog {
        egui::Window::new("Resize / Scale Canvas")
            .fixed_size(Vec2::new(380.0, 240.0))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("New Width:");
                    let mut w_str = state.resize_canvas_w.to_string();
                    if ui.add(egui::TextEdit::singleline(&mut w_str).desired_width(60.0)).changed() {
                        if let Ok(w) = w_str.parse::<u32>() {
                            state.resize_canvas_w = w.clamp(16, 8192);
                        }
                    }
                    ui.label("px");

                    ui.label("New Height:");
                    let mut h_str = state.resize_canvas_h.to_string();
                    if ui.add(egui::TextEdit::singleline(&mut h_str).desired_width(60.0)).changed() {
                        if let Ok(h) = h_str.parse::<u32>() {
                            state.resize_canvas_h = h.clamp(16, 8192);
                        }
                    }
                    ui.label("px");
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.resize_scale_mode, false, "Canvas Crop / Extend");
                    ui.selectable_value(&mut state.resize_scale_mode, true, "Resample Scale Image");
                });

                if state.resize_scale_mode {
                    ui.checkbox(&mut state.resize_bilinear, "Bilinear Smoothing (vs Nearest Neighbor)");
                } else {
                    ui.checkbox(&mut state.resize_anchor_center, "Anchor Center (vs Top-Left)");
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Apply Changes").strong().color(accent_c32)).clicked() {
                        let (nw, nh) = (state.resize_canvas_w, state.resize_canvas_h);
                        if nw > 0 && nh > 0 {
                            if state.resize_scale_mode {
                                state.document.scale_canvas(nw, nh, state.resize_bilinear);
                                state.set_status(format!("Resampled canvas to {}×{}", nw, nh));
                            } else {
                                let (ox, oy) = if state.resize_anchor_center {
                                    (
                                        (nw as i32 - state.document.width as i32) / 2,
                                        (nh as i32 - state.document.height as i32) / 2,
                                    )
                                } else {
                                    (0, 0)
                                };
                                state.document.resize_canvas(nw, nh, ox, oy);
                                state.set_status(format!("Resized canvas bounds to {}×{}", nw, nh));
                            }
                        }
                        state.show_resize_canvas_dialog = false;
                    }

                    if ui.button("Cancel").clicked() {
                        state.show_resize_canvas_dialog = false;
                    }
                });
            });
    }

    // ── 7. STUDIO DUAL REFERENCE & TRACING SYSTEM DOCK ──
    if state.show_ref_window {
        egui::Window::new("🖼 Reference & Tracing Studio")
            .default_size(Vec2::new(420.0, 440.0))
            .show(ctx, |ui| {
                // Mode Switcher Segmented Bar
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Mode:").strong().color(Color32::from_rgb(150, 165, 195)));
                    ui.selectable_value(&mut state.reference_mode, crate::state::ReferenceMode::CanvasTracing, "📐 Canvas Tracing Paper");
                    ui.selectable_value(&mut state.reference_mode, crate::state::ReferenceMode::FloatingWindow, "🖼 Floating Lightbox");
                });

                ui.separator();

                // Top Actions: Load Image & Status
                ui.horizontal(|ui| {
                    if ui.button("📂 Load Image...").clicked() {
                        state.pending_file_action = Some(PendingFileAction::OpenReferenceImage);
                    }
                    if let Some((w, h, _)) = state.reference_image {
                        ui.label(RichText::new(format!("{} × {} px", w, h)).size(10.0).color(accent_c32));
                    }
                });

                ui.add_space(4.0);

                if let Some((w, h, _)) = state.reference_image {
                    match state.reference_mode {
                        crate::state::ReferenceMode::CanvasTracing => {
                            egui::Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(14, 20, 38, 220))
                                .stroke(egui::Stroke::new(1.0_f32, accent_c32))
                                .rounding(6.0)
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut state.tracing_enabled, RichText::new("✨ Enable On-Canvas Tracing").strong().color(accent_c32));
                                        ui.checkbox(&mut state.tracing_locked, "🔒 Lock Position");
                                    });

                                    ui.add_space(4.0);
                                    ui.add(egui::Slider::new(&mut state.tracing_opacity, 0.05..=1.0).text("Tracing Opacity (Ghosting)"));

                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Placement:");
                                        ui.selectable_value(&mut state.tracing_as_underlay, true, "💡 Underlay (Light Table)");
                                        ui.selectable_value(&mut state.tracing_as_underlay, false, "👻 Ghost Overlay");
                                    });

                                    ui.add_space(6.0);
                                    ui.separator();
                                    ui.label(RichText::new("TRANSFORM & POSITION").size(9.0).strong().color(Color32::from_rgb(120, 135, 170)));

                                    ui.add(egui::Slider::new(&mut state.tracing_scale, 0.05..=4.0).text("Scale"));

                                    ui.horizontal(|ui| {
                                        if ui.button("✦ Fit to Canvas").clicked() {
                                            state.fit_tracing_to_canvas();
                                        }
                                        if ui.button("✛ Center Canvas").clicked() {
                                            state.center_tracing_on_canvas();
                                        }
                                        if ui.button("1:1 Native").clicked() {
                                            state.tracing_scale = 1.0;
                                            state.tracing_pos = glam::Vec2::ZERO;
                                        }
                                    });

                                    let doc_w_f = state.document.width as f32;
                                    let doc_h_f = state.document.height as f32;
                                    ui.add(egui::Slider::new(&mut state.tracing_pos.x, -doc_w_f..=doc_w_f).text("Offset X (px)"));
                                    ui.add(egui::Slider::new(&mut state.tracing_pos.y, -doc_h_f..=doc_h_f).text("Offset Y (px)"));
                                });
                        }

                        crate::state::ReferenceMode::FloatingWindow => {
                            ui.horizontal(|ui| {
                                if ui.selectable_label(state.ref_backlight, "💡 Backlight").clicked() {
                                    state.ref_backlight = !state.ref_backlight;
                                }
                                if state.ref_backlight {
                                    ui.label("Mode:");
                                    ui.selectable_value(&mut state.ref_backlight_mode, 0, "Dark");
                                    ui.selectable_value(&mut state.ref_backlight_mode, 1, "White");
                                    ui.selectable_value(&mut state.ref_backlight_mode, 2, "Checker");
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.add(egui::Slider::new(&mut state.ref_zoom, 0.1..=4.0).text("Zoom"));
                                if ui.button("1:1").clicked() {
                                    state.ref_zoom = 1.0;
                                }
                            });

                            let bg_color = match state.ref_backlight_mode {
                                1 => Color32::from_rgb(250, 250, 250), // Pure White Lightbox
                                2 => Color32::from_rgb(120, 120, 120), // Neutral Checker / Gray
                                _ => Color32::from_rgb(10, 14, 25),    // Dark studio
                            };

                            egui::Frame::none()
                                .fill(bg_color)
                                .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(40, 50, 80)))
                                .rounding(4.0)
                                .inner_margin(4.0)
                                .show(ui, |ui| {
                                    ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                                        if let Some(tex) = &state.ref_texture {
                                            let img_size = Vec2::new(w as f32 * state.ref_zoom, h as f32 * state.ref_zoom);
                                            ui.image((tex.id(), img_size));
                                        }
                                    });
                                });
                        }
                    }
                } else {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No reference image loaded.").size(12.0).color(Color32::from_rgb(180, 190, 220)));
                        ui.add_space(4.0);
                        ui.label(RichText::new("Click '📂 Load Image...' to load a character sketch, lineart, anatomy reference, or color moodboard.").size(10.0).color(Color32::from_rgb(130, 142, 172)));
                    });
                }
            });
    }

    // ── 8. STUDIO ADJUSTMENTS & FILTER MODAL DIALOGS ──
    let doc_w = state.document.width;
    let doc_h = state.document.height;
    let sel_mask = state.selection.clone();

    match state.active_filter_modal {
        ActiveFilterModal::Hsl => {
            let mut do_apply = false;
            let mut do_cancel = false;
            let mut changed = false;

            egui::Window::new("🎨 HSL Adjustments")
                .fixed_size(Vec2::new(380.0, 220.0))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    changed |= ui.add(egui::Slider::new(&mut state.filter_hue_shift, -180.0..=180.0).text("Hue (°)")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_saturation_scale, 0.0..=2.5).text("Saturation")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_lightness_shift, -0.8..=0.8).text("Lightness")).changed();

                    ui.add_space(8.0);
                    ui.checkbox(&mut state.filter_preview_active, "Live Preview");

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("✓ Apply").strong().color(accent_c32)).clicked() {
                            do_apply = true;
                        }
                        if ui.button("Cancel").clicked() {
                            do_cancel = true;
                        }
                    });
                });

            if changed || state.filter_preview_active {
                if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                    layer.pixels = orig.clone();
                    if state.filter_preview_active {
                        hollow_core::filter::adjust_hsl(
                            &mut layer.pixels,
                            doc_w,
                            doc_h,
                            state.filter_hue_shift,
                            state.filter_saturation_scale,
                            state.filter_lightness_shift,
                            sel_mask.as_ref(),
                        );
                    }
                }
            }

            if do_apply {
                state.apply_filter_modal("HSL Adjustments");
            } else if do_cancel {
                state.cancel_filter_modal();
            }
        }

        ActiveFilterModal::BrightnessContrast => {
            let mut do_apply = false;
            let mut do_cancel = false;
            let mut changed = false;

            egui::Window::new("☀️ Brightness & Contrast")
                .fixed_size(Vec2::new(380.0, 200.0))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    changed |= ui.add(egui::Slider::new(&mut state.filter_brightness, -80.0..=80.0).text("Brightness")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_contrast, -80.0..=80.0).text("Contrast")).changed();

                    ui.add_space(8.0);
                    ui.checkbox(&mut state.filter_preview_active, "Live Preview");

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("✓ Apply").strong().color(accent_c32)).clicked() {
                            do_apply = true;
                        }
                        if ui.button("Cancel").clicked() {
                            do_cancel = true;
                        }
                    });
                });

            if changed || state.filter_preview_active {
                if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                    layer.pixels = orig.clone();
                    if state.filter_preview_active {
                        hollow_core::filter::adjust_brightness_contrast(
                            &mut layer.pixels,
                            doc_w,
                            doc_h,
                            state.filter_brightness,
                            state.filter_contrast,
                            sel_mask.as_ref(),
                        );
                    }
                }
            }

            if do_apply {
                state.apply_filter_modal("Brightness & Contrast");
            } else if do_cancel {
                state.cancel_filter_modal();
            }
        }

        ActiveFilterModal::ColorBalance => {
            let mut do_apply = false;
            let mut do_cancel = false;
            let mut changed = false;

            egui::Window::new("⚖️ Color Balance")
                .fixed_size(Vec2::new(400.0, 240.0))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    changed |= ui.add(egui::Slider::new(&mut state.filter_cyan_red, -80.0..=80.0).text("Cyan / Red")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_magenta_green, -80.0..=80.0).text("Magenta / Green")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_yellow_blue, -80.0..=80.0).text("Yellow / Blue")).changed();

                    ui.add_space(8.0);
                    ui.checkbox(&mut state.filter_preview_active, "Live Preview");

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("✓ Apply").strong().color(accent_c32)).clicked() {
                            do_apply = true;
                        }
                        if ui.button("Cancel").clicked() {
                            do_cancel = true;
                        }
                    });
                });

            if changed || state.filter_preview_active {
                if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                    layer.pixels = orig.clone();
                    if state.filter_preview_active {
                        hollow_core::filter::adjust_color_balance(
                            &mut layer.pixels,
                            doc_w,
                            doc_h,
                            state.filter_cyan_red,
                            state.filter_magenta_green,
                            state.filter_yellow_blue,
                            sel_mask.as_ref(),
                        );
                    }
                }
            }

            if do_apply {
                state.apply_filter_modal("Color Balance");
            } else if do_cancel {
                state.cancel_filter_modal();
            }
        }

        ActiveFilterModal::PosterizeThreshold => {
            let mut do_apply = false;
            let mut do_cancel = false;
            let mut changed = false;

            egui::Window::new("🎚️ Posterize & Threshold")
                .fixed_size(Vec2::new(380.0, 220.0))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    changed |= ui.add(egui::Slider::new(&mut state.filter_posterize_levels, 2..=24).text("Posterize Levels")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_threshold_val, 1..=254).text("B&W Threshold")).changed();

                    ui.add_space(8.0);
                    ui.checkbox(&mut state.filter_preview_active, "Live Preview");

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Apply Posterize").strong().color(accent_c32)).clicked() {
                            do_apply = true;
                        }
                        if ui.button(RichText::new("Apply Threshold").strong()).clicked() {
                            if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                                layer.pixels = orig.clone();
                                hollow_core::filter::filter_threshold(&mut layer.pixels, doc_w, doc_h, state.filter_threshold_val, sel_mask.as_ref());
                            }
                            state.apply_filter_modal("Threshold");
                            return;
                        }
                        if ui.button("Cancel").clicked() {
                            do_cancel = true;
                        }
                    });
                });

            if changed || state.filter_preview_active {
                if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                    layer.pixels = orig.clone();
                    if state.filter_preview_active {
                        hollow_core::filter::filter_posterize(
                            &mut layer.pixels,
                            doc_w,
                            doc_h,
                            state.filter_posterize_levels,
                            sel_mask.as_ref(),
                        );
                    }
                }
            }

            if do_apply {
                state.apply_filter_modal("Posterize");
            } else if do_cancel {
                state.cancel_filter_modal();
            }
        }

        ActiveFilterModal::GaussianBlur => {
            let mut do_apply = false;
            let mut do_cancel = false;
            let mut changed = false;

            egui::Window::new("💧 Gaussian Blur")
                .fixed_size(Vec2::new(380.0, 180.0))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    changed |= ui.add(egui::Slider::new(&mut state.filter_blur_radius, 1.0..=32.0).text("Blur Radius (px)")).changed();

                    ui.add_space(8.0);
                    ui.checkbox(&mut state.filter_preview_active, "Live Preview");

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("✓ Apply").strong().color(accent_c32)).clicked() {
                            do_apply = true;
                        }
                        if ui.button("Cancel").clicked() {
                            do_cancel = true;
                        }
                    });
                });

            if changed || state.filter_preview_active {
                if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                    layer.pixels = orig.clone();
                    if state.filter_preview_active {
                        hollow_core::filter::filter_gaussian_blur(
                            &mut layer.pixels,
                            doc_w,
                            doc_h,
                            state.filter_blur_radius,
                            sel_mask.as_ref(),
                        );
                    }
                }
            }

            if do_apply {
                state.apply_filter_modal("Gaussian Blur");
            } else if do_cancel {
                state.cancel_filter_modal();
            }
        }

        ActiveFilterModal::SharpenUnsharp => {
            let mut do_apply = false;
            let mut do_cancel = false;
            let mut changed = false;

            egui::Window::new("🔪 Sharpen & Unsharp Mask")
                .fixed_size(Vec2::new(400.0, 240.0))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Sharpen Amount:").size(10.0));
                    changed |= ui.add(egui::Slider::new(&mut state.filter_sharpen_amount, 0.1..=2.5).text("Sharpen")).changed();

                    ui.separator();
                    ui.label(RichText::new("Unsharp Mask Parameters:").size(10.0));
                    changed |= ui.add(egui::Slider::new(&mut state.filter_unsharp_radius, 1.0..=8.0).text("Radius")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_unsharp_amount, 0.2..=2.5).text("Amount")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_unsharp_threshold, 0.0..=20.0).text("Threshold")).changed();

                    ui.add_space(8.0);
                    ui.checkbox(&mut state.filter_preview_active, "Live Preview");

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Apply Sharpen").strong().color(accent_c32)).clicked() {
                            do_apply = true;
                        }
                        if ui.button(RichText::new("Apply Unsharp Mask").strong()).clicked() {
                            if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                                layer.pixels = orig.clone();
                                hollow_core::filter::filter_unsharp_mask(
                                    &mut layer.pixels,
                                    doc_w,
                                    doc_h,
                                    state.filter_unsharp_radius,
                                    state.filter_unsharp_amount,
                                    state.filter_unsharp_threshold,
                                    sel_mask.as_ref(),
                                );
                            }
                            state.apply_filter_modal("Unsharp Mask");
                            return;
                        }
                        if ui.button("Cancel").clicked() {
                            do_cancel = true;
                        }
                    });
                });

            if changed || state.filter_preview_active {
                if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                    layer.pixels = orig.clone();
                    if state.filter_preview_active {
                        hollow_core::filter::filter_sharpen(
                            &mut layer.pixels,
                            doc_w,
                            doc_h,
                            state.filter_sharpen_amount,
                            sel_mask.as_ref(),
                        );
                    }
                }
            }

            if do_apply {
                state.apply_filter_modal("Sharpen");
            } else if do_cancel {
                state.cancel_filter_modal();
            }
        }

        ActiveFilterModal::FilmGrain => {
            let mut do_apply = false;
            let mut do_cancel = false;
            let mut changed = false;

            egui::Window::new("📺 Film Grain & Noise")
                .fixed_size(Vec2::new(380.0, 200.0))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    changed |= ui.add(egui::Slider::new(&mut state.filter_noise_intensity, 0.05..=0.8).text("Intensity")).changed();
                    changed |= ui.checkbox(&mut state.filter_noise_colored, "Chromatic (RGB Color Noise)").changed();

                    ui.add_space(8.0);
                    ui.checkbox(&mut state.filter_preview_active, "Live Preview");

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("✓ Apply").strong().color(accent_c32)).clicked() {
                            do_apply = true;
                        }
                        if ui.button("Cancel").clicked() {
                            do_cancel = true;
                        }
                    });
                });

            if changed || state.filter_preview_active {
                if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                    layer.pixels = orig.clone();
                    if state.filter_preview_active {
                        hollow_core::filter::filter_film_grain(
                            &mut layer.pixels,
                            doc_w,
                            doc_h,
                            state.filter_noise_intensity,
                            state.filter_noise_colored,
                            sel_mask.as_ref(),
                        );
                    }
                }
            }

            if do_apply {
                state.apply_filter_modal("Film Grain");
            } else if do_cancel {
                state.cancel_filter_modal();
            }
        }

        ActiveFilterModal::VignetteChromatic => {
            let mut do_apply = false;
            let mut do_cancel = false;
            let mut changed = false;

            egui::Window::new("🔍 Vignette & Lens FX")
                .fixed_size(Vec2::new(420.0, 280.0))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Vignette Settings:").size(10.0));
                    changed |= ui.add(egui::Slider::new(&mut state.filter_vignette_radius, 0.2..=1.4).text("Radius")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_vignette_softness, 0.1..=1.0).text("Softness")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_vignette_darkness, 0.1..=1.0).text("Darkness")).changed();

                    ui.separator();
                    ui.label(RichText::new("Chromatic Lens Aberration:").size(10.0));
                    changed |= ui.add(egui::Slider::new(&mut state.filter_chromatic_shift, 1.0..=18.0).text("Shift (px)")).changed();
                    changed |= ui.add(egui::Slider::new(&mut state.filter_chromatic_angle, 0.0..=360.0).text("Angle (°)")).changed();

                    ui.add_space(8.0);
                    ui.checkbox(&mut state.filter_preview_active, "Live Preview");

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Apply Vignette").strong().color(accent_c32)).clicked() {
                            do_apply = true;
                        }
                        if ui.button(RichText::new("Apply Chromatic Aberration").strong()).clicked() {
                            if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                                layer.pixels = orig.clone();
                                hollow_core::filter::filter_chromatic_aberration(
                                    &mut layer.pixels,
                                    doc_w,
                                    doc_h,
                                    state.filter_chromatic_shift,
                                    state.filter_chromatic_angle,
                                    sel_mask.as_ref(),
                                );
                            }
                            state.apply_filter_modal("Chromatic Aberration");
                            return;
                        }
                        if ui.button("Cancel").clicked() {
                            do_cancel = true;
                        }
                    });
                });

            if changed || state.filter_preview_active {
                if let (Some(orig), Some(layer)) = (&state.filter_original_pixels, state.document.active_layer_mut()) {
                    layer.pixels = orig.clone();
                    if state.filter_preview_active {
                        hollow_core::filter::filter_vignette(
                            &mut layer.pixels,
                            doc_w,
                            doc_h,
                            state.filter_vignette_radius,
                            state.filter_vignette_softness,
                            state.filter_vignette_darkness,
                            sel_mask.as_ref(),
                        );
                    }
                }
            }

            if do_apply {
                state.apply_filter_modal("Vignette");
            } else if do_cancel {
                state.cancel_filter_modal();
            }
        }

        ActiveFilterModal::None => {}
    }

    // ── 9. ABOUT HOLLOW CANVAS MODAL ──
    if state.show_about_dialog {
        egui::Window::new("About Hollow Canvas")
            .fixed_size(Vec2::new(420.0, 320.0))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("HOLLOW CANVAS").size(18.0).strong().color(Color32::from_rgb(235, 242, 255)));
                    ui.label(RichText::new("Digital Illustration & Graphics Studio").size(11.0).color(accent_c32));
                    ui.label(RichText::new("Version 0.6.0 · Pure Native Rust").size(10.0).color(Color32::from_rgb(130, 142, 172)));
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                ui.label(RichText::new("Features & Guarantees:").size(10.0).strong().color(Color32::from_rgb(205, 215, 240)));
                ui.label(RichText::new("• ⚡ Catmull-Rom Spline Drawing Engine with 0-allocation compositing\n• 🪄 Magic Wand, Gradients, Shapes, Multi-Axis Symmetry\n• 🎨 Full Adjustments & Artistic Filter FX Engine (HSL, Blur, Vignette, Grain)\n• 🔒 100% Local-First: Completely offline, zero telemetry, zero trackers\n• 📦 Universal VPack & Zip portable distribution").size(10.0).color(Color32::from_rgb(155, 165, 195)));

                ui.add_space(8.0);
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(RichText::new("License:").size(10.0).color(Color32::from_rgb(120, 130, 160)));
                    ui.label(RichText::new("GNU General Public License v3.0 (GPL-3.0)").size(10.0).color(Color32::from_rgb(185, 195, 225)));
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Author:").size(10.0).color(Color32::from_rgb(120, 130, 160)));
                    ui.label(RichText::new("LeTrollologist").size(10.0).strong().color(accent_c32));
                });

                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    if ui.add_sized([100.0, 24.0], egui::Button::new(RichText::new("Close").strong())).clicked() {
                        state.show_about_dialog = false;
                    }
                });
            });
    }

    // ── 10. HELP & SHORTCUTS MODAL ──
    if state.show_help {
        egui::Window::new("Hollow Canvas Studio · Shortcuts & Help")
            .fixed_size(Vec2::new(420.0, 380.0))
            .collapsible(false)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new("STUDIO SHORTCUTS").size(10.0).strong().color(accent_c32));
                    let shortcuts = [
                        ("Tab", "Toggle Full Canvas / Zen Mode"),
                        ("B", "Brush Tool"),
                        ("P", "Pencil Tool"),
                        ("W", "Magic Wand Selection"),
                        ("G", "Gradient / Fill Tool"),
                        ("E", "Eraser Tool"),
                        ("I / Alt+Click", "Eyedropper Color Picker"),
                        ("V", "Move Layer Tool"),
                        ("M", "Marquee Selection Tool"),
                        ("X", "Swap Primary & Secondary Colors"),
                        ("Space + Drag", "Pan Canvas Viewport"),
                        ("Mouse Wheel", "Zoom Canvas In / Out"),
                        ("Ctrl + N", "Create New Canvas"),
                        ("Ctrl + S", "Save Project (.hcv)"),
                        ("Ctrl + O", "Open Project (.hcv)"),
                        ("Ctrl + E", "Export PNG Image"),
                        ("Ctrl + I", "Invert Layer Colors"),
                        ("Ctrl + Z", "Undo Action"),
                        ("Ctrl + Y / Ctrl+Shift+Z", "Redo Action"),
                        ("Ctrl + D", "Deselect"),
                        ("T", "Toggle On-Canvas Tracing Paper"),
                        ("Ctrl + '", "Toggle Viewport Grid"),
                        ("Ctrl + R", "Toggle Viewport Rulers"),
                    ];

                    egui::Grid::new("help_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                        for (k, desc) in shortcuts {
                            ui.label(RichText::new(k).strong().color(Color32::from_rgb(225, 235, 255)));
                            ui.label(RichText::new(desc).color(Color32::from_rgb(165, 175, 205)));
                            ui.end_row();
                        }
                    });
                });
                if ui.button("Close").clicked() {
                    state.show_help = false;
                }
            });
    }
}
