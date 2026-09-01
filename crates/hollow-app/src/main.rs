use glam::Vec2;
use hollow_core::brush::{BrushPoint, ToolType};
use hollow_core::history::LayerPixelsSnapshotCommand;
use hollow_core::rasterizer::StrokeRasterizer;
use hollow_core::selection::SelectionMask;
use hollow_io::export::{export_flat_image, ExportFormat};
use hollow_io::project::{load_project_file, save_project_file};
use hollow_render::SoftwareRenderer;
use hollow_ui::{
    configure_hollow_style, render_ui, AppState, PendingFileAction,
};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::Instant;

type HWND = *mut std::ffi::c_void;
type HDC = *mut std::ffi::c_void;
type HMODULE = *mut std::ffi::c_void;
type HCURSOR = *mut std::ffi::c_void;
type HBRUSH = *mut std::ffi::c_void;
type HICON = *mut std::ffi::c_void;
type LRESULT = isize;
type UINT = u32;
type WPARAM = usize;
type LPARAM = isize;
type WNDPROC = unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT;

#[repr(C)]
struct WNDCLASSEXW {
    cb_size: u32,
    style: u32,
    lpfn_wnd_proc: WNDPROC,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: HMODULE,
    h_icon: HICON,
    h_cursor: HCURSOR,
    h_br_background: HBRUSH,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
    h_icon_sm: HICON,
}

#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
    time: u32,
    pt: POINT,
}

#[repr(C)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct PAINTSTRUCT {
    hdc: HDC,
    f_erase: i32,
    rc_paint: RECT,
    f_restore: i32,
    f_inc_update: i32,
    rgb_reserved: [u8; 32],
}

#[repr(C)]
struct BITMAPINFOHEADER {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
struct BITMAPINFO {
    bmi_header: BITMAPINFOHEADER,
    bmi_colors: [u32; 1],
}

extern "system" {
    fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: *mut std::ffi::c_void,
        hInstance: HMODULE,
        lpParam: *mut std::ffi::c_void,
    ) -> HWND;
    fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> i32;
    fn UpdateWindow(hWnd: HWND) -> i32;
    fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn PostQuitMessage(nExitCode: i32);
    fn GetModuleHandleW(lpModuleName: *const u16) -> HMODULE;
    fn LoadCursorW(hInstance: HMODULE, lpCursorName: *const u16) -> HCURSOR;
    fn CreateIconFromResourceEx(
        pbIconBits: *const u8,
        cbIconBits: u32,
        fIcon: i32,
        dwVersion: u32,
        cxDesired: i32,
        cyDesired: i32,
        flags: u32,
    ) -> HICON;
    fn SendMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> i32;
    fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> i32;
    fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: i32) -> i32;
    fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: UINT, wMsgFilterMax: UINT) -> i32;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    fn GetKeyState(nVirtKey: i32) -> i16;
    fn SetDIBitsToDevice(
        hdc: HDC,
        xDest: i32,
        yDest: i32,
        w: u32,
        h: u32,
        xSrc: i32,
        ySrc: i32,
        uStartScan: u32,
        cScanLines: u32,
        lpvBits: *const std::ffi::c_void,
        lpbmi: *const BITMAPINFO,
        fuColorUse: u32,
    ) -> i32;
}

static mut GLOBAL_APP_PTR: *mut HollowCanvasDesktopApp = std::ptr::null_mut();

struct HollowCanvasDesktopApp {
    state: AppState,
    renderer: SoftwareRenderer,
    egui_ctx: egui::Context,
    start_time: Instant,
    events: Vec<egui::Event>,
    buffer: Vec<u32>,
    win_w: usize,
    win_h: usize,
    is_pointer_down: bool,
    is_space_down: bool,
    last_canvas_pos: Option<Vec2>,
    stroke_points: Vec<BrushPoint>,
    before_stroke_pixels: Vec<u8>,
    before_move_offset: (i32, i32),
    active_snapshot_taken: bool,
    mouse_pos: egui::Pos2,
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    const WM_DESTROY: UINT = 0x0002;
    const WM_SIZE: UINT = 0x0005;
    const WM_PAINT: UINT = 0x000F;
    const WM_MOUSEMOVE: UINT = 0x0200;
    const WM_LBUTTONDOWN: UINT = 0x0201;
    const WM_LBUTTONUP: UINT = 0x0202;
    const WM_RBUTTONDOWN: UINT = 0x0204;
    const WM_RBUTTONUP: UINT = 0x0205;
    const WM_MBUTTONDOWN: UINT = 0x0207;
    const WM_MBUTTONUP: UINT = 0x0208;
    const WM_MOUSEWHEEL: UINT = 0x020A;
    const WM_KEYDOWN: UINT = 0x0100;
    const WM_KEYUP: UINT = 0x0101;

    let app = if !GLOBAL_APP_PTR.is_null() {
        &mut *GLOBAL_APP_PTR
    } else {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    };

    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        WM_SIZE => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let width = (rc.right - rc.left).max(1) as usize;
            let height = (rc.bottom - rc.top).max(1) as usize;

