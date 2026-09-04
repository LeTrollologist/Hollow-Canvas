use crate::icons::draw_tool_icon;
use crate::state::{ActiveFilterModal, AppState, CanvasPreset, PendingFileAction};
use egui::{Align, Color32, Layout, Rect, RichText, ScrollArea, Stroke, Vec2};
use hollow_core::brush::{EraserMode, GradientType, ShapeFillMode, ToolType};
use hollow_core::color::{Color, DEFAULT_PALETTE};
use hollow_core::selection::SelectionMask;
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
            .frame(egui::Frame::none().fill(Color32::from_rgb(8, 12, 24)).stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(25, 34, 58))).inner_margin(egui::Margin::symmetric(8.0, 3.5)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Left Brand & Version
                    ui.label(RichText::new("✦ HOLLOW CANVAS").size(12.5).strong().color(Color32::from_rgb(235, 242, 255)));
                    ui.label(RichText::new("v0.17.0").size(9.0).color(Color32::from_rgb(115, 130, 165)));

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
                            if ui.button("🎬 Export Animation...").clicked() {
                                state.show_export_animation_dialog = true;
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
                            if ui.button("⤢ Free Transform (Ctrl+T)").clicked() {
                                state.brush.tool = ToolType::Transform;
                                state.begin_transform_session();
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("⇄ Swap Colors (X)").clicked() {
                                state.swap_colors();
                                ui.close_menu();
                            }
                        });

                        ui.menu_button("Select", |ui| {
                            if ui.button("Select All (Ctrl+A)").clicked() {
                                state.selection = Some(SelectionMask::select_all(state.document.width, state.document.height));
                                state.set_status("Selected All");
                                ui.close_menu();
                            }
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
                            ui.separator();
                            if ui.button("Feather Selection...").clicked() {
                                state.show_feather_dialog = true;
                                ui.close_menu();
                            }
                            if ui.button("Expand Selection...").clicked() {
                                state.show_expand_dialog = true;
                                ui.close_menu();
                            }
                            if ui.button("Contract Selection...").clicked() {
                                state.show_contract_dialog = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Fill Selection (Shift+F5 / Alt+Bksp)").clicked() {
                                state.fill_selection_active_layer();
                                ui.close_menu();
                            }
                            if ui.button("Stroke Selection...").clicked() {
                                state.show_stroke_dialog = true;
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
                            if ui.checkbox(&mut state.show_navigator, "🧭 Viewport / Navigator Dock").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.show_perspective_dock, "📐 Perspective Studio (F5)").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.perspective.snap_enabled, "⇲ Perspective Snap (Ctrl+Shift+P)").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.show_scratchpad, "🎨 Mixing Scratchpad (F4)").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.show_grid, "⊞ Toggle Grid (Ctrl+')").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.show_rulers, "📏 Toggle Rulers (Ctrl+R)").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.timeline.is_enabled, "🎞 Animation Timeline").clicked() {
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.checkbox(&mut state.show_ui_panels, "👁 Studio Panels (Tab)").clicked() {
                                ui.close_menu();
                            }
                            if ui.button("⛶ Fit to Screen (Ctrl+0)").clicked() {
                                let sr = ctx.screen_rect();
                                state.reset_view_centered(sr.width(), sr.height());
                                ui.close_menu();
                            }
                            if ui.button("1:1 Actual Size (100% / Ctrl+1)").clicked() {
                                state.zoom = 1.0;
                                state.pan = glam::Vec2::ZERO;
                                state.set_status("View reset to 100%");
                                ui.close_menu();
                            }
                        });

                        ui.menu_button("Window", |ui| {
                            if ui.checkbox(&mut state.show_navigator, "🧭 Viewport / Navigator Panel").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.show_perspective_dock, "📐 Perspective Studio (F5)").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.show_scratchpad, "🎨 Mixing Scratchpad (F4)").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.show_ref_window, "🖼 Reference Viewer").clicked() {
                                ui.close_menu();
                            }
                            if ui.checkbox(&mut state.timeline.is_enabled, "🎞 Animation Timeline Strip").clicked() {
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

                        let persp_label = if state.show_perspective_dock { "📐 [ON]" } else { "📐 Persp" };
                        if ui.selectable_label(state.show_perspective_dock, persp_label).on_hover_text("Visual Perspective Rulers & Vanishing Point Studio (F5)").clicked() {
                            state.show_perspective_dock = !state.show_perspective_dock;
                        }

                        let snap_label = if state.perspective.snap_enabled { "⇲ [SNAP]" } else { "⇲ Snap" };
                        if ui.selectable_label(state.perspective.snap_enabled, snap_label).on_hover_text("Constraint Snapping to Perspective Vectors (Ctrl+Shift+P)").clicked() {
                            state.perspective.snap_enabled = !state.perspective.snap_enabled;
                            state.set_status(if state.perspective.snap_enabled { "Perspective Snapping: ON" } else { "Perspective Snapping: OFF" });
                        }

                        let scratch_label = if state.show_scratchpad { "🎨 [ON]" } else { "🎨 Palette" };
                        if ui.selectable_label(state.show_scratchpad, scratch_label).on_hover_text("Floating Color Mixing Scratchpad (F4)").clicked() {
                            state.show_scratchpad = !state.show_scratchpad;
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
                        ui.add_space(3.0);

                        // S-Level Stabilizer quick chip
                        let s_lvl_txt = format!("🎯 S-{}", state.brush.stabilization_level);
                        let s_btn = egui::Button::new(RichText::new(s_lvl_txt).size(10.0).strong().color(accent_c32))
                            .fill(if state.brush.stabilization_level > 0 { accent_dim_c32 } else { Color32::from_rgb(18, 24, 40) })
                            .stroke(egui::Stroke::new(1.0_f32, if state.brush.stabilization_level > 0 { accent_c32 } else { Color32::from_rgb(45, 55, 80) }));
                        if ui.add(s_btn).on_hover_text(format!("{}: Click to cycle (S-0..S-7)", state.brush.stabilization_label())).clicked() {
                            state.brush.stabilization_level = (state.brush.stabilization_level + 1) % 8;
                            state.set_status(format!("Stabilizer: {}", state.brush.stabilization_label()));
                        }

                        ui.add_space(4.0);

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

    // ── 2. BOTTOM TIMELINE STRIP PANEL ──
    if state.show_ui_panels && state.timeline.is_enabled {
        egui::TopBottomPanel::bottom("animation_timeline_panel")
            .frame(egui::Frame::none().fill(Color32::from_rgb(8, 11, 22)).stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(25, 34, 58))).inner_margin(egui::Margin::symmetric(8.0, 6.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Playback Controls
                    let play_icon = if state.timeline.is_playing { "⏸ Pause" } else { "▶ Play" };
                    if ui.button(RichText::new(play_icon).size(11.0).strong().color(if state.timeline.is_playing { Color32::from_rgb(255, 200, 80) } else { Color32::from_rgb(100, 240, 150) })).clicked() {
                        state.toggle_animation_playback();
                    }
                    if ui.button("⏮").on_hover_text("Previous Frame ([)").clicked() {
                        state.step_prev_frame();
                    }
                    if ui.button("⏭").on_hover_text("Next Frame (])").clicked() {
                        state.step_next_frame();
                    }
                    if ui.selectable_label(state.timeline.loop_playback, "🔁 Loop").clicked() {
                        state.timeline.loop_playback = !state.timeline.loop_playback;
                    }

                    ui.separator();

                    // FPS slider
                    ui.label(RichText::new("FPS:").size(10.5).color(Color32::from_rgb(150, 165, 195)));
                    ui.add(egui::DragValue::new(&mut state.timeline.fps).range(1..=60).speed(0.5));

                    ui.separator();

                    // Onion Skin toggle & settings
                    let onion_active = state.timeline.onion_skin_enabled;
                    let onion_btn = egui::Button::new(RichText::new("🧅 Onion Skin (O)").size(10.5).color(if onion_active { accent_c32 } else { Color32::from_rgb(140, 150, 175) }))
                        .fill(if onion_active { accent_c32.linear_multiply(0.2) } else { Color32::from_rgb(20, 26, 44) });
                    if ui.add(onion_btn).on_hover_text("Toggle Ghost Onion Skinning (O)").clicked() {
                        state.toggle_onion_skin();
                    }

                    if onion_active {
                        ui.label(RichText::new("Prev:").size(9.5).color(Color32::from_rgb(255, 120, 120)));
                        ui.add(egui::DragValue::new(&mut state.timeline.onion_skin_prev_count).range(1..=5).speed(0.1));
                        ui.label(RichText::new("Next:").size(9.5).color(Color32::from_rgb(120, 255, 150)));
                        ui.add(egui::DragValue::new(&mut state.timeline.onion_skin_next_count).range(1..=5).speed(0.1));
                        ui.label(RichText::new("Opacity:").size(9.5).color(Color32::from_rgb(160, 170, 195)));
                        ui.add(egui::Slider::new(&mut state.timeline.onion_skin_opacity, 0.1..=1.0).show_value(false));
                    }

                    ui.separator();

                    // Actions
                    if ui.button(RichText::new("＋ Add Frame").strong().color(accent_c32)).clicked() {
                        state.add_animation_frame();
                    }
                    if ui.button("Duplicate").on_hover_text("Duplicate Active Frame").clicked() {
                        state.duplicate_animation_frame();
                    }
                    if ui.button("🗑 Delete").on_hover_text("Delete Active Frame").clicked() {
                        state.delete_animation_frame();
                    }
                    if ui.button("🎬 Export...").on_hover_text("Export Animated GIF / WebP").clicked() {
                        state.show_export_animation_dialog = true;
                    }
                });

                ui.add_space(4.0);

                // Frame Reel Strip
                ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let total_frames = state.timeline.frames.len();
                        for i in 0..total_frames {
                            let is_current = i == state.timeline.current_frame_idx;
                            let frame_name = format!("{}: {}", i + 1, state.timeline.frames[i].name);
                            let chip = egui::Button::new(RichText::new(frame_name).size(10.5).strong())
                                .fill(if is_current { accent_c32.linear_multiply(0.4) } else { Color32::from_rgb(16, 22, 36) })
                                .stroke(egui::Stroke::new(1.0_f32, if is_current { accent_c32 } else { Color32::from_rgb(40, 50, 75) }))
                                .min_size(Vec2::new(76.0, 24.0));
                            if ui.add(chip).clicked() {
                                state.select_animation_frame(i);
                            }
                            if i > 0 {
                                if ui.small_button("◀").on_hover_text("Move frame left").clicked() {
                                    state.reorder_animation_frame(i, i - 1);
                                }
                            }
                            if i + 1 < total_frames {
                                if ui.small_button("▶").on_hover_text("Move frame right").clicked() {
                                    state.reorder_animation_frame(i, i + 1);
                                }
                            }
                            ui.add_space(4.0);
                        }
                    });
                });
            });
    }

    // ── 2. BOTTOM STATUS BAR ──
    if state.show_ui_panels {
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none().fill(Color32::from_rgb(5, 7, 15)).stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(25, 34, 58))).inner_margin(4.0))
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
            .exact_width(220.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(Color32::from_rgb(10, 14, 26)).stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(25, 34, 58))).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.set_max_width(220.0);
                ui.set_width(220.0);
                ScrollArea::vertical().max_width(220.0).show(ui, |ui| {
                    ui.set_max_width(204.0);
                    ui.set_width(204.0);
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
                        (ToolType::SelectionBrush, "Sel Brush"),
                        (ToolType::SelectionEraser, "Sel Erase"),
                        (ToolType::Eraser, "Eraser"),
                        (ToolType::Fill, "Fill"),
                        (ToolType::Line, "Line"),
                        (ToolType::Rect, "Rect"),
                        (ToolType::Ellipse, "Oval"),
                        (ToolType::Polygon, "Poly"),
                        (ToolType::Marquee, "Select"),
                        (ToolType::Lasso, "Lasso"),
                        (ToolType::Move, "Move"),
                        (ToolType::Transform, "Trans"),
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

                    ui.add_space(6.0);
                    ui.separator();

                    // ── PRESET SHELF ──
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("PRESET SHELF").size(9.5).strong().color(Color32::from_rgb(110, 125, 158)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("＋ Save").on_hover_text("Save current brush settings as a new custom preset").clicked() {
                                state.show_save_preset_dialog = true;
                                state.new_preset_name = format!("Preset {}", state.presets.len() + 1);
                            }
                        });
                    });

                    egui::Grid::new("preset_shelf_grid").num_columns(2).spacing([4.0, 4.0]).show(ui, |ui| {
                        let num_presets = state.presets.len();
                        for i in 0..num_presets {
                            let is_active = state.active_preset_idx == Some(i);
                            let p = &state.presets[i];
                            let label = format!("{} {}", p.icon, p.name);
                            let btn = egui::Button::new(RichText::new(label).size(10.0))
                                .fill(if is_active { accent_c32.linear_multiply(0.35) } else { Color32::from_rgb(18, 24, 40) })
                                .stroke(egui::Stroke::new(1.0_f32, if is_active { accent_c32 } else { Color32::from_rgb(45, 55, 80) }))
                                .min_size(Vec2::new(98.0, 22.0));
                            if ui.add(btn).on_hover_text(&p.description).clicked() {
                                state.select_preset(i);
                            }
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });

                    ui.add_space(6.0);
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
                        ToolType::Lasso => {
                            ui.label(RichText::new("Draw freehand loop to select").size(9.0).color(Color32::from_rgb(130, 142, 172)));
                            ui.label(RichText::new("Shift: Add · Alt: Subtract").size(8.5).color(accent_c32));
                            if let Some(mask) = &state.selection {
                                if mask.has_selection() {
                                    ui.horizontal(|ui| {
                                        if ui.button("Feather...").clicked() {
                                            state.show_feather_dialog = true;
                                        }
                                        if ui.button("Fill").clicked() {
                                            state.fill_selection_active_layer();
                                        }
                                        if ui.button("✕ Deselect").clicked() {
                                            state.selection = None;
                                        }
                                    });
                                }
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
                        ToolType::Transform => {
                            if !state.transform_session.is_active {
                                if ui.button(RichText::new("✦ Start Free Transform (Ctrl+T)").strong().color(accent_c32)).clicked() {
                                    state.begin_transform_session();
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    if ui.button(RichText::new("✓ Apply (Enter)").strong().color(Color32::from_rgb(100, 240, 150))).clicked() {
                                        state.commit_transform_session();
                                    }
                                    if ui.button(RichText::new("✕ Cancel (Esc)").strong().color(Color32::from_rgb(250, 120, 120))).clicked() {
                                        state.cancel_transform_session();
                                    }
                                });
                                ui.add_space(4.0);
                                let mut rot_deg = state.transform_session.transform.rotation_rad.to_degrees();
                                if ui.add(egui::Slider::new(&mut rot_deg, -180.0..=180.0).text("Angle (°)")).changed() {
                                    state.transform_session.transform.rotation_rad = rot_deg.to_radians();
                                    state.update_transform_preview();
                                }
                                let mut scale_pct = state.transform_session.transform.scale.x * 100.0;
                                if ui.add(egui::Slider::new(&mut scale_pct, 5.0..=400.0).text("Scale (%)")).changed() {
                                    let s = scale_pct / 100.0;
                                    state.transform_session.transform.scale = glam::Vec2::new(s, s);
                                    state.update_transform_preview();
                                }
                                ui.horizontal(|ui| {
                                    if ui.button("⇄ Flip H").clicked() {
                                        state.transform_session.transform.flip_h = !state.transform_session.transform.flip_h;
                                        state.update_transform_preview();
                                    }
                                    if ui.button("⇅ Flip V").clicked() {
                                        state.transform_session.transform.flip_v = !state.transform_session.transform.flip_v;
                                        state.update_transform_preview();
                                    }
                                });
                                egui::ComboBox::from_label("Filter")
                                    .selected_text(match state.transform_session.interpolation {
                                        hollow_core::transform::InterpolationMode::Nearest => "Nearest",
                                        hollow_core::transform::InterpolationMode::Bilinear => "Bilinear",
                                        hollow_core::transform::InterpolationMode::Bicubic => "Bicubic",
                                    })
                                    .show_ui(ui, |ui| {
                                        if ui.selectable_label(state.transform_session.interpolation == hollow_core::transform::InterpolationMode::Bicubic, "Bicubic").clicked() {
                                            state.transform_session.interpolation = hollow_core::transform::InterpolationMode::Bicubic;
                                            state.update_transform_preview();
                                        }
                                        if ui.selectable_label(state.transform_session.interpolation == hollow_core::transform::InterpolationMode::Bilinear, "Bilinear").clicked() {
                                            state.transform_session.interpolation = hollow_core::transform::InterpolationMode::Bilinear;
                                            state.update_transform_preview();
                                        }
                                        if ui.selectable_label(state.transform_session.interpolation == hollow_core::transform::InterpolationMode::Nearest, "Nearest").clicked() {
                                            state.transform_session.interpolation = hollow_core::transform::InterpolationMode::Nearest;
                                            state.update_transform_preview();
                                        }
                                    });
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

                    // ── GLOBAL STROKE STABILIZATION (S-LEVELS) ──
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🎯 STABILIZATION").size(9.5).strong().color(accent_c32));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(state.brush.stabilization_label()).size(9.0).color(Color32::from_rgb(170, 190, 230)));
                        });
                    });
                    ui.add(egui::Slider::new(&mut state.brush.stabilization_level, 0..=7).text("S-Level"));

                    ui.horizontal_wrapped(|ui| {
                        let s_presets = [(0, "S-0"), (1, "S-1"), (2, "S-2"), (3, "S-3"), (4, "S-4"), (5, "S-5"), (6, "S-6"), (7, "S-7")];
                        for (lvl, lbl) in s_presets {
                            let is_sel = state.brush.stabilization_level == lvl;
                            let btn = egui::Button::new(RichText::new(lbl).size(9.0))
                                .fill(if is_sel { accent_c32.linear_multiply(0.35) } else { Color32::from_rgb(18, 24, 40) })
                                .stroke(egui::Stroke::new(1.0_f32, if is_sel { accent_c32 } else { Color32::from_rgb(45, 55, 80) }))
                                .min_size(Vec2::new(21.0, 18.0));
                            if ui.add(btn).on_hover_text(state.brush.stabilization_description(lvl)).clicked() {
                                state.brush.stabilization_level = lvl;
                                state.set_status(format!("Stabilizer: {}", state.brush.stabilization_label()));
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.checkbox(&mut state.brush.velocity_dynamics, "⚡ Velocity Taper");
                    if state.brush.velocity_dynamics {
                        ui.add(egui::Slider::new(&mut state.brush.velocity_taper_strength, 0.0..=1.0).text("Taper Sharpness"));
                        ui.add(egui::Slider::new(&mut state.brush.velocity_min_size, 0.05..=0.8).text("Min Tip Ratio"));
                    }

                    ui.add_space(4.0);
                    ui.add(egui::Slider::new(&mut state.brush.calligraphy_weight, 0.0..=1.0).text("Calligraphy Chisel"));
                    if state.brush.calligraphy_weight > 0.001 {
                        ui.add(egui::Slider::new(&mut state.brush.calligraphy_angle, 0.0..=180.0).text("Chisel Angle (°)"));
                    }

                    ui.add_space(4.0);
                    ui.add(egui::Slider::new(&mut state.brush.wet_edge_strength, 0.0..=1.0).text("Wet Edge Pooling"));
                    if state.brush.wet_edge_strength > 0.001 {
                        ui.add(egui::Slider::new(&mut state.brush.wet_edge_fringe_width, 0.05..=0.5).text("Fringe Width"));
                    }

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
            .exact_width(240.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(Color32::from_rgb(10, 14, 26)).stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(25, 34, 58))).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.set_max_width(240.0);
                ui.set_width(240.0);
                ScrollArea::vertical().max_width(240.0).show(ui, |ui| {
                    ui.set_max_width(224.0);
                    ui.set_width(224.0);
                    // ── 🧭 VIEWPORT / NAVIGATOR ──
                    if state.show_navigator {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("🧭 VIEWPORT / NAVIGATOR").size(9.5).strong().color(Color32::from_rgb(110, 125, 158)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let pct = (state.zoom * 100.0).round() as i32;
                                ui.label(RichText::new(format!("{}%", pct)).size(9.5).strong().color(accent_c32));
                            });
                        });
                        ui.add_space(2.0);

                        // Mini-map Thumbnail with Viewport Rect
                        let doc_w = state.document.width as f32;
                        let doc_h = state.document.height as f32;
                        let aspect = if doc_h > 0.0 { doc_w / doc_h } else { 1.0 };
                        let thumb_w = 224.0_f32;
                        let thumb_h = (thumb_w / aspect).clamp(60.0, 140.0);
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(thumb_w, thumb_h), egui::Sense::click_and_drag());

                        let painter = ui.painter_at(rect);
                        // Background paper frame
                        let bg_c = if state.document.is_transparent {
                            Color32::from_rgb(18, 24, 38)
                        } else {
                            let v = state.document.background_value;
                            Color32::from_rgb(v, v, v)
                        };
                        painter.rect_filled(rect, 4.0, bg_c);
                        painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0_f32, Color32::from_rgb(45, 60, 95)));

                        // Draw live active viewport framing box inside the thumbnail
                        let canvas_center_x = doc_w * 0.5 - state.pan.x / state.zoom;
                        let canvas_center_y = doc_h * 0.5 - state.pan.y / state.zoom;

                        let win_w_f = ctx.screen_rect().width();
                        let win_h_f = ctx.screen_rect().height();
                        let open_vp_w = (win_w_f - 490.0).max(100.0);
                        let open_vp_h = (win_h_f - 70.0).max(100.0);

                        let vp_canvas_w = open_vp_w / state.zoom;
                        let vp_canvas_h = open_vp_h / state.zoom;

                        let vp_min_cx = canvas_center_x - vp_canvas_w * 0.5;
                        let vp_min_cy = canvas_center_y - vp_canvas_h * 0.5;
                        let vp_max_cx = canvas_center_x + vp_canvas_w * 0.5;
                        let vp_max_cy = canvas_center_y + vp_canvas_h * 0.5;

                        let scale_x = thumb_w / doc_w;
                        let scale_y = thumb_h / doc_h;

                        let box_min_x = (rect.min.x + vp_min_cx * scale_x).clamp(rect.min.x, rect.max.x);
                        let box_min_y = (rect.min.y + vp_min_cy * scale_y).clamp(rect.min.y, rect.max.y);
                        let box_max_x = (rect.min.x + vp_max_cx * scale_x).clamp(rect.min.x, rect.max.x);
                        let box_max_y = (rect.min.y + vp_max_cy * scale_y).clamp(rect.min.y, rect.max.y);

                        let vp_rect = egui::Rect::from_min_max(
                            egui::pos2(box_min_x, box_min_y),
                            egui::pos2(box_max_x.max(box_min_x + 4.0), box_max_y.max(box_min_y + 4.0)),
                        );

                        painter.rect_filled(vp_rect, 2.0, Color32::from_rgba_unmultiplied(92, 224, 216, 35));
                        painter.rect_stroke(vp_rect, 2.0, egui::Stroke::new(1.5_f32, Color32::from_rgb(92, 224, 216)));

                        // Interactive: Click / Drag in Navigator to pan canvas immediately
                        if response.clicked() || response.dragged() {
                            if let Some(mouse_pos) = response.interact_pointer_pos() {
                                let rel_x = (mouse_pos.x - rect.min.x) / thumb_w;
                                let rel_y = (mouse_pos.y - rect.min.y) / thumb_h;
                                let target_canvas_pos = glam::Vec2::new(
                                    (rel_x * doc_w).clamp(0.0, doc_w),
                                    (rel_y * doc_h).clamp(0.0, doc_h),
                                );
                                state.pan_to_canvas_center(target_canvas_pos);
                            }
                        }

                        // Quick Viewport Action Buttons
                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            if ui.button("⛶ Fit").on_hover_text("Fit Canvas to Screen (Ctrl+0)").clicked() {
                                state.reset_view_centered(win_w_f, win_h_f);
                            }
                            if ui.button("100%").on_hover_text("Actual 1:1 Pixel Size (Ctrl+1)").clicked() {
                                state.zoom = 1.0;
                                state.pan = glam::Vec2::ZERO;
                            }
                            if ui.button("⟲ Center").on_hover_text("Reset Pan to Center").clicked() {
                                state.pan = glam::Vec2::ZERO;
                            }
                            let flip_label = if state.flip_view_horizontal { "⇄ Unflip" } else { "⇄ Flip (H)" };
                            if ui.button(flip_label).on_hover_text("Flip Canvas View Horizontally").clicked() {
                                state.flip_view_horizontal = !state.flip_view_horizontal;
                            }
                        });

                        // Zoom Slider
                        let mut zoom_pct = (state.zoom * 100.0).round() as i32;
                        if ui.add(egui::Slider::new(&mut zoom_pct, 5..=800).text("Zoom %").suffix("%")).changed() {
                            state.zoom = (zoom_pct as f32 / 100.0).clamp(0.05, 8.0);
                        }

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);
                    }

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
                            egui::ComboBox::from_id_source("add_adj_layer_combo")
                                .selected_text("✨+ Adj")
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(false, "☀️ Brightness / Contrast").clicked() {
                                        let id = state.document.add_adjustment_layer(hollow_core::layer::AdjustmentType::BrightnessContrast { brightness: 0.0, contrast: 0.0 }, None);
                                        state.active_adjustment_modal = Some(id);
                                        state.set_status(format!("Added Brightness/Contrast Adjustment Layer {}", id));
                                    }
                                    if ui.selectable_label(false, "🎨 HSL").clicked() {
                                        let id = state.document.add_adjustment_layer(hollow_core::layer::AdjustmentType::Hsl { hue_shift: 0.0, saturation: 1.0, lightness: 0.0 }, None);
                                        state.active_adjustment_modal = Some(id);
                                        state.set_status(format!("Added HSL Adjustment Layer {}", id));
                                    }
                                    if ui.selectable_label(false, "⚖️ Color Balance").clicked() {
                                        let id = state.document.add_adjustment_layer(hollow_core::layer::AdjustmentType::ColorBalance { cyan_red: 0.0, magenta_green: 0.0, yellow_blue: 0.0 }, None);
                                        state.active_adjustment_modal = Some(id);
                                        state.set_status(format!("Added Color Balance Adjustment Layer {}", id));
                                    }
                                    if ui.selectable_label(false, "🔄 Invert").clicked() {
                                        let id = state.document.add_adjustment_layer(hollow_core::layer::AdjustmentType::Invert, None);
                                        state.set_status(format!("Added Invert Adjustment Layer {}", id));
                                    }
                                    if ui.selectable_label(false, "🎚️ Posterize").clicked() {
                                        let id = state.document.add_adjustment_layer(hollow_core::layer::AdjustmentType::Posterize { levels: 6 }, None);
                                        state.active_adjustment_modal = Some(id);
                                        state.set_status(format!("Added Posterize Adjustment Layer {}", id));
                                    }
                                    if ui.selectable_label(false, "🌓 Threshold").clicked() {
                                        let id = state.document.add_adjustment_layer(hollow_core::layer::AdjustmentType::Threshold { cutoff: 128 }, None);
                                        state.active_adjustment_modal = Some(id);
                                        state.set_status(format!("Added Threshold Adjustment Layer {}", id));
                                    }
                                    if ui.selectable_label(false, "🎞️ Sepia").clicked() {
                                        let id = state.document.add_adjustment_layer(hollow_core::layer::AdjustmentType::Sepia { strength: 1.0 }, None);
                                        state.active_adjustment_modal = Some(id);
                                        state.set_status(format!("Added Sepia Adjustment Layer {}", id));
                                    }
                                });
                            if ui.button("📁+ Folder").on_hover_text("Create a new Layer Group Folder").clicked() {
                                let gid = state.document.add_group(None);
                                state.set_status(format!("Created Folder Group {}", gid));
                            }
                            if ui.button("➕ Layer").on_hover_text("Add a new drawing layer").clicked() {
                                let id = state.document.add_layer(None);
                                state.set_status(format!("Added Layer {}", id));
                            }
                        });
                    });

                    let active_id = state.document.active_layer_id;
                    let layer_count = state.document.layers.len();

                    // Collect all group IDs for the parent assignment selector
                    let group_options: Vec<(u64, String)> = state
                        .document
                        .layers
                        .iter()
                        .filter(|l| l.is_group())
                        .map(|l| (l.id, l.name.clone()))
                        .collect();

                    // Render layers in reverse order (top to bottom)
                    for i in (0..layer_count).rev() {
                        let layer_id = state.document.layers[i].id;
                        let is_active = layer_id == active_id;
                        let is_group = state.document.layers[i].is_group();
                        let is_adj = state.document.layers[i].is_adjustment();
                        let is_ref = state.document.layers[i].is_reference;
                        let is_clip = state.document.layers[i].clipping_mask;

                        // Calculate depth and check if any ancestor is collapsed
                        let mut depth = 0usize;
                        let mut is_hidden_by_collapse = false;
                        let mut curr_parent = state.document.layers[i].parent_id;
                        while let Some(pid) = curr_parent {
                            depth += 1;
                            if let Some(parent_layer) = state.document.get_layer(pid) {
                                if !parent_layer.is_expanded {
                                    is_hidden_by_collapse = true;
                                    break;
                                }
                                curr_parent = parent_layer.parent_id;
                            } else {
                                break;
                            }
                        }

                        if is_hidden_by_collapse {
                            continue;
                        }

                        let indent_px = (depth as f32 * 12.0).min(48.0);

                        ui.horizontal(|ui| {
                            if indent_px > 0.0 {
                                ui.add_space(indent_px);
                            }

                            egui::Frame::none()
                                .fill(if is_active {
                                    if is_group { Color32::from_rgba_unmultiplied(ar, ag, ab, 55) } else if is_adj { Color32::from_rgba_unmultiplied(ar, ag, ab, 48) } else { Color32::from_rgba_unmultiplied(ar, ag, ab, 42) }
                                } else if is_group {
                                    Color32::from_rgba_unmultiplied(24, 32, 58, 210)
                                } else if is_adj {
                                    Color32::from_rgba_unmultiplied(28, 22, 52, 190)
                                } else {
                                    Color32::from_rgba_unmultiplied(18, 25, 46, 180)
                                })
                                .stroke(egui::Stroke::new(1.0_f32, if is_active { accent_c32 } else if is_group { Color32::from_rgb(48, 64, 100) } else if is_adj { Color32::from_rgb(70, 52, 110) } else { Color32::from_rgb(36, 48, 78) }))
                                .rounding(4.0)
                                .inner_margin(egui::vec2(5.0, 4.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        // Visibility toggle
                                        let vis_icon = if state.document.layers[i].visible { "👁" } else { "⊘" };
                                        if ui.button(vis_icon).clicked() {
                                            state.document.layers[i].visible = !state.document.layers[i].visible;
                                        }

                                        if is_group {
                                            // Folder collapse/expand toggle
                                            let fold_icon = if state.document.layers[i].is_expanded { "▼" } else { "▶" };
                                            if ui.small_button(fold_icon).clicked() {
                                                state.document.layers[i].is_expanded = !state.document.layers[i].is_expanded;
                                            }
                                            let folder_icon = if state.document.layers[i].is_expanded { "📂" } else { "📁" };
                                            ui.label(RichText::new(folder_icon).size(11.0));
                                        } else if depth > 0 {
                                            ui.label(RichText::new("└").size(10.0).color(Color32::from_rgb(100, 115, 145)));
                                        }

                                        if is_clip {
                                            ui.label(RichText::new("⮑").size(11.0).color(accent_c32));
                                        }
                                        if is_ref {
                                            ui.label(RichText::new("⭐").size(10.0));
                                        }
                                        if is_adj {
                                            if let Some(adj) = &state.document.layers[i].adjustment {
                                                ui.label(RichText::new(adj.adjustment_type.badge()).size(9.0).strong().color(accent_c32));
                                            }
                                        }

                                        // Editable Layer/Group Name
                                        let name_edit = egui::TextEdit::singleline(&mut state.document.layers[i].name)
                                            .desired_width(75.0);
                                        let response = ui.add(name_edit);
                                        if response.clicked() || response.gained_focus() {
                                            state.document.active_layer_id = layer_id;
                                        }

                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if is_group {
                                                if ui.button("🗑").on_hover_text("Delete Folder and all contained layers").clicked() {
                                                    state.document.delete_layer(layer_id);
                                                }
                                                if ui.button("🔓").on_hover_text("Ungroup (release child layers)").clicked() {
                                                    state.document.ungroup(layer_id);
                                                }
                                                if ui.button("➕").on_hover_text("Add new layer inside this folder").clicked() {
                                                    let new_id = state.document.add_layer(None);
                                                    state.document.set_layer_parent(new_id, Some(layer_id));
                                                }
                                            } else {
                                                if ui.button("🗑").on_hover_text("Delete Layer").clicked() {
                                                    state.document.delete_layer(layer_id);
                                                }
                                                if ui.button("⎘").on_hover_text("Duplicate Layer").clicked() {
                                                    state.document.duplicate_active_layer();
                                                }
                                                if is_adj {
                                                    if ui.button("⚙").on_hover_text("Configure Adjustment Filter Parameters").clicked() {
                                                        state.active_adjustment_modal = Some(layer_id);
                                                    }
                                                }
                                                if state.document.layers[i].parent_id.is_some() {
                                                    if ui.button("⮯").on_hover_text("Remove from folder (move to top level)").clicked() {
                                                        state.document.layers[i].parent_id = None;
                                                    }
                                                }
                                            }
                                        });
                                    });

                                    if is_active {
                                        ui.horizontal(|ui| {
                                            ui.add(egui::Slider::new(&mut state.document.layers[i].opacity, 0.0..=1.0).text("Opacity"));
                                        });

                                        if is_adj {
                                            ui.horizontal(|ui| {
                                                ui.checkbox(&mut state.document.layers[i].clipping_mask, "⮑ Clip");
                                                if ui.small_button("⚙ Edit Filter Parameters...").clicked() {
                                                    state.active_adjustment_modal = Some(layer_id);
                                                }
                                            });

                                            // Folder parent assignment selector
                                            if !group_options.is_empty() {
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new("📁 In:").size(10.0).color(Color32::from_rgb(130, 142, 172)));
                                                    let current_parent_id = state.document.layers[i].parent_id;
                                                    let current_name = current_parent_id
                                                        .and_then(|pid| group_options.iter().find(|(gid, _)| *gid == pid).map(|(_, n)| n.as_str()))
                                                        .unwrap_or("Root");

                                                    egui::ComboBox::from_id_source(format!("group_sel_{}", layer_id))
                                                        .selected_text(current_name)
                                                        .show_ui(ui, |ui| {
                                                            if ui.selectable_label(current_parent_id.is_none(), "Root (No Folder)").clicked() {
                                                                state.document.set_layer_parent(layer_id, None);
                                                            }
                                                            for (gid, gname) in &group_options {
                                                                if *gid != layer_id {
                                                                    if ui.selectable_label(current_parent_id == Some(*gid), format!("📁 {}", gname)).clicked() {
                                                                        state.document.set_layer_parent(layer_id, Some(*gid));
                                                                    }
                                                                }
                                                            }
                                                        });
                                                });
                                            }
                                        } else if !is_group {
                                            ui.horizontal(|ui| {
                                                ui.checkbox(&mut state.document.layers[i].alpha_locked, "🔒α Lock");
                                                ui.checkbox(&mut state.document.layers[i].clipping_mask, "⮑ Clip");
                                                ui.checkbox(&mut state.document.layers[i].locked, "🔒");
                                            });
                                            ui.horizontal(|ui| {
                                                ui.checkbox(&mut state.document.layers[i].is_reference, "Ref Boundary");
                                            });

                                            // Folder parent assignment selector
                                            if !group_options.is_empty() {
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new("📁 In:").size(10.0).color(Color32::from_rgb(130, 142, 172)));
                                                    let current_parent_id = state.document.layers[i].parent_id;
                                                    let current_name = current_parent_id
                                                        .and_then(|pid| group_options.iter().find(|(gid, _)| *gid == pid).map(|(_, n)| n.as_str()))
                                                        .unwrap_or("Root");

                                                    egui::ComboBox::from_id_source(format!("group_sel_{}", layer_id))
                                                        .selected_text(current_name)
                                                        .show_ui(ui, |ui| {
                                                            if ui.selectable_label(current_parent_id.is_none(), "Root (No Folder)").clicked() {
                                                                state.document.set_layer_parent(layer_id, None);
                                                            }
                                                            for (gid, gname) in &group_options {
                                                                if *gid != layer_id {
                                                                    if ui.selectable_label(current_parent_id == Some(*gid), format!("📁 {}", gname)).clicked() {
                                                                        state.document.set_layer_parent(layer_id, Some(*gid));
                                                                    }
                                                                }
                                                            }
                                                        });
                                                });
                                            }
                                        }
                                    }
                                });
                        });
                        ui.add_space(2.0);
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
                        if ui.button("⛶ Fit").clicked() {
                            let sr = ctx.screen_rect();
                            state.reset_view_centered(sr.width(), sr.height());
                        }
                        if ui.button("100%").clicked() {
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
            .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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
            .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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

    // ── 7.5 FLOATING COLOR MIXING SCRATCHPAD DOCK ──
    if state.show_scratchpad {
        // Upload/refresh texture when created or marked dirty
        if state.scratchpad_texture.is_none() || state.scratchpad_texture_dirty {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [state.scratchpad_w as usize, state.scratchpad_h as usize],
                &state.scratchpad_pixels,
            );
            match &mut state.scratchpad_texture {
                Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
                None => {
                    state.scratchpad_texture = Some(ctx.load_texture(
                        "scratchpad_texture",
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            }
            state.scratchpad_texture_dirty = false;
        }

        let mut is_open = state.show_scratchpad;
        egui::Window::new("🎨 Mixing Scratchpad")
            .open(&mut is_open)
            .default_size(Vec2::new(360.0, 420.0))
            .resizable(true)
            .show(ctx, |ui| {
                // Toolbar: Tools & Clear
                ui.horizontal(|ui| {
                    let modes = [
                        (0, "🖌️ Paint"),
                        (1, "💧 Blend"),
                        (2, "👁 Eyedrop"),
                        (3, "🧹 Erase"),
                    ];
                    for (m, lbl) in modes {
                        let sel = state.scratchpad_mode == m;
                        if ui.selectable_label(sel, lbl).clicked() {
                            state.scratchpad_mode = m;
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("🗑 Clear").on_hover_text("Clear Scratchpad Canvas").clicked() {
                            state.clear_scratchpad(state.scratchpad_bg_mode);
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Size:").size(10.0).color(Color32::from_rgb(150, 165, 195)));
                    ui.add(egui::Slider::new(&mut state.scratchpad_brush_size, 2.0..=64.0).suffix("px"));

                    ui.separator();
                    ui.label(RichText::new("Canvas:").size(10.0).color(Color32::from_rgb(150, 165, 195)));
                    if ui.selectable_label(state.scratchpad_bg_mode == 0, "Dark").clicked() {
                        state.clear_scratchpad(0);
                    }
                    if ui.selectable_label(state.scratchpad_bg_mode == 1, "White").clicked() {
                        state.clear_scratchpad(1);
                    }
                    if ui.selectable_label(state.scratchpad_bg_mode == 2, "Grid").clicked() {
                        state.clear_scratchpad(2);
                    }
                });

                ui.separator();

                // Scratchpad Canvas Drawing Area
                let avail_w = ui.available_width().max(100.0);
                let avail_h = (ui.available_height() - 4.0).max(100.0);
                let pad_size = avail_w.min(avail_h);
                let (rect, response) = ui.allocate_exact_size(egui::vec2(pad_size, pad_size), egui::Sense::click_and_drag());

                if let Some(tex) = &state.scratchpad_texture {
                    let painter = ui.painter_at(rect);
                    // Draw background frame
                    painter.rect_filled(rect, 4.0, Color32::from_rgb(16, 20, 32));
                    // Paint scratchpad texture
                    painter.image(
                        tex.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0_f32, Color32::from_rgb(50, 65, 100)));
                }

                // Interaction Handling
                if response.hovered() || response.dragged() {
                    if let Some(mouse_pos) = response.interact_pointer_pos() {
                        let norm_x = ((mouse_pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                        let norm_y = ((mouse_pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
                        let sp_x = norm_x * (state.scratchpad_w as f32);
                        let sp_y = norm_y * (state.scratchpad_h as f32);
                        let current_pt = glam::Vec2::new(sp_x, sp_y);

                        let is_alt = ui.input(|i| i.modifiers.alt);
                        if is_alt || state.scratchpad_mode == 2 {
                            if response.clicked() || response.dragged() {
                                state.sample_scratchpad_pixel(sp_x.round() as u32, sp_y.round() as u32);
                            }
                        } else if response.drag_started() {
                            state.paint_scratchpad_point(current_pt, None, 1.0);
                            state.scratchpad_last_pos = Some(current_pt);
                        } else if response.dragged() {
                            if let Some(last) = state.scratchpad_last_pos {
                                state.paint_scratchpad_stroke(last, current_pt, 1.0);
                            } else {
                                state.paint_scratchpad_point(current_pt, None, 1.0);
                            }
                            state.scratchpad_last_pos = Some(current_pt);
                        } else if response.clicked() {
                            state.paint_scratchpad_point(current_pt, None, 1.0);
                        }
                    }
                }

                if response.drag_stopped() {
                    state.scratchpad_last_pos = None;
                }
            });
        state.show_scratchpad = is_open;
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
                .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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
                .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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
                .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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
                .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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
                .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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
                .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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
                .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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
                .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
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

    // ── 9. FREE TRANSFORM 8-POINT GIZMO & FLOATING HUD ──
    if state.transform_session.is_active {
        let win_size = ctx.screen_rect().size();
        let win_w = win_size.x;
        let win_h = win_size.y;

        let to_screen_pos = |cv_pt: glam::Vec2| -> egui::Pos2 {
            let s_pt = state.canvas_to_screen(cv_pt, win_w, win_h);
            egui::pos2(s_pt.x, s_pt.y)
        };

        let pw = state.transform_session.patch_w as f32;
        let ph = state.transform_session.patch_h as f32;
        let origin = state.transform_session.patch_origin;
        let tf = &state.transform_session.transform;

        let local_corners = [
            origin,
            origin + glam::Vec2::new(pw, 0.0),
            origin + glam::Vec2::new(pw, ph),
            origin + glam::Vec2::new(0.0, ph),
        ];

        let screen_corners = [
            to_screen_pos(tf.forward(local_corners[0])),
            to_screen_pos(tf.forward(local_corners[1])),
            to_screen_pos(tf.forward(local_corners[2])),
            to_screen_pos(tf.forward(local_corners[3])),
        ];

        let screen_tc = egui::pos2(
            (screen_corners[0].x + screen_corners[1].x) * 0.5,
            (screen_corners[0].y + screen_corners[1].y) * 0.5,
        );
        let screen_mr = egui::pos2(
            (screen_corners[1].x + screen_corners[2].x) * 0.5,
            (screen_corners[1].y + screen_corners[2].y) * 0.5,
        );
        let screen_bc = egui::pos2(
            (screen_corners[2].x + screen_corners[3].x) * 0.5,
            (screen_corners[2].y + screen_corners[3].y) * 0.5,
        );
        let screen_ml = egui::pos2(
            (screen_corners[3].x + screen_corners[0].x) * 0.5,
            (screen_corners[3].y + screen_corners[0].y) * 0.5,
        );

        let stem_dir = (screen_tc - screen_bc).normalized();
        let screen_stem = screen_tc + stem_dir * 28.0;
        let screen_pivot = to_screen_pos(tf.pivot + tf.translation);

        // Render Gizmo lines & handles using foreground painter
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("transform_gizmo_layer")));

        let border_stroke = egui::Stroke::new(1.5_f32, Color32::from_rgb(0, 240, 255));
        painter.line_segment([screen_corners[0], screen_corners[1]], border_stroke);
        painter.line_segment([screen_corners[1], screen_corners[2]], border_stroke);
        painter.line_segment([screen_corners[2], screen_corners[3]], border_stroke);
        painter.line_segment([screen_corners[3], screen_corners[0]], border_stroke);

        let stem_stroke = egui::Stroke::new(1.2_f32, Color32::from_rgb(0, 200, 255));
        painter.line_segment([screen_tc, screen_stem], stem_stroke);

        let handle_fill = Color32::from_rgb(10, 18, 36);
        let handle_stroke = egui::Stroke::new(1.5_f32, Color32::from_rgb(0, 240, 255));
        let handle_size = 8.0_f32;

        let draw_square_handle = |p: egui::Pos2| {
            let r = egui::Rect::from_center_size(p, egui::vec2(handle_size, handle_size));
            painter.rect_filled(r, 1.5_f32, handle_fill);
            painter.rect_stroke(r, 1.5_f32, handle_stroke);
        };

        draw_square_handle(screen_corners[0]);
        draw_square_handle(screen_tc);
        draw_square_handle(screen_corners[1]);
        draw_square_handle(screen_mr);
        draw_square_handle(screen_corners[2]);
        draw_square_handle(screen_bc);
        draw_square_handle(screen_corners[3]);
        draw_square_handle(screen_ml);

        // Rotation Handle (Circle)
        painter.circle_filled(screen_stem, 5.5_f32, Color32::from_rgb(0, 240, 255));
        painter.circle_stroke(screen_stem, 5.5_f32, egui::Stroke::new(1.5_f32, Color32::from_rgb(255, 255, 255)));

        // Pivot Handle (Crosshair circle)
        painter.circle_stroke(screen_pivot, 6.0_f32, egui::Stroke::new(1.2_f32, Color32::from_rgb(255, 180, 0)));
        painter.line_segment(
            [screen_pivot - egui::vec2(8.0, 0.0), screen_pivot + egui::vec2(8.0, 0.0)],
            egui::Stroke::new(1.2_f32, Color32::from_rgb(255, 180, 0)),
        );
        painter.line_segment(
            [screen_pivot - egui::vec2(0.0, 8.0), screen_pivot + egui::vec2(0.0, 8.0)],
            egui::Stroke::new(1.2_f32, Color32::from_rgb(255, 180, 0)),
        );

        // Floating Transform HUD Toolbar at Bottom-Center
        let hud_pos = egui::pos2((win_w - 480.0) * 0.5, win_h - 90.0);
        egui::Area::new("transform_hud_area".into())
            .fixed_pos(hud_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(10, 14, 28, 245))
                    .stroke(egui::Stroke::new(1.5_f32, Color32::from_rgb(0, 240, 255)))
                    .rounding(8.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("✦ TRANSFORM").strong().color(Color32::from_rgb(0, 240, 255)));
                            ui.separator();

                            if ui.button(RichText::new("✓ Apply (Enter)").strong().color(Color32::from_rgb(80, 240, 140))).clicked() {
                                state.commit_transform_session();
                                return;
                            }
                            if ui.button(RichText::new("✕ Cancel (Esc)").strong().color(Color32::from_rgb(255, 100, 100))).clicked() {
                                state.cancel_transform_session();
                                return;
                            }

                            ui.separator();

                            let mut rot_deg = state.transform_session.transform.rotation_rad.to_degrees();
                            if ui.add(egui::DragValue::new(&mut rot_deg).speed(0.5).prefix("Angle: ").suffix("°")).changed() {
                                state.transform_session.transform.rotation_rad = rot_deg.to_radians();
                                state.update_transform_preview();
                            }

                            let mut scale_pct = state.transform_session.transform.scale.x * 100.0;
                            if ui.add(egui::DragValue::new(&mut scale_pct).speed(1.0).range(5.0..=500.0).prefix("Scale: ").suffix("%")).changed() {
                                let s = scale_pct / 100.0;
                                state.transform_session.transform.scale = glam::Vec2::new(s, s);
                                state.update_transform_preview();
                            }

                            if ui.button("⇄ Flip H").clicked() {
                                state.transform_session.transform.flip_h = !state.transform_session.transform.flip_h;
                                state.update_transform_preview();
                            }
                            if ui.button("⇅ Flip V").clicked() {
                                state.transform_session.transform.flip_v = !state.transform_session.transform.flip_v;
                                state.update_transform_preview();
                            }
                        });
                    });
            });
    }

    // ── 9.4 SELECTION BOUNDARY MARCHING ANTS OVERLAY ──
    if let Some(mask) = &state.selection {
        if mask.has_selection() {
            let win_size = ctx.screen_rect().size();
            let win_w = win_size.x;
            let win_h = win_size.y;
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("selection_boundary_layer")));

            let time = ctx.input(|i| i.time);
            let phase = (time * 6.0).fract() as f32;
            let c_pri = Color32::from_rgb(0, 240, 255);
            let c_sec = Color32::from_rgb(255, 255, 255);

            for (i, &(p0, p1)) in mask.cached_boundary.iter().enumerate() {
                let s0 = state.canvas_to_screen(p0, win_w, win_h);
                let s1 = state.canvas_to_screen(p1, win_w, win_h);
                let is_pri = (((i as f32) * 0.5 + phase * 4.0) as usize) % 2 == 0;
                let stroke_c = if is_pri { c_pri } else { c_sec };
                painter.line_segment([egui::pos2(s0.x, s0.y), egui::pos2(s1.x, s1.y)], egui::Stroke::new(1.3_f32, stroke_c));
            }
            ctx.request_repaint();

            // Floating Selection HUD (unless Transform tool is currently HUD-active)
            if !state.transform_session.is_active && state.show_ui_panels {
                let hud_pos = egui::pos2((win_w - 360.0) * 0.5, win_h - 60.0);
                egui::Area::new("selection_hud_area".into())
                    .fixed_pos(hud_pos)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_unmultiplied(10, 14, 28, 240))
                            .stroke(egui::Stroke::new(1.2_f32, Color32::from_rgb(0, 240, 255)))
                            .rounding(7.0)
                            .inner_margin(6.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("✦ SELECTION").size(10.5).strong().color(Color32::from_rgb(0, 240, 255)));
                                    ui.separator();
                                    if ui.button(RichText::new("Fill").strong().color(Color32::from_rgb(100, 240, 150))).clicked() {
                                        state.fill_selection_active_layer();
                                    }
                                    if ui.button("Stroke...").clicked() {
                                        state.show_stroke_dialog = true;
                                    }
                                    if ui.button("Feather...").clicked() {
                                        state.show_feather_dialog = true;
                                    }
                                    ui.separator();
                                    if ui.button(RichText::new("✕ Deselect (Ctrl+D)").strong().color(Color32::from_rgb(255, 120, 120))).clicked() {
                                        state.selection = None;
                                        state.set_status("Deselected");
                                    }
                                });
                            });
                    });
            }
        }
    }

    // ── 9.5 LASSO PREVIEW OVERLAY ──
    if !state.lasso_points.is_empty() {
        let win_size = ctx.screen_rect().size();
        let win_w = win_size.x;
        let win_h = win_size.y;
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lasso_preview_layer")));
        let stroke = egui::Stroke::new(1.5_f32, Color32::from_rgb(0, 240, 255));

        for i in 0..state.lasso_points.len() - 1 {
            let p0 = state.canvas_to_screen(state.lasso_points[i], win_w, win_h);
            let p1 = state.canvas_to_screen(state.lasso_points[i + 1], win_w, win_h);
            painter.line_segment([egui::pos2(p0.x, p0.y), egui::pos2(p1.x, p1.y)], stroke);
        }
        if state.lasso_points.len() >= 3 {
            let p_last = state.canvas_to_screen(*state.lasso_points.last().unwrap(), win_w, win_h);
            let p_first = state.canvas_to_screen(state.lasso_points[0], win_w, win_h);
            painter.line_segment(
                [egui::pos2(p_last.x, p_last.y), egui::pos2(p_first.x, p_first.y)],
                egui::Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 240, 255, 120)),
            );
        }
    }

    // ── 9.6 SELECTION MODIFIER MODALS ──
    // Feather Selection Modal
    if state.show_feather_dialog {
        egui::Window::new("Feather Selection")
            .fixed_size(Vec2::new(320.0, 150.0))
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                ui.label(RichText::new("Feather Radius (px):").size(11.0).color(Color32::from_rgb(205, 215, 240)));
                ui.add(egui::Slider::new(&mut state.feather_radius, 1..=50).text("px"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("✓ Apply Feather").strong().color(Color32::from_rgb(100, 240, 150))).clicked() {
                        let r = state.feather_radius;
                        state.feather_selection(r);
                        state.show_feather_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_feather_dialog = false;
                    }
                });
            });
    }

    // Expand Selection Modal
    if state.show_expand_dialog {
        egui::Window::new("Expand Selection")
            .fixed_size(Vec2::new(320.0, 150.0))
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                ui.label(RichText::new("Expand By (px):").size(11.0).color(Color32::from_rgb(205, 215, 240)));
                ui.add(egui::Slider::new(&mut state.expand_radius, 1..=50).text("px"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("✓ Apply Expand").strong().color(Color32::from_rgb(100, 240, 150))).clicked() {
                        let r = state.expand_radius;
                        state.expand_selection(r);
                        state.show_expand_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_expand_dialog = false;
                    }
                });
            });
    }

    // Contract Selection Modal
    if state.show_contract_dialog {
        egui::Window::new("Contract Selection")
            .fixed_size(Vec2::new(320.0, 150.0))
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                ui.label(RichText::new("Contract By (px):").size(11.0).color(Color32::from_rgb(205, 215, 240)));
                ui.add(egui::Slider::new(&mut state.contract_radius, 1..=50).text("px"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("✓ Apply Contract").strong().color(Color32::from_rgb(100, 240, 150))).clicked() {
                        let r = state.contract_radius;
                        state.contract_selection(r);
                        state.show_contract_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_contract_dialog = false;
                    }
                });
            });
    }

    // Stroke Selection Modal
    if state.show_stroke_dialog {
        egui::Window::new("Stroke Selection")
            .fixed_size(Vec2::new(340.0, 200.0))
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                ui.label(RichText::new("Stroke Width (px):").size(11.0).color(Color32::from_rgb(205, 215, 240)));
                ui.add(egui::Slider::new(&mut state.stroke_width, 1..=50).text("px"));
                ui.add_space(6.0);
                ui.label(RichText::new("Location:").size(11.0).color(Color32::from_rgb(205, 215, 240)));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut state.stroke_position, 0, "Center");
                    ui.radio_value(&mut state.stroke_position, 1, "Inside");
                    ui.radio_value(&mut state.stroke_position, 2, "Outside");
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("✓ Apply Stroke").strong().color(Color32::from_rgb(100, 240, 150))).clicked() {
                        let w = state.stroke_width;
                        let pos = state.stroke_position;
                        state.stroke_selection_active_layer(w, pos);
                        state.show_stroke_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_stroke_dialog = false;
                    }
                });
            });
    }

    // ── Save Brush Preset Modal ──
    if state.show_save_preset_dialog {
        egui::Window::new("💾 Save Custom Brush Preset")
            .fixed_size(Vec2::new(320.0, 140.0))
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                ui.label(RichText::new("Preset Name:").size(11.0).color(Color32::from_rgb(205, 215, 240)));
                ui.text_edit_singleline(&mut state.new_preset_name);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("✓ Save Preset").strong().color(Color32::from_rgb(100, 240, 150))).clicked() {
                        let name = state.new_preset_name.clone();
                        state.save_current_as_preset(&name);
                        state.show_save_preset_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_save_preset_dialog = false;
                    }
                });
            });
    }

    // ── Export Animation Modal ──
    if state.show_export_animation_dialog {
        egui::Window::new("🎬 Export Flipbook Animation")
            .fixed_size(Vec2::new(360.0, 240.0))
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                ui.label(RichText::new("Export Multi-Frame Animation").size(12.0).strong().color(Color32::from_rgb(235, 242, 255)));
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Total Frames:").size(11.0).color(Color32::from_rgb(180, 190, 215)));
                    ui.label(RichText::new(format!("{}", state.timeline.frames.len())).strong().color(accent_c32));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Playback FPS:").size(11.0).color(Color32::from_rgb(180, 190, 215)));
                    ui.add(egui::DragValue::new(&mut state.export_anim_fps).range(1..=60).suffix(" fps"));
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Format:").size(11.0).color(Color32::from_rgb(180, 190, 215)));
                    ui.radio_value(&mut state.export_anim_format, 0, "GIF (.gif)");
                    ui.radio_value(&mut state.export_anim_format, 1, "PNG Sequence");
                });

                ui.checkbox(&mut state.export_anim_loop, "Infinite Loop (Repeat)");
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui.button(RichText::new("✓ Export Animation").strong().color(Color32::from_rgb(100, 240, 150))).clicked() {
                        state.timeline.sync_from_document(&state.document);
                        let w = state.document.width;
                        let h = state.document.height;
                        let bg_val = state.document.background_value;
                        let inc_bg = !state.document.is_transparent;

                        let mut frames_rgba = Vec::with_capacity(state.timeline.frames.len());
                        for f in &state.timeline.frames {
                            let comp = f.composite_layers(w, h, inc_bg, bg_val);
                            frames_rgba.push(comp);
                        }

                        let out_dir = std::path::PathBuf::from("exports");
                        let _ = std::fs::create_dir_all(&out_dir);

                        if state.export_anim_format == 0 {
                            let out_path = out_dir.join("animation.gif");
                            match hollow_io::export_animated_gif(&frames_rgba, w, h, state.export_anim_fps, state.export_anim_loop, &out_path) {
                                Ok(_) => state.set_status(format!("Exported animated GIF to {}", out_path.display())),
                                Err(e) => state.set_status(format!("Export failed: {}", e)),
                            }
                        } else {
                            match hollow_io::export_frame_sequence(&frames_rgba, w, h, &out_dir, "frame", hollow_io::ExportFormat::Png) {
                                Ok(paths) => state.set_status(format!("Exported {} frames to {}", paths.len(), out_dir.display())),
                                Err(e) => state.set_status(format!("Export sequence failed: {}", e)),
                            }
                        }
                        state.show_export_animation_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_export_animation_dialog = false;
                    }
                });
            });
    }

    // ── 9. FLOATING ADJUSTMENT LAYER SETTINGS INSPECTOR ──
    if let Some(adj_id) = state.active_adjustment_modal {
        let mut close_modal = false;
        let mut title = "✨ Adjustment Layer Settings".to_string();
        if let Some(layer) = state.document.get_layer(adj_id) {
            if let Some(adj) = &layer.adjustment {
                title = format!("✨ {} ({})", adj.adjustment_type.name(), layer.name);
            }
        }

        let mut is_open = true;
        egui::Window::new(title)
            .open(&mut is_open)
            .fixed_size(Vec2::new(300.0, 240.0))
            .collapsible(false)
            .resizable(false)
            .default_pos(egui::pos2(ctx.screen_rect().width() - 560.0, 60.0))
            .show(ctx, |ui| {
                if let Some(layer) = state.document.get_layer_mut(adj_id) {
                    if let Some(adj) = &mut layer.adjustment {
                        match &mut adj.adjustment_type {
                            hollow_core::layer::AdjustmentType::BrightnessContrast { brightness, contrast } => {
                                ui.label(RichText::new("BRIGHTNESS & CONTRAST").size(9.0).strong().color(accent_c32));
                                ui.add(egui::Slider::new(brightness, -100.0..=100.0).text("Brightness"));
                                ui.add(egui::Slider::new(contrast, -100.0..=100.0).text("Contrast"));
                                if ui.button("Reset Values").clicked() {
                                    *brightness = 0.0;
                                    *contrast = 0.0;
                                }
                            }
                            hollow_core::layer::AdjustmentType::Hsl { hue_shift, saturation, lightness } => {
                                ui.label(RichText::new("HUE / SATURATION / LIGHTNESS").size(9.0).strong().color(accent_c32));
                                ui.add(egui::Slider::new(hue_shift, -180.0..=180.0).text("Hue (°)"));
                                ui.add(egui::Slider::new(saturation, 0.0..=3.0).text("Saturation (x)"));
                                ui.add(egui::Slider::new(lightness, -1.0..=1.0).text("Lightness"));
                                if ui.button("Reset Values").clicked() {
                                    *hue_shift = 0.0;
                                    *saturation = 1.0;
                                    *lightness = 0.0;
                                }
                            }
                            hollow_core::layer::AdjustmentType::ColorBalance { cyan_red, magenta_green, yellow_blue } => {
                                ui.label(RichText::new("RGB COLOR BALANCE").size(9.0).strong().color(accent_c32));
                                ui.add(egui::Slider::new(cyan_red, -100.0..=100.0).text("Cyan / Red"));
                                ui.add(egui::Slider::new(magenta_green, -100.0..=100.0).text("Magenta / Green"));
                                ui.add(egui::Slider::new(yellow_blue, -100.0..=100.0).text("Yellow / Blue"));
                                if ui.button("Reset Values").clicked() {
                                    *cyan_red = 0.0;
                                    *magenta_green = 0.0;
                                    *yellow_blue = 0.0;
                                }
                            }
                            hollow_core::layer::AdjustmentType::Invert => {
                                ui.label(RichText::new("INVERT COLORS").size(9.0).strong().color(accent_c32));
                                ui.label("Inverts all RGB chromatic channels beneath this layer in real time.");
                            }
                            hollow_core::layer::AdjustmentType::Posterize { levels } => {
                                ui.label(RichText::new("POSTERIZATION").size(9.0).strong().color(accent_c32));
                                ui.add(egui::Slider::new(levels, 2..=32).text("Tonal Levels"));
                            }
                            hollow_core::layer::AdjustmentType::Threshold { cutoff } => {
                                ui.label(RichText::new("BINARY THRESHOLD").size(9.0).strong().color(accent_c32));
                                ui.add(egui::Slider::new(cutoff, 0..=255).text("Cutoff"));
                            }
                            hollow_core::layer::AdjustmentType::Sepia { strength } => {
                                ui.label(RichText::new("VINTAGE SEPIA TONE").size(9.0).strong().color(accent_c32));
                                ui.add(egui::Slider::new(strength, 0.0..=1.0).text("Sepia Strength"));
                            }
                        }
                    }
                } else {
                    close_modal = true;
                }

                ui.add_space(8.0);
                ui.separator();
                if ui.button("Done / Close Inspector").clicked() {
                    close_modal = true;
                }
            });

        if close_modal || !is_open {
            state.active_adjustment_modal = None;
        }
    }

    // ── 10. FLOATING PERSPECTIVE STUDIO DOCK ──
    if state.show_perspective_dock {
        let mut is_open = state.show_perspective_dock;
        egui::Window::new("📐 Perspective Studio")
            .open(&mut is_open)
            .fixed_size(Vec2::new(310.0, 360.0))
            .default_pos(egui::pos2(ctx.screen_rect().width() - 570.0, 70.0))
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new("PERSPECTIVE PROJECTION MODE").size(9.0).strong().color(accent_c32));
                    ui.horizontal(|ui| {
                        let cur_type = state.perspective.p_type;
                        if ui.selectable_label(cur_type == hollow_core::perspective::PerspectiveType::None, "Off").clicked() {
                            state.perspective.p_type = hollow_core::perspective::PerspectiveType::None;
                        }
                        if ui.selectable_label(cur_type == hollow_core::perspective::PerspectiveType::OnePoint, "1-Point").clicked() {
                            state.perspective.reset_preset(hollow_core::perspective::PerspectiveType::OnePoint, state.document.width, state.document.height);
                        }
                        if ui.selectable_label(cur_type == hollow_core::perspective::PerspectiveType::TwoPoint, "2-Point").clicked() {
                            state.perspective.reset_preset(hollow_core::perspective::PerspectiveType::TwoPoint, state.document.width, state.document.height);
                        }
                        if ui.selectable_label(cur_type == hollow_core::perspective::PerspectiveType::ThreePoint, "3-Point").clicked() {
                            state.perspective.reset_preset(hollow_core::perspective::PerspectiveType::ThreePoint, state.document.width, state.document.height);
                        }
                    });

                    if state.perspective.p_type != hollow_core::perspective::PerspectiveType::None {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.label(RichText::new("CONSTRAINT SNAPPING ENGINE").size(9.0).strong().color(accent_c32));
                        ui.checkbox(&mut state.perspective.snap_enabled, "⇲ Enable Perspective Snapping (Ctrl+Shift+P)");
                        ui.add(egui::Slider::new(&mut state.perspective.snap_strength, 0.1..=1.0).text("Snap Strictness"));

                        ui.add_space(6.0);
                        ui.separator();
                        ui.label(RichText::new("GUIDE OVERLAY RENDERING").size(9.0).strong().color(accent_c32));
                        ui.checkbox(&mut state.perspective.show_guides, "Show Guide Overlay");
                        ui.add(egui::Slider::new(&mut state.perspective.guide_density, 6..=36).text("Ray Density"));
                        ui.add(egui::Slider::new(&mut state.perspective.guide_opacity, 0.05..=1.0).text("Guide Opacity"));

                        ui.add_space(6.0);
                        ui.separator();
                        ui.label(RichText::new("HORIZON & VANISHING POINTS").size(9.0).strong().color(accent_c32));
                        let doc_h = state.document.height as f32;
                        ui.add(egui::Slider::new(&mut state.perspective.horizon_y, -500.0..=(doc_h * 2.0)).text("Horizon Y"));
                        ui.add(egui::Slider::new(&mut state.perspective.horizon_angle, -45.0..=45.0).text("Horizon Tilt (°)"));

                        ui.add_space(4.0);
                        ui.label(RichText::new("VP 1 (Primary / Center)").size(8.5).color(Color32::from_rgb(100, 200, 255)));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut state.perspective.vp1.x).prefix("X: ").speed(1.0));
                            ui.add(egui::DragValue::new(&mut state.perspective.vp1.y).prefix("Y: ").speed(1.0));
                        });

                        if state.perspective.p_type == hollow_core::perspective::PerspectiveType::TwoPoint
                            || state.perspective.p_type == hollow_core::perspective::PerspectiveType::ThreePoint
                        {
                            ui.label(RichText::new("VP 2 (Right Vanishing Point)").size(8.5).color(Color32::from_rgb(80, 230, 180)));
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut state.perspective.vp2.x).prefix("X: ").speed(1.0));
                                ui.add(egui::DragValue::new(&mut state.perspective.vp2.y).prefix("Y: ").speed(1.0));
                            });
                        }

                        if state.perspective.p_type == hollow_core::perspective::PerspectiveType::ThreePoint {
                            ui.label(RichText::new("VP 3 (Vertical / Nadir Point)").size(8.5).color(Color32::from_rgb(230, 120, 240)));
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut state.perspective.vp3.x).prefix("X: ").speed(1.0));
                                ui.add(egui::DragValue::new(&mut state.perspective.vp3.y).prefix("Y: ").speed(1.0));
                            });
                        }

                        ui.add_space(6.0);
                        if ui.button("Reset Vanishing Points to Center").clicked() {
                            state.perspective.init_for_canvas(state.document.width, state.document.height);
                        }
                    }
                });
            });
        state.show_perspective_dock = is_open;
    }

    // ── 10.5. FLOATING TRANSFORM & MESH WARP STUDIO HUD ──
    if state.transform_session.is_active {
        egui::Window::new("⛶ Transform & Warp Studio")
            .fixed_size(Vec2::new(340.0, 220.0))
            .collapsible(false)
            .default_pos(egui::pos2((ctx.screen_rect().width() - 340.0) * 0.5, 52.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let cur_mode = state.transform_session.mode;
                    if ui.selectable_label(cur_mode == hollow_core::transform::TransformMode::Affine, "⛶ Free").on_hover_text("Standard Affine Box (Scale, Rotate, Skew, Translate)").clicked() {
                        state.set_transform_mode(hollow_core::transform::TransformMode::Affine);
                    }
                    if ui.selectable_label(cur_mode == hollow_core::transform::TransformMode::PerspectiveQuad, "☖ Quad").on_hover_text("4-Corner Perspective Quad Warping").clicked() {
                        state.set_transform_mode(hollow_core::transform::TransformMode::PerspectiveQuad);
                    }
                    if ui.selectable_label(cur_mode == hollow_core::transform::TransformMode::MeshGrid, "▦ Mesh").on_hover_text("Bicubic Mesh Grid Deformation").clicked() {
                        state.set_transform_mode(hollow_core::transform::TransformMode::MeshGrid);
                    }
                    if ui.selectable_label(cur_mode == hollow_core::transform::TransformMode::ThinPlateSpline, "✦ TPS Pins").on_hover_text("Thin Plate Spline Multi-Pin Landmark Warping").clicked() {
                        state.set_transform_mode(hollow_core::transform::TransformMode::ThinPlateSpline);
                    }
                });

                ui.add_space(4.0);
                ui.separator();

                match state.transform_session.mode {
                    hollow_core::transform::TransformMode::Affine => {
                        let mut rot_deg = state.transform_session.transform.rotation_rad.to_degrees();
                        if ui.add(egui::Slider::new(&mut rot_deg, -180.0..=180.0).text("Angle (°)")).changed() {
                            state.transform_session.transform.rotation_rad = rot_deg.to_radians();
                            state.update_transform_preview();
                        }
                        let mut scale_pct = state.transform_session.transform.scale.x * 100.0;
                        if ui.add(egui::Slider::new(&mut scale_pct, 5.0..=400.0).text("Scale (%)")).changed() {
                            let s = scale_pct / 100.0;
                            state.transform_session.transform.scale = glam::Vec2::new(s, s);
                            state.update_transform_preview();
                        }
                        ui.horizontal(|ui| {
                            if ui.button("⇄ Flip H").clicked() {
                                state.transform_session.transform.flip_h = !state.transform_session.transform.flip_h;
                                state.update_transform_preview();
                            }
                            if ui.button("⇅ Flip V").clicked() {
                                state.transform_session.transform.flip_v = !state.transform_session.transform.flip_v;
                                state.update_transform_preview();
                            }
                            ui.checkbox(&mut state.transform_session.lock_aspect, "Lock Ratio");
                        });
                    }
                    hollow_core::transform::TransformMode::PerspectiveQuad => {
                        ui.label(RichText::new("Drag the 4 corner handles on canvas to create perspective planes.").size(9.5).color(Color32::from_rgb(170, 185, 220)));
                        if ui.button("↺ Reset Quad Corners").clicked() {
                            state.reset_transform_deformation();
                        }
                    }
                    hollow_core::transform::TransformMode::MeshGrid => {
                        ui.horizontal(|ui| {
                            ui.label("Density:");
                            let densities = [(3, 3, "3×3"), (4, 4, "4×4"), (5, 5, "5×5"), (6, 6, "6×6"), (8, 8, "8×8")];
                            for (r, c, lbl) in densities {
                                let is_active = state.transform_session.grid_rows == r && state.transform_session.grid_cols == c;
                                if ui.selectable_label(is_active, lbl).clicked() {
                                    state.set_transform_grid_density(r, c);
                                }
                            }
                        });
                        if ui.button("↺ Reset Mesh Grid").clicked() {
                            state.reset_transform_deformation();
                        }
                    }
                    hollow_core::transform::TransformMode::ThinPlateSpline => {
                        ui.label(RichText::new(format!("Active Landmark Pins: {}", state.transform_session.tps_target_pins.len())).size(9.5).color(accent_c32));
                        ui.horizontal(|ui| {
                            if ui.button("+ Add Center Pin").clicked() {
                                let center = state.transform_session.patch_origin + glam::Vec2::new(
                                    state.transform_session.patch_w as f32 * 0.5,
                                    state.transform_session.patch_h as f32 * 0.5,
                                );
                                state.add_tps_pin(center);
                            }
                            if ui.button("↺ Reset Pins").clicked() {
                                state.reset_transform_deformation();
                            }
                        });
                    }
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Interpolation:");
                    let interp = state.transform_session.interpolation;
                    if ui.selectable_label(interp == hollow_core::transform::InterpolationMode::Bicubic, "Bicubic").on_hover_text("High-quality smooth Catmull-Rom cubic sampling").clicked() {
                        state.transform_session.interpolation = hollow_core::transform::InterpolationMode::Bicubic;
                        state.update_transform_preview();
                    }
                    if ui.selectable_label(interp == hollow_core::transform::InterpolationMode::Bilinear, "Bilinear").clicked() {
                        state.transform_session.interpolation = hollow_core::transform::InterpolationMode::Bilinear;
                        state.update_transform_preview();
                    }
                    if ui.selectable_label(interp == hollow_core::transform::InterpolationMode::Nearest, "Nearest").on_hover_text("Crisp pixel art nearest neighbor").clicked() {
                        state.transform_session.interpolation = hollow_core::transform::InterpolationMode::Nearest;
                        state.update_transform_preview();
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("✓ Apply (Enter)").strong().color(Color32::from_rgb(100, 240, 150))).clicked() {
                        state.commit_transform_session();
                    }
                    if ui.button(RichText::new("✕ Cancel (Esc)").strong().color(Color32::from_rgb(250, 120, 120))).clicked() {
                        state.cancel_transform_session();
                    }
                });
            });
    }

    // ── ON-CANVAS TRANSFORM GIZMOS ──
    if state.transform_session.is_active {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("transform_gizmos_layer")));
        let screen_rect = ctx.screen_rect();
        render_transform_gizmos(&painter, state, screen_rect.width(), screen_rect.height(), accent_c32);
    }

    // ── 11. ABOUT HOLLOW CANVAS MODAL ──
    if state.show_about_dialog {
        egui::Window::new("About Hollow Canvas")
            .fixed_size(Vec2::new(430.0, 340.0))
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.screen_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("HOLLOW CANVAS").size(18.0).strong().color(Color32::from_rgb(235, 242, 255)));
                    ui.label(RichText::new("Digital Illustration & Graphics Studio").size(11.0).color(accent_c32));
                    ui.label(RichText::new("Version 0.17.0 · Pure Native Rust").size(10.0).color(Color32::from_rgb(130, 142, 172)));
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                ui.label(RichText::new("Features & Guarantees:").size(10.0).strong().color(Color32::from_rgb(205, 215, 240)));
                ui.label(RichText::new("• ⛶ Free Transform & Mesh Warp: Affine, Perspective Quad, Bicubic Mesh Grid & Thin Plate Splines (TPS)\n• 📐 Visual Perspective Rulers: 1, 2, and 3-Point Vanishing Guides with Dynamic Stroke Constraint Snapping\n• ✨ Real-Time Non-Destructive Adjustment Layers (Brightness/Contrast, HSL, Color Balance, Invert, Posterize, Threshold, Sepia)\n• 🎨 Floating Mixing Scratchpad: Off-canvas color mixing, pigment smudging, and eyedropper dock\n• 🎯 Subpixel-Antialiased Stroke Engine with True Tangent Catmull-Rom Curvature\n• ⚡ S-Level Stroke Stabilization (S-0 .. S-7 Lazy Rope & Tremor Filtering)\n• 📁 Collapsible Nested Layer Groups & Clipping Masks\n• 🪄 Magic Wand, Gradients, Shapes, Multi-Axis Symmetry\n• 🔒 100% Local-First: Completely offline, zero telemetry, zero trackers\n• 📦 Signed VPack 2.0.0 & Zip portable distribution (Ed25519 Chain of Trust)").size(10.0).color(Color32::from_rgb(155, 165, 195)));

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

    // ── 12. HELP & SHORTCUTS MODAL ──
    if state.show_help {
        egui::Window::new("Hollow Canvas Studio · Shortcuts & Help")
            .fixed_size(Vec2::new(420.0, 400.0))
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
                        ("L", "Freehand Lasso Selection Tool"),
                        ("X", "Swap Primary & Secondary Colors"),
                        ("F4", "Toggle Floating Mixing Scratchpad"),
                        ("F5", "Toggle Perspective Studio Dock"),
                        ("Ctrl + Shift + P", "Toggle Perspective Vector Snapping"),
                        ("Space + Drag", "Pan Canvas Viewport"),
                        ("Mouse Wheel", "Zoom Canvas In / Out"),
                        ("Ctrl + N", "Create New Canvas"),
                        ("Ctrl + S", "Save Project (.hcv)"),
                        ("Ctrl + O", "Open Project (.hcv)"),
                        ("Ctrl + E", "Export PNG Image"),
                        ("Ctrl + T", "Free Transform & Mesh Warp Layer / Selection"),
                        ("Ctrl + A", "Select All"),
                        ("Ctrl + I", "Invert Layer Colors"),
                        ("Ctrl + Z", "Undo Action"),
                        ("Ctrl + Y / Ctrl+Shift+Z", "Redo Action"),
                        ("Ctrl + D", "Deselect"),
                        ("Shift + F5", "Fill Selection with Primary Color"),
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

/// Renders on-canvas interactive transform gizmos, wireframe grids, handles, and TPS pin vectors
fn render_transform_gizmos(
    painter: &egui::Painter,
    state: &AppState,
    win_w: f32,
    win_h: f32,
    accent_c32: Color32,
) {
    if !state.transform_session.is_active {
        return;
    }

    let session = &state.transform_session;
    let white = Color32::from_rgb(255, 255, 255);
    let shadow = Color32::from_rgba_unmultiplied(0, 0, 0, 180);
    let cyan = Color32::from_rgb(80, 230, 255);

    match session.mode {
        hollow_core::transform::TransformMode::Affine => {
            let origin = session.patch_origin;
            let pw = session.patch_w as f32;
            let ph = session.patch_h as f32;

            let corners_canvas = [
                origin,
                origin + glam::Vec2::new(pw, 0.0),
                origin + glam::Vec2::new(pw, ph),
                origin + glam::Vec2::new(0.0, ph),
            ];

            let corners_screen: Vec<egui::Pos2> = corners_canvas
                .iter()
                .map(|&c| {
                    let tc = session.transform.forward(c);
                    let sp = state.canvas_to_screen(tc, win_w, win_h);
                    egui::pos2(sp.x, sp.y)
                })
                .collect();

            // Draw bounding quad
            for i in 0..4 {
                let p0 = corners_screen[i];
                let p1 = corners_screen[(i + 1) % 4];
                painter.line_segment([p0, p1], Stroke::new(2.5_f32, shadow));
                painter.line_segment([p0, p1], Stroke::new(1.5_f32, accent_c32));
            }

            // Draw rotation stem
            let top_mid = (corners_screen[0].to_vec2() + corners_screen[1].to_vec2()) * 0.5;
            let edge_dir = (corners_screen[1] - corners_screen[0]).normalized();
            let normal = egui::vec2(-edge_dir.y, edge_dir.x);
            let rot_handle = top_mid + normal * 24.0;
            painter.line_segment([top_mid.to_pos2(), rot_handle.to_pos2()], Stroke::new(2.0_f32, shadow));
            painter.line_segment([top_mid.to_pos2(), rot_handle.to_pos2()], Stroke::new(1.2_f32, accent_c32));
            painter.circle_filled(rot_handle.to_pos2(), 5.0, accent_c32);
            painter.circle_stroke(rot_handle.to_pos2(), 5.0, Stroke::new(1.5_f32, white));

            // Draw 8 handle boxes
            let mut handles = Vec::new();
            handles.extend(corners_screen.iter().cloned());
            for i in 0..4 {
                let mid = (corners_screen[i].to_vec2() + corners_screen[(i + 1) % 4].to_vec2()) * 0.5;
                handles.push(mid.to_pos2());
            }

            for hp in handles {
                let r = Rect::from_center_size(hp, egui::vec2(8.0, 8.0));
                painter.rect_filled(r, 1.0, shadow);
                painter.rect_filled(r.shrink(1.0), 1.0, white);
                painter.rect_stroke(r.shrink(1.0), 1.0, Stroke::new(1.0_f32, accent_c32));
            }

            // Draw pivot point
            let pivot_sp = state.canvas_to_screen(session.transform.pivot + session.transform.translation, win_w, win_h);
            painter.circle_stroke(egui::pos2(pivot_sp.x, pivot_sp.y), 6.0, Stroke::new(1.5_f32, white));
            painter.circle_filled(egui::pos2(pivot_sp.x, pivot_sp.y), 2.0, accent_c32);
        }
        hollow_core::transform::TransformMode::PerspectiveQuad => {
            let corners_screen: Vec<egui::Pos2> = session
                .quad
                .dst_corners
                .iter()
                .map(|&c| {
                    let sp = state.canvas_to_screen(c, win_w, win_h);
                    egui::pos2(sp.x, sp.y)
                })
                .collect();

            // Draw quad outline
            for i in 0..4 {
                let p0 = corners_screen[i];
                let p1 = corners_screen[(i + 1) % 4];
                painter.line_segment([p0, p1], Stroke::new(2.5_f32, shadow));
                painter.line_segment([p0, p1], Stroke::new(1.5_f32, cyan));
            }

            // Draw 4 corner handles with glowing cyan rings
            for (i, &cp) in corners_screen.iter().enumerate() {
                painter.circle_filled(cp, 7.0, Color32::from_rgb(15, 25, 45));
                painter.circle_stroke(cp, 7.0, Stroke::new(2.0_f32, cyan));
                painter.circle_filled(cp, 3.5, white);
                painter.text(
                    cp + egui::vec2(10.0, -10.0),
                    egui::Align2::LEFT_CENTER,
                    format!("C{}", i + 1),
                    egui::FontId::proportional(10.0),
                    cyan,
                );
            }
        }
        hollow_core::transform::TransformMode::MeshGrid => {
            let mesh = &session.mesh_grid;
            let rows = mesh.rows;
            let cols = mesh.cols;

            // Draw horizontal mesh lines
            for r in 0..rows {
                for c in 0..(cols - 1) {
                    let p0 = state.canvas_to_screen(mesh.get_vertex(r, c), win_w, win_h);
                    let p1 = state.canvas_to_screen(mesh.get_vertex(r, c + 1), win_w, win_h);
                    painter.line_segment([egui::pos2(p0.x, p0.y), egui::pos2(p1.x, p1.y)], Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(100, 180, 255, 160)));
                }
            }

            // Draw vertical mesh lines
            for c in 0..cols {
                for r in 0..(rows - 1) {
                    let p0 = state.canvas_to_screen(mesh.get_vertex(r, c), win_w, win_h);
                    let p1 = state.canvas_to_screen(mesh.get_vertex(r + 1, c), win_w, win_h);
                    painter.line_segment([egui::pos2(p0.x, p0.y), egui::pos2(p1.x, p1.y)], Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(100, 180, 255, 160)));
                }
            }

            // Draw vertex pin dots
            for r in 0..rows {
                for c in 0..cols {
                    let v_can = mesh.get_vertex(r, c);
                    let v_sp = state.canvas_to_screen(v_can, win_w, win_h);
                    let pos = egui::pos2(v_sp.x, v_sp.y);
                    let is_corner = (r == 0 || r == rows - 1) && (c == 0 || c == cols - 1);
                    let radius = if is_corner { 5.0 } else { 4.0 };
                    let fill = if is_corner { cyan } else { accent_c32 };
                    painter.circle_filled(pos, radius, shadow);
                    painter.circle_filled(pos, radius - 1.0, fill);
                    painter.circle_stroke(pos, radius, Stroke::new(1.0_f32, white));
                }
            }
        }
        hollow_core::transform::TransformMode::ThinPlateSpline => {
            for (i, (&src_can, &tgt_can)) in session.tps_source_pins.iter().zip(session.tps_target_pins.iter()).enumerate() {
                let src_sp = state.canvas_to_screen(src_can, win_w, win_h);
                let tgt_sp = state.canvas_to_screen(tgt_can, win_w, win_h);
                let p_src = egui::pos2(src_sp.x, src_sp.y);
                let p_tgt = egui::pos2(tgt_sp.x, tgt_sp.y);

                if p_src.distance(p_tgt) > 1.0 {
                    painter.line_segment([p_src, p_tgt], Stroke::new(1.5_f32, Color32::from_rgb(255, 200, 50)));
                    painter.circle_stroke(p_src, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(180, 180, 180)));
                }

                painter.circle_filled(p_tgt, 6.0, Color32::from_rgb(20, 25, 45));
                painter.circle_stroke(p_tgt, 6.0, Stroke::new(2.0_f32, Color32::from_rgb(255, 180, 40)));
                painter.circle_filled(p_tgt, 3.0, white);
                painter.text(
                    p_tgt + egui::vec2(8.0, -8.0),
                    egui::Align2::LEFT_CENTER,
                    format!("P{}", i + 1),
                    egui::FontId::proportional(9.0),
                    Color32::from_rgb(255, 210, 80),
                );
            }
        }
    }
}