            app.win_w = width;
            app.win_h = height;
            app.buffer.resize(width * height, 0);
            0
        }
        WM_MOUSEMOVE => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let width = (rc.right - rc.left).max(1) as f32;
            let height = (rc.bottom - rc.top).max(1) as f32;

            let raw_x = (lparam as u32 & 0xFFFF) as i16 as f32;
            let raw_y = ((lparam as u32 >> 16) & 0xFFFF) as i16 as f32;
            let x = raw_x.clamp(0.0, width);
            let y = raw_y.clamp(0.0, height);

            app.mouse_pos = egui::pos2(x, y);
            app.events.push(egui::Event::PointerMoved(app.mouse_pos));

            let screen_pos = Vec2::new(x, y);
            if app.is_space_down {
                if let Some(last) = app.last_canvas_pos {
                    app.state.pan += screen_pos - last;
                }
                app.last_canvas_pos = Some(screen_pos);
                InvalidateRect(hwnd, std::ptr::null(), 0);
                return 0;
            }

            let canvas_pos = app.state.screen_to_canvas(screen_pos, app.win_w as f32, app.win_h as f32);
            app.state.cursor_canvas_pos = canvas_pos;

            let is_over_ui = y < 42.0
                || y > (app.win_h as f32 - 28.0)
                || x < 205.0
                || x > (app.win_w as f32 - 235.0)
                || ((app.state.show_ref_window || app.state.show_help || app.state.show_gallery)
                    && app.egui_ctx.is_pointer_over_area());

            if app.is_pointer_down && !is_over_ui {
                let tool = app.state.brush.tool;
                if tool == ToolType::Move {
                    let is_ctrl = (GetKeyState(0x11) as i32 & 0x8000) != 0;
                    if is_ctrl {
                        if let Some(prev) = app.last_canvas_pos {
                            let delta = canvas_pos - prev;
                            if let Some(layer) = app.state.document.active_layer_mut() {
                                layer.offset_x += delta.x.round() as i32;
                                layer.offset_y += delta.y.round() as i32;
                            }
                        }
                        app.last_canvas_pos = Some(canvas_pos);
                    } else {
                        if let Some(prev) = app.last_canvas_pos {
                            let delta = screen_pos - prev;
                            app.state.pan += delta;
                        }
                        app.last_canvas_pos = Some(screen_pos);
                    }
                } else if tool == ToolType::Lasso {
                    let should_add = match app.state.lasso_points.last() {
                        Some(&last_pt) => last_pt.distance(canvas_pos) >= 2.0,
                        None => true,
                    };
                    if should_add {
                        app.state.lasso_points.push(canvas_pos);
                    }
                } else if tool.is_shape_tool() || tool == ToolType::Marquee || tool == ToolType::Crop || tool == ToolType::Polygon || tool == ToolType::Eyedropper || tool == ToolType::Fill {
                    // Explicitly NEVER paint continuous brush strokes when non-freehand tools are active
                } else if tool.is_freehand_stroke_tool() {
                    // Smooth Catmull-Rom spline curve interpolation with stabilization
                    let smoothing = app.state.brush.smoothing;
                    let smoothed_pos = if let Some(&last_pt) = app.stroke_points.last() {
                        let smooth_weight = (1.0 - smoothing * 0.85).clamp(0.05, 1.0);
                        last_pt.position.lerp(canvas_pos, smooth_weight)
                    } else {
                        canvas_pos
                    };

                    let pt = BrushPoint::new(smoothed_pos, 1.0);
                    app.stroke_points.push(pt);
                    let n = app.stroke_points.len();
                    let sel = app.state.selection.as_ref();

                    if n == 2 {
                        StrokeRasterizer::paint_segment(
                            &mut app.state.document,
                            app.stroke_points[0],
                            app.stroke_points[1],
                            &app.state.brush,
                            &app.state.symmetry,
                            sel,
                        );
                    } else if n == 3 {
                        StrokeRasterizer::paint_spline(
                            &mut app.state.document,
                            app.stroke_points[0],
                            app.stroke_points[0],
                            app.stroke_points[1],
                            app.stroke_points[2],
                            &app.state.brush,
                            &app.state.symmetry,
                            sel,
                        );
                    } else if n >= 4 {
                        StrokeRasterizer::paint_spline(
                            &mut app.state.document,
                            app.stroke_points[n - 4],
                            app.stroke_points[n - 3],
                            app.stroke_points[n - 2],
                            app.stroke_points[n - 1],
                            &app.state.brush,
                            &app.state.symmetry,
                            sel,
                        );
                    }
                    app.last_canvas_pos = Some(smoothed_pos);
                }
            }
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_LBUTTONDOWN => {
            app.is_pointer_down = true;
            app.events.push(egui::Event::PointerButton {
                pos: app.mouse_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            });

            let raw_x = (lparam as u32 & 0xFFFF) as i16 as f32;
            let raw_y = ((lparam as u32 >> 16) & 0xFFFF) as i16 as f32;
            let screen_pos = Vec2::new(raw_x, raw_y);

            if app.is_space_down {
                app.last_canvas_pos = Some(screen_pos);
                return 0;
            }

            let canvas_pos = app.state.screen_to_canvas(screen_pos, app.win_w as f32, app.win_h as f32);
            app.last_canvas_pos = Some(canvas_pos);
            app.stroke_points.clear();
            app.stroke_points.push(BrushPoint::new(canvas_pos, 1.0));

            let is_alt = (GetKeyState(0x12) as i32 & 0x8000) != 0; // VK_MENU (Alt)

            let is_over_ui = if app.state.show_ui_panels {
                raw_y < 38.0
                    || raw_y > (app.win_h as f32 - 28.0)
                    || raw_x < 218.0
                    || raw_x > (app.win_w as f32 - 240.0)
                    || ((app.state.show_ref_window || app.state.show_help || app.state.show_about_dialog || app.state.show_new_canvas_dialog || app.state.show_resize_canvas_dialog)
                        && app.egui_ctx.is_pointer_over_area())
            } else {
                (raw_x < 250.0 && raw_y < 50.0)
                    || ((app.state.show_ref_window || app.state.show_help || app.state.show_about_dialog || app.state.show_new_canvas_dialog || app.state.show_resize_canvas_dialog)
                        && app.egui_ctx.is_pointer_over_area())
            };

            if !is_over_ui {
                let tool = app.state.brush.tool;

                if is_alt {
                    let flat = app.state.document.composite_layers(false);
                    let cx = canvas_pos.x as i32;
                    let cy = canvas_pos.y as i32;
                    if cx >= 0 && cx < app.state.document.width as i32 && cy >= 0 && cy < app.state.document.height as i32 {
                        let idx = ((cy * app.state.document.width as i32 + cx) * 4) as usize;
                        if idx + 3 < flat.len() {
                            let picked = hollow_core::color::Color::from_rgba8(flat[idx], flat[idx + 1], flat[idx + 2], flat[idx + 3]);
                            app.state.brush.primary_color = picked;
                            app.state.push_color_history(picked);
                            app.state.set_status(format!("Sampled color: {}", picked.to_hex()));
                        }
                    }
                } else if tool == ToolType::Wand {
                    if canvas_pos.x >= 0.0 && canvas_pos.y >= 0.0 {
                        let mask = StrokeRasterizer::rasterize_magic_wand(
                            &app.state.document,
                            canvas_pos.x as u32,
                            canvas_pos.y as u32,
                            app.state.wand_tolerance,
                            app.state.wand_contiguous,
                            app.state.wand_sample_all_layers,
                        );
                        app.state.selection = Some(mask);
                        app.state.set_status("Magic Wand selection active");
                    }
                } else if tool == ToolType::Fill {
                    if let Some(layer) = app.state.document.active_layer() {
                        app.before_stroke_pixels = layer.pixels.clone();
                        app.active_snapshot_taken = true;
                    }
                    let sel = app.state.selection.as_ref();
                    if canvas_pos.x >= 0.0 && canvas_pos.y >= 0.0 {
                        StrokeRasterizer::flood_fill(
                            &mut app.state.document,
                            canvas_pos.x as u32,
                            canvas_pos.y as u32,
                            app.state.brush.primary_color,
                            sel,
                            app.state.wand_tolerance,
                        );
                    }
                } else if tool == ToolType::Gradient {
                    if let Some(layer) = app.state.document.active_layer() {
                        app.before_stroke_pixels = layer.pixels.clone();
                        app.active_snapshot_taken = true;
                    }
                    app.state.drag_start_canvas_pos = Some(canvas_pos);
                } else if tool == ToolType::Eyedropper {
                    let flat = app.state.document.composite_layers(false);
                    let cx = canvas_pos.x as i32;
                    let cy = canvas_pos.y as i32;
                    if cx >= 0 && cx < app.state.document.width as i32 && cy >= 0 && cy < app.state.document.height as i32 {
                        let idx = ((cy * app.state.document.width as i32 + cx) * 4) as usize;
                        if idx + 3 < flat.len() {
                            let picked = hollow_core::color::Color::from_rgba8(flat[idx], flat[idx + 1], flat[idx + 2], flat[idx + 3]);
                            app.state.brush.primary_color = picked;
                            app.state.push_color_history(picked);
                        }
                    }
                } else if tool == ToolType::Polygon {
                    let pts_len = app.state.polygon_points.len();
                    if pts_len >= 2 && app.state.polygon_points[0].distance(canvas_pos) < 15.0 {
                        // Close polygon
                        if let Some(layer) = app.state.document.active_layer() {
                            app.before_stroke_pixels = layer.pixels.clone();
                            app.active_snapshot_taken = true;
                        }
                        let pts = app.state.polygon_points.clone();
                        let sel = app.state.selection.as_ref();
                        StrokeRasterizer::rasterize_polygon(&mut app.state.document, &pts, &app.state.brush, &app.state.symmetry, sel);
                        app.state.polygon_points.clear();
                        app.state.set_status("Polygon committed");
                    } else {
                        app.state.polygon_points.push(canvas_pos);
                        app.state.set_status(format!("Polygon: {} points", app.state.polygon_points.len()));
                    }
                } else if tool == ToolType::Lasso {
                    app.state.lasso_points.clear();
                    app.state.lasso_points.push(canvas_pos);
                    app.state.set_status("Drawing Lasso selection loop...");
                } else if tool == ToolType::Move {
                    let is_ctrl = (GetKeyState(0x11) as i32 & 0x8000) != 0;
                    if is_ctrl {
                        if let Some(layer) = app.state.document.active_layer() {
                            app.before_move_offset = (layer.offset_x, layer.offset_y);
                        }
                        app.last_canvas_pos = Some(canvas_pos);
                    } else {
                        app.last_canvas_pos = Some(screen_pos);
                    }
                } else if tool.is_shape_tool() || tool == ToolType::Marquee || tool == ToolType::Crop {
                    if tool.is_shape_tool() {
                        if let Some(layer) = app.state.document.active_layer() {
                            app.before_stroke_pixels = layer.pixels.clone();
                            app.active_snapshot_taken = true;
                        }
                    }
                    app.state.drag_start_canvas_pos = Some(canvas_pos);
                } else if tool.is_freehand_stroke_tool() {
                    if let Some(layer) = app.state.document.active_layer() {
                        app.before_stroke_pixels = layer.pixels.clone();
                        app.active_snapshot_taken = true;
                    }
                    let point = BrushPoint::new(canvas_pos, 1.0);
                    let sel = app.state.selection.as_ref();
                    StrokeRasterizer::paint_dot(&mut app.state.document, point, &app.state.brush, &app.state.symmetry, sel);
                }
            }
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_LBUTTONUP => {
            app.is_pointer_down = false;
            app.events.push(egui::Event::PointerButton {
                pos: app.mouse_pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            });

            let canvas_pos = app.state.cursor_canvas_pos;
            let tool = app.state.brush.tool;

            if tool == ToolType::Lasso {
                let pts = std::mem::take(&mut app.state.lasso_points);
                let is_shift = (GetKeyState(0x10) as i32 & 0x8000) != 0;
                let is_alt = (GetKeyState(0x12) as i32 & 0x8000) != 0;

                if pts.len() >= 3 {
                    let new_mask = SelectionMask::from_polygon(app.state.document.width, app.state.document.height, &pts);
                    if is_shift {
                        if let Some(existing) = &mut app.state.selection {
                            existing.union(&new_mask);
                        } else if new_mask.has_selection() {
                            app.state.selection = Some(new_mask);
                        }
                        app.state.set_status("Added to selection (Lasso)");
                    } else if is_alt {
                        if let Some(existing) = &mut app.state.selection {
                            existing.subtract(&new_mask);
                            if !existing.has_selection() {
                                app.state.selection = None;
                            }
                        }
                        app.state.set_status("Subtracted from selection (Lasso)");
                    } else {
                        if new_mask.has_selection() {
                            app.state.selection = Some(new_mask);
                            app.state.set_status("Lasso selection created");
                        } else {
                            app.state.selection = None;
                            app.state.set_status("Deselected");
                        }
                    }
                } else if !is_shift && !is_alt {
                    // Single click or tap with Lasso clears selection
                    app.state.selection = None;
                    app.state.set_status("Deselected");
                }
            }

            if let Some(start) = app.state.drag_start_canvas_pos.take() {
                let sel = app.state.selection.as_ref();
                match tool {
                    ToolType::Line => {
                        StrokeRasterizer::rasterize_line(&mut app.state.document, start, canvas_pos, &app.state.brush, &app.state.symmetry, sel);
                    }
                    ToolType::Rect => {
                        StrokeRasterizer::rasterize_rect(&mut app.state.document, start, canvas_pos, &app.state.brush, &app.state.symmetry, sel);
                    }
                    ToolType::Ellipse => {
                        StrokeRasterizer::rasterize_ellipse(&mut app.state.document, start, canvas_pos, &app.state.brush, &app.state.symmetry, sel);
                    }
                    ToolType::Gradient => {
                        StrokeRasterizer::rasterize_gradient(&mut app.state.document, start, canvas_pos, &app.state.brush, sel);
                        app.state.set_status("Gradient applied");
                    }
                    ToolType::Marquee => {
                        let is_shift = (GetKeyState(0x10) as i32 & 0x8000) != 0;
                        let is_alt = (GetKeyState(0x12) as i32 & 0x8000) != 0;
                        if start.distance(canvas_pos) < 3.0 && !is_shift && !is_alt {
                            // Single click clears selection
                            app.state.selection = None;
                            app.state.set_status("Deselected");
                        } else {
                            let mask = SelectionMask::from_rect(app.state.document.width, app.state.document.height, start, canvas_pos);
                            if is_shift {
                                if let Some(existing) = &mut app.state.selection {
                                    existing.union(&mask);
                                } else if mask.has_selection() {
                                    app.state.selection = Some(mask);
                                }
                                app.state.set_status("Added to selection (Marquee)");
                            } else if is_alt {
                                if let Some(existing) = &mut app.state.selection {
                                    existing.subtract(&mask);
                                    if !existing.has_selection() {
                                        app.state.selection = None;
                                    }
                                }
                                app.state.set_status("Subtracted from selection (Marquee)");
                            } else {
                                if mask.has_selection() {
                                    app.state.selection = Some(mask);
                                    app.state.set_status("Selected area");
                                } else {
                                    app.state.selection = None;
                                    app.state.set_status("Deselected");
                                }
                            }
                        }
                    }
                    ToolType::Crop => {
                        app.state.crop_box = Some((start, canvas_pos));
                        app.state.set_status("Crop box defined. Click 'Apply Crop' in sidebar to confirm.");
                    }
                    _ => {}
                }
            }

            if tool == ToolType::Move {
                if let Some(layer) = app.state.document.active_layer() {
                    let current_offset = (layer.offset_x, layer.offset_y);
                    if current_offset != app.before_move_offset {
                        let cmd = Box::new(hollow_core::history::TranslateLayerCommand {
                            layer_id: layer.id,
                            before_offset: app.before_move_offset,
                            after_offset: current_offset,
                        });
                        app.state.history.push(cmd);
                    }
                }
            } else if tool.is_freehand_stroke_tool() {
                let n = app.stroke_points.len();
                let sel = app.state.selection.as_ref();
                if n >= 3 {
                    StrokeRasterizer::paint_spline(
                        &mut app.state.document,
                        app.stroke_points[n - 3],
                        app.stroke_points[n - 2],
                        app.stroke_points[n - 1],
                        app.stroke_points[n - 1],
                        &app.state.brush,
                        &app.state.symmetry,
                        sel,
                    );
                }
                app.stroke_points.clear();
            }

            if app.active_snapshot_taken {
                if let Some(layer) = app.state.document.active_layer() {
                    if app.before_stroke_pixels != layer.pixels {
                        let cmd = Box::new(LayerPixelsSnapshotCommand {
                            layer_id: layer.id,
                            description: app.state.brush.tool.label(),
                            before_pixels: app.before_stroke_pixels.clone(),
                            after_pixels: layer.pixels.clone(),
                        });
                        app.state.history.push(cmd);
                    }
                }
                app.active_snapshot_taken = false;
            }
            app.last_canvas_pos = None;
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_RBUTTONDOWN => {
            app.events.push(egui::Event::PointerButton {
                pos: app.mouse_pos,
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers: Default::default(),
            });
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_RBUTTONUP => {
            app.events.push(egui::Event::PointerButton {
                pos: app.mouse_pos,
                button: egui::PointerButton::Secondary,
                pressed: false,
                modifiers: Default::default(),
            });
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_MBUTTONDOWN => {
            app.is_space_down = true;
            let raw_x = (lparam as u32 & 0xFFFF) as i16 as f32;
            let raw_y = ((lparam as u32 >> 16) & 0xFFFF) as i16 as f32;
            app.last_canvas_pos = Some(Vec2::new(raw_x, raw_y));
            0
        }
        WM_MBUTTONUP => {
            app.is_space_down = false;
            app.last_canvas_pos = None;
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xFFFF) as i16 as f32 / 120.0;
            app.state.zoom = (app.state.zoom * (1.0 + delta * 0.1)).clamp(0.05, 8.0);
            app.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta * 30.0),
                modifiers: Default::default(),
            });
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_KEYDOWN => {
            let is_ctrl = (GetKeyState(0x11) as i32 & 0x8000) != 0; // VK_CONTROL
            let is_shift = (GetKeyState(0x10) as i32 & 0x8000) != 0; // VK_SHIFT

            match wparam {
                0x09 => { // Tab (Toggle Zen / Full Canvas mode)
                    app.state.show_ui_panels = !app.state.show_ui_panels;
                }
                0x20 => { // Space
                    if !app.is_space_down {
                        app.is_space_down = true;
                        app.last_canvas_pos = None;
                    }
                }
                0x4E if is_ctrl => { // Ctrl+N (New Canvas)
                    app.state.show_new_canvas_dialog = true;
                }
                0x5A if is_ctrl => { // Ctrl+Z
                    if is_shift {
                        if let Some(action) = app.state.history.redo(&mut app.state.document) {
                            app.state.set_status(format!("Redo: {}", action));
                        }
                    } else {
                        if let Some(action) = app.state.history.undo(&mut app.state.document) {
                            app.state.set_status(format!("Undo: {}", action));
                        }
                    }
                }
                0x59 if is_ctrl => { // Ctrl+Y
                    if let Some(action) = app.state.history.redo(&mut app.state.document) {
                        app.state.set_status(format!("Redo: {}", action));
                    }
                }
                0x53 if is_ctrl => { // Ctrl+S
                    app.state.pending_file_action = Some(PendingFileAction::SaveProject);
                }
                0x4F if is_ctrl => { // Ctrl+O
                    app.state.pending_file_action = Some(PendingFileAction::OpenProject);
                }
                0x45 if is_ctrl => { // Ctrl+E
                    app.state.pending_file_action = Some(PendingFileAction::ExportPng);
                }
                0x44 if is_ctrl => { // Ctrl+D (Deselect)
                    app.state.selection = None;
                    app.state.set_status("Deselected");
                }
                0x49 if is_ctrl => { // Ctrl+I (Invert Colors)
                    let w = app.state.document.width;
                    let h = app.state.document.height;
                    let sel = app.state.selection.clone();
                    if let Some(layer) = app.state.document.active_layer_mut() {
                        let before = layer.pixels.clone();
                        hollow_core::filter::filter_invert(&mut layer.pixels, w, h, sel.as_ref());
                        let cmd = Box::new(hollow_core::history::LayerPixelsSnapshotCommand {
                            layer_id: layer.id,
                            description: "Invert Colors",
                            before_pixels: before,
                            after_pixels: layer.pixels.clone(),
                        });
                        app.state.history.push(cmd);
                        app.state.set_status("Inverted Colors");
                    }
                }
                0x54 if is_ctrl => { // Ctrl+T (Free Transform)
                    if app.state.transform_session.is_active {
                        app.state.commit_transform_session();
                    } else {
                        app.state.begin_transform_session();
                    }
                }
                0x52 if is_ctrl => { // Ctrl+R (Toggle Rulers)
                    app.state.show_rulers = !app.state.show_rulers;
                }
                0x41 if is_ctrl => { // Ctrl+A (Select All)
                    app.state.selection = Some(SelectionMask::select_all(app.state.document.width, app.state.document.height));
                    app.state.set_status("Selected All");
                }
                0x74 if is_shift => { // Shift+F5 (Fill Selection)
                    app.state.fill_selection_active_layer();
                }
                0x08 if (GetKeyState(0x12) as i32 & 0x8000) != 0 => { // Alt+Backspace (Fill Selection)
                    app.state.fill_selection_active_layer();
                }
                0xDE if is_ctrl => { // Ctrl+' (Toggle Grid)
                    app.state.show_grid = !app.state.show_grid;
                }
                0x1B => { // Esc (Cancel Transform, selection or crop)
                    if app.state.transform_session.is_active {
                        app.state.cancel_transform_session();
                    } else if app.state.crop_box.is_some() {
                        app.state.crop_box = None;
                        app.state.set_status("Canceled Crop");
                    } else if app.state.polygon_points.len() > 0 {
                        app.state.polygon_points.clear();
                        app.state.set_status("Canceled Polygon");
                    } else if app.state.lasso_points.len() > 0 {
                        app.state.lasso_points.clear();
                        app.state.set_status("Canceled Lasso");
                    }
                    app.state.drag_start_canvas_pos = None;
                }
                0x58 => app.state.swap_colors(),                     // X (Swap colors)
                0x42 => app.state.brush.tool = ToolType::Brush,      // B
                0x50 => app.state.brush.tool = ToolType::Pencil,     // P
                0x57 => app.state.brush.tool = ToolType::Wand,       // W
                0x47 => app.state.brush.tool = ToolType::Gradient,   // G
                0x45 => app.state.brush.tool = ToolType::Eraser,     // E
                0x49 => app.state.brush.tool = ToolType::Eyedropper, // I
                0x4D => app.state.brush.tool = ToolType::Marquee,    // M
                0x4C => app.state.brush.tool = ToolType::Lasso,      // L
                0x54 => { // T (Toggle Canvas Tracing Reference)
                    app.state.tracing_enabled = !app.state.tracing_enabled;
                    app.state.set_status(if app.state.tracing_enabled { "Tracing Paper: ON" } else { "Tracing Paper: OFF" });
                }
                0x56 => app.state.brush.tool = ToolType::Move,       // V
                0x0D => { // Enter (Commit transform, polygon or crop)
                    if app.state.transform_session.is_active {
                        app.state.commit_transform_session();
                    } else if app.state.polygon_points.len() >= 3 {
                        let pts = app.state.polygon_points.clone();
                        let sel = app.state.selection.as_ref();
                        StrokeRasterizer::rasterize_polygon(&mut app.state.document, &pts, &app.state.brush, &app.state.symmetry, sel);
                        app.state.polygon_points.clear();
                        app.state.set_status("Polygon committed");
                    } else if let Some((min_p, max_p)) = app.state.crop_box {
                        let min_x = min_p.x.min(max_p.x).max(0.0) as u32;
                        let min_y = min_p.y.min(max_p.y).max(0.0) as u32;
                        let max_x = min_p.x.max(max_p.x).min(app.state.document.width as f32) as u32;
                        let max_y = min_p.y.max(max_p.y).min(app.state.document.height as f32) as u32;
                        let w = max_x.saturating_sub(min_x);
                        let h = max_y.saturating_sub(min_y);
                        if w > 10 && h > 10 {
                            app.state.document.resize_canvas(w, h, -(min_x as i32), -(min_y as i32));
                            app.state.crop_box = None;
                            app.state.brush.tool = ToolType::Brush;
                            app.state.set_status(format!("Cropped canvas to {}×{}", w, h));
                        }
                    }
                }
                _ => {}
            }
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_KEYUP => {
            if wparam == 0x20 {
                app.is_space_down = false;
                app.last_canvas_pos = None;
            }
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let width = (rc.right - rc.left).max(1) as usize;
            let height = (rc.bottom - rc.top).max(1) as usize;

            if app.win_w != width || app.win_h != height || app.buffer.len() != width * height {
                app.win_w = width;
                app.win_h = height;
                app.buffer.resize(width * height, 0);
            }

            if app.win_w > 0 && app.win_h > 0 && app.buffer.len() >= app.win_w * app.win_h {
                // 1. Process and Run egui UI layout FIRST so state mutations (grid, rulers, layers, tools) apply immediately
                let mut raw_input = egui::RawInput::default();
                raw_input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(app.win_w as f32, app.win_h as f32),
                ));
                raw_input.time = Some(app.start_time.elapsed().as_secs_f64());
                raw_input.events = std::mem::take(&mut app.events);

                let full_output = app.egui_ctx.run(raw_input, |ctx| {
                    render_ui(ctx, &mut app.state);
                });

                let needs_repaint = full_output.viewport_output.get(&egui::ViewportId::ROOT)
                    .map(|v| v.repaint_delay.is_zero())
                    .unwrap_or(false);

                // 2. Render canvas layer composite with up-to-date state and optional tracing paper reference
                let tracing_cfg = if app.state.tracing_enabled {
                    app.state.reference_image.as_ref().map(|(w, h, rgba)| {
                        hollow_render::TracingReferenceConfig {
                            width: *w,
                            height: *h,
                            rgba: rgba.as_slice(),
                            opacity: app.state.tracing_opacity,
                            offset: app.state.tracing_pos,
                            scale: app.state.tracing_scale,
                            is_underlay: app.state.tracing_as_underlay,
                        }
                    })
                } else {
                    None
                };

                app.renderer.render_canvas(
                    &mut app.buffer,
                    app.win_w,
                    app.win_h,
                    &app.state.document,
                    app.state.pan,
                    app.state.zoom,
                    tracing_cfg,
                );

                // 3. Render toggleable grid with up-to-date state
                if app.state.show_grid {
                    app.renderer.render_grid(
                        &mut app.buffer,
                        app.win_w,
                        app.win_h,
                        &app.state.document,
                        app.state.pan,
                        app.state.zoom,
                        app.state.grid_size,
                        app.state.grid_opacity,
                    );
                }

                // 4. Render live shape / tool drag preview overlays
                let accent_color = 0xFF5CE0D8; // Bright cyan overlay
                if let Some(start) = app.state.drag_start_canvas_pos {
                    let cur = app.state.cursor_canvas_pos;
                    let p0 = app.state.canvas_to_screen(start, app.win_w as f32, app.win_h as f32);
                    let p1 = app.state.canvas_to_screen(cur, app.win_w as f32, app.win_h as f32);

                    match app.state.brush.tool {
                        ToolType::Line | ToolType::Gradient => {
                            app.renderer.draw_screen_line(&mut app.buffer, app.win_w, app.win_h, p0, p1, accent_color, true);
                        }
                        ToolType::Rect | ToolType::Marquee | ToolType::Crop => {
                            app.renderer.draw_screen_rect(&mut app.buffer, app.win_w, app.win_h, p0, p1, accent_color, true);
                        }
                        ToolType::Ellipse => {
                            app.renderer.draw_screen_ellipse(&mut app.buffer, app.win_w, app.win_h, p0, p1, accent_color, true);
                        }
                        _ => {}
                    }
                }

                // Render crop box if active
                if let Some((cp0, cp1)) = app.state.crop_box {
                    let p0 = app.state.canvas_to_screen(cp0, app.win_w as f32, app.win_h as f32);
                    let p1 = app.state.canvas_to_screen(cp1, app.win_w as f32, app.win_h as f32);
                    app.renderer.draw_screen_rect(&mut app.buffer, app.win_w, app.win_h, p0, p1, 0xFFFFAA00, false);
                }

                // Render polygon vertices and line to cursor
                if !app.state.polygon_points.is_empty() {
                    for i in 0..app.state.polygon_points.len() - 1 {
                        let p0 = app.state.canvas_to_screen(app.state.polygon_points[i], app.win_w as f32, app.win_h as f32);
                        let p1 = app.state.canvas_to_screen(app.state.polygon_points[i + 1], app.win_w as f32, app.win_h as f32);
                        app.renderer.draw_screen_line(&mut app.buffer, app.win_w, app.win_h, p0, p1, accent_color, false);
                    }
                    if let Some(&last_pt) = app.state.polygon_points.last() {
                        let p0 = app.state.canvas_to_screen(last_pt, app.win_w as f32, app.win_h as f32);
                        let p1 = app.state.canvas_to_screen(app.state.cursor_canvas_pos, app.win_w as f32, app.win_h as f32);
                        app.renderer.draw_screen_line(&mut app.buffer, app.win_w, app.win_h, p0, p1, accent_color, true);
                    }
                }

                // 5. Render Dynamic Rulers if enabled
                if app.state.show_rulers {
                    app.renderer.render_rulers(
                        &mut app.buffer,
                        app.win_w,
                        app.win_h,
                        &app.state.document,
                        app.state.pan,
                        app.state.zoom,
                        app.state.cursor_canvas_pos,
                    );
                }

                // 6. Render egui UI primitives on top
                app.renderer.update_textures(&full_output.textures_delta);
                let clipped_primitives = app.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
                app.renderer.render_egui_primitives(&mut app.buffer, app.win_w, app.win_h, &clipped_primitives);

                // 7. Blit to Win32 window surface via GDI
                let bmi = BITMAPINFO {
                    bmi_header: BITMAPINFOHEADER {
                        bi_size: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        bi_width: app.win_w as i32,
                        bi_height: -(app.win_h as i32), // top-down DIB
                        bi_planes: 1,
                        bi_bit_count: 32,
                        bi_compression: 0, // BI_RGB
                        bi_size_image: 0,
                        bi_x_pels_per_meter: 0,
                        bi_y_pels_per_meter: 0,
                        bi_clr_used: 0,
                        bi_clr_important: 0,
                    },
                    bmi_colors: [0],
                };

                SetDIBitsToDevice(
                    hdc,
                    0,
                    0,
                    app.win_w as u32,
                    app.win_h as u32,
                    0,
                    0,
                    0,
                    app.win_h as u32,
                    app.buffer.as_ptr() as *const std::ffi::c_void,
                    &bmi,
                    0,
                );

                if needs_repaint {
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }

            EndPaint(hwnd, &ps);

            // 9. Handle pending file actions SAFELY OUTSIDE OF EGUI FRAME
            if let Some(action) = app.state.pending_file_action.take() {
                match action {
                    PendingFileAction::NewCanvas(w, h, bg_mode) => {
                        let mut doc = hollow_core::document::Document::new(w, h);
                        match bg_mode {
                            1 => { // Pure white
                                doc.is_transparent = false;
                                doc.background_value = 255;
                            }
                            2 => { // Transparent
                                doc.is_transparent = true;
                            }
                            _ => { // Dark studio
                                doc.is_transparent = false;
                                doc.background_value = 13;
                            }
                        }
                        app.state.document = doc;
                        app.state.history.clear();
                        app.state.selection = None;
                        app.state.reset_view_centered(app.win_w as f32, app.win_h as f32);
                        app.state.set_status(format!("New canvas created: {} × {} px", w, h));
                    }
                    PendingFileAction::SaveProject => {
                        if let Some(path) = hollow_ui::save_project_dialog() {
                            match save_project_file(&app.state.document, &path) {
                                Ok(_) => app.state.set_status(format!("Saved to {}", path.display())),
                                Err(e) => app.state.set_status(format!("Save error: {}", e)),
                            }
                        }
                    }
                    PendingFileAction::OpenProject => {
                        if let Some(path) = hollow_ui::open_project_dialog() {
                            match load_project_file(&path) {
                                Ok(doc) => {
                                    app.state.document = doc;
                                    app.state.history.clear();
                                    app.state.set_status(format!("Loaded {}", path.display()));
                                }
                                Err(e) => app.state.set_status(format!("Load error: {}", e)),
                            }
                        }
                    }
                    PendingFileAction::ExportPng => {
                        if let Some(path) = hollow_ui::export_png_dialog() {
                            match export_flat_image(&app.state.document, &path, ExportFormat::Png, !app.state.document.is_transparent) {
                                Ok(_) => app.state.set_status(format!("Exported PNG to {}", path.display())),
                                Err(e) => app.state.set_status(format!("Export error: {}", e)),
                            }
                        }
                    }
                    PendingFileAction::OpenReferenceImage => {
                        if let Some(path) = hollow_ui::open_image_dialog() {
                            if let Ok(img) = image::open(&path) {
                                let rgba = img.to_rgba8();
                                let (w, h) = rgba.dimensions();
                                app.state.reference_image = Some((w, h, rgba.into_raw()));
                                app.state.ref_texture = None;
                                app.state.show_ref_window = true;
                                app.state.set_status(format!("Loaded reference: {}", path.display()));
                            }
                        }
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }

            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // CLI Subcommands
    if args.len() > 1 {
        if args[1] == "export" {
            if args.len() < 4 {
                println!("Usage: hollow-app export <INPUT> -o <OUTPUT> [--format png|jpg|webp]");
                return Ok(());
            }
            let input = PathBuf::from(&args[2]);
            let mut output = PathBuf::from("output.png");
            let mut fmt_str = "png";

            let mut i = 3;
            while i < args.len() {
                if args[i] == "-o" && i + 1 < args.len() {
                    output = PathBuf::from(&args[i + 1]);
                    i += 2;
                } else if args[i] == "--format" && i + 1 < args.len() {
                    fmt_str = &args[i + 1];
                    i += 2;
                } else {
                    i += 1;
                }
            }

            println!("Loading project: {}", input.display());
            let doc = load_project_file(&input)?;
            let fmt = match fmt_str {
                "jpg" | "jpeg" => ExportFormat::Jpeg,
                "webp" => ExportFormat::WebP,
                _ => ExportFormat::Png,
            };
            export_flat_image(&doc, &output, fmt, true)?;
            println!("Successfully exported flat image to: {}", output.display());
            return Ok(());
        } else if args[1] == "inspect" {
            if args.len() < 3 {
                println!("Usage: hollow-app inspect <INPUT>");
                return Ok(());
            }
            let input = PathBuf::from(&args[2]);
            let doc = load_project_file(&input)?;
            println!("==========================================");
            println!(" Hollow Canvas Project: {}", input.display());
            println!("==========================================");
            println!("Canvas Size: {} x {}", doc.width, doc.height);
            println!("Active Layer ID: {}", doc.active_layer_id);
            println!("Background Value: {}", doc.background_value);
            println!("Is Transparent: {}", doc.is_transparent);
            println!("Theme: {:?}", doc.theme);
            println!("Total Layers: {}", doc.layers.len());
            for (idx, layer) in doc.layers.iter().enumerate() {
                println!(
                    "  [{}] ID: {} | Name: '{}' | Visible: {} | Opacity: {:.2} | Blend: {:?}",
                    idx, layer.id, layer.name, layer.visible, layer.opacity, layer.blend_mode
                );
            }
            println!("==========================================");
            return Ok(());
        } else if args[1] == "--help" || args[1] == "-h" {
            println!("Hollow Canvas · Native Windows Graphics Application");
            println!();
            println!("Usage: hollow-app [FILE]");
            println!("       hollow-app export <INPUT> -o <OUTPUT> [--format png|jpg|webp]");
            println!("       hollow-app inspect <INPUT>");
            return Ok(());
        }
    }

    println!("=== HOLLOW CANVAS · GRAPHICS STUDIO ===");
    let initial_width = 1380;
    let initial_height = 860;

    let app_state = if args.len() > 1 && !args[1].starts_with('-') {
        let initial_file = PathBuf::from(&args[1]);
        match load_project_file(&initial_file) {
            Ok(doc) => {
                let mut s = AppState::from_document(doc);
                s.set_status(format!("Loaded {}", initial_file.display()));
                s
            }
            Err(e) => {
                eprintln!("Warning: Failed to load specified project file: {}", e);
                AppState::default()
            }
        }
    } else {
        AppState::default()
    };

    let egui_ctx = egui::Context::default();
    configure_hollow_style(&egui_ctx, app_state.theme_accent_color());

    let mut desktop_app = Box::new(HollowCanvasDesktopApp {
        state: app_state,
        renderer: SoftwareRenderer::new(),
        egui_ctx,
        start_time: Instant::now(),
        events: Vec::new(),
        buffer: vec![0u32; initial_width * initial_height],
        win_w: initial_width,
        win_h: initial_height,
        is_pointer_down: false,
        is_space_down: false,
        last_canvas_pos: None,
        stroke_points: Vec::new(),
        before_stroke_pixels: Vec::new(),
        before_move_offset: (0, 0),
        active_snapshot_taken: false,
        mouse_pos: egui::Pos2::ZERO,
    });

    unsafe {
        GLOBAL_APP_PTR = &mut *desktop_app;

        let h_instance = GetModuleHandleW(std::ptr::null());
        let class_name = to_wide("HollowCanvasStudio");
        let title = to_wide("Hollow Canvas · Graphics Studio");

        let idc_arrow = 32512 as usize as *const u16;
        let h_cursor = LoadCursorW(std::ptr::null_mut(), idc_arrow);

        let wnd_class = WNDCLASSEXW {
            cb_size: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0x0003, // CS_HREDRAW | CS_VREDRAW
            lpfn_wnd_proc: window_proc,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance,
            h_icon: std::ptr::null_mut(),
            h_cursor,
            h_br_background: std::ptr::null_mut(),
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_name.as_ptr(),
            h_icon_sm: std::ptr::null_mut(),
        };

        RegisterClassExW(&wnd_class);

        const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            60,
            40,
            initial_width as i32,
            initial_height as i32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            h_instance,
            std::ptr::null_mut(),
        );

        if hwnd.is_null() {
            eprintln!("Failed to create Win32 window.");
            return Ok(());
        }

        // Load and set custom Application & Taskbar Icon
        const APP_ICON_BYTES: &[u8] = include_bytes!("../../../assets/icon.png");
        let h_icon_big = CreateIconFromResourceEx(
            APP_ICON_BYTES.as_ptr(),
            APP_ICON_BYTES.len() as u32,
            1, // TRUE: Icon
            0x00030000,
            64,
            64,
            0,
        );
        let h_icon_small = CreateIconFromResourceEx(
            APP_ICON_BYTES.as_ptr(),
            APP_ICON_BYTES.len() as u32,
            1, // TRUE: Icon
            0x00030000,
            32,
            32,
            0,
        );
        if !h_icon_big.is_null() {
            SendMessageW(hwnd, 0x0080 /* WM_SETICON */, 1 /* ICON_BIG */, h_icon_big as isize);
        }
        if !h_icon_small.is_null() {
            SendMessageW(hwnd, 0x0080 /* WM_SETICON */, 0 /* ICON_SMALL */, h_icon_small as isize);
        }

        ShowWindow(hwnd, 5); // SW_SHOW
        UpdateWindow(hwnd);
        println!("Hollow Canvas Window running successfully.");

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        GLOBAL_APP_PTR = std::ptr::null_mut();
    }

    Ok(())
}
