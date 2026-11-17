// Screen magnifier (ZoomIt-style). Included into `platform` (see native_drawing.rs).
//
// Entering zoom captures the monitor under the cursor (without any Keyviz window), blends the
// current drawings onto it and shows that frozen snapshot magnified on the monitor's overlay
// surface. The mouse pans (the point under the cursor stays fixed), the wheel changes the level,
// and the zoom shortcut / magnifier button / Esc / right-click leave it again.

use crate::app::zoom_math::{next_zoom_level, render_zoom, ZOOM_START};

static ZOOM_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Whether the magnifier is showing; read from the keyboard hook so Esc leaves the zoom
/// instead of closing the whole drawing session.
pub fn zoom_active() -> bool {
    ZOOM_ACTIVE.load(Ordering::Relaxed)
}

struct ZoomSession {
    /// Monitor rectangle in virtual-screen coordinates.
    rect: RECT,
    /// Opaque snapshot (screen + drawings), `rect`-sized.
    pixels: Vec<u32>,
    level: f64,
    /// Last cursor position in virtual-screen coordinates.
    cursor: WinPoint,
    /// Input mode to restore when the zoom ends (pointer mode lets clicks through).
    saved_click_through: bool,
    /// Started from the plain desktop: leaving the zoom also ends the drawing session.
    close_on_exit: bool,
}

unsafe fn cursor_position() -> Option<WinPoint> {
    let mut cursor = WinPoint::default();
    windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor)
        .as_bool()
        .then_some(cursor)
}

unsafe fn enter_zoom(state: &mut OverlayState, close_on_exit: bool) -> Result<(), String> {
    if !state.visible {
        return Err("Open the drawing mode before zooming".into());
    }
    if state.zoom.is_some() {
        return Ok(());
    }
    if state.radial.is_some() {
        cancel_radial(state);
    }
    commit_text_editor(state);
    finish_gesture_history(state);
    ReleaseCapture();
    state.active = None;
    state.selection = None;

    let index = target_surface(state).ok_or("No drawing surface is available")?;
    let rect = state.surfaces[index].bounds;
    let base: Vec<u32> = match &state.background {
        Some(background) if same_rect(&background.rect, &rect) => background.pixels.clone(),
        _ => capture_screen(state, rect, true)?,
    };
    let layer = render_drawings_layer(state, rect)?;
    let mut pixels = base;
    for (target, source) in pixels.iter_mut().zip(layer.pixels().iter()) {
        *target = blend_layer_pixel(*target, *source) | 0xff00_0000;
    }
    layer.free();

    let cursor = cursor_position().unwrap_or(WinPoint {
        x: (rect.left + rect.right) / 2,
        y: (rect.top + rect.bottom) / 2,
    });
    state.zoom = Some(ZoomSession {
        rect,
        pixels,
        level: ZOOM_START,
        cursor,
        saved_click_through: state.click_through,
        close_on_exit,
    });
    ZOOM_ACTIVE.store(true, Ordering::Relaxed);
    // The magnifier needs the mouse (move = pan, wheel = level) even in pointer mode.
    state.click_through = false;
    sync_input_surface_visibility(state);
    SetCursor(LoadCursorW(None, IDC_ARROW).unwrap_or_default());
    refresh_overlay(state);
    raise_toolbar(&state.app);
    let _ = state.app.emit("drawing-zoom-changed", true);
    record_event(format!(
        "Zoom started on monitor ({},{},{},{})",
        rect.left, rect.top, rect.right, rect.bottom
    ));
    Ok(())
}

/// Leaves the magnifier. `allow_close`: a zoom started from the desktop also ends the session.
unsafe fn exit_zoom(state: &mut OverlayState, allow_close: bool) {
    let Some(zoom) = state.zoom.take() else {
        return;
    };
    ZOOM_ACTIVE.store(false, Ordering::Relaxed);
    state.click_through = zoom.saved_click_through;
    sync_input_surface_visibility(state);
    if state.cursor.0 != 0 && !state.click_through {
        SetCursor(state.cursor);
    }
    refresh_overlay(state);
    let _ = state.app.emit("drawing-zoom-changed", false);
    record_event("Zoom ended");
    if allow_close && zoom.close_on_exit && state.visible {
        let app = state.app.clone();
        thread::spawn(move || {
            if let Err(error) = crate::close_screen_drawing_impl(app) {
                record_error(format!("Failed to close drawing after zoom: {error}"));
            }
        });
    }
}

/// Drops the zoom without side effects (session hidden / monitors rebuilt).
fn discard_zoom(state: &mut OverlayState) {
    if state.zoom.take().is_some() {
        ZOOM_ACTIVE.store(false, Ordering::Relaxed);
        let _ = state.app.emit("drawing-zoom-changed", false);
    }
}

unsafe fn apply_zoom_command(state: &mut OverlayState, enable: Option<bool>, close_on_exit: bool) {
    let want = enable.unwrap_or(state.zoom.is_none());
    if want {
        if let Err(error) = enter_zoom(state, close_on_exit) {
            record_error(format!("Failed to start zoom: {error}"));
            let _ = state.app.emit_to("drawing-toolbar", "drawing-zoom-error", error.clone());
            if close_on_exit && state.visible {
                // Nothing to show: do not leave an empty session open.
                let app = state.app.clone();
                thread::spawn(move || {
                    let _ = crate::close_screen_drawing_impl(app);
                });
            }
        }
    } else {
        exit_zoom(state, true);
    }
}

/// Redraws only the zoomed monitor (mouse moves / wheel are frequent).
unsafe fn refresh_zoom_surface(state: &mut OverlayState) {
    let Some(rect) = state.zoom.as_ref().map(|zoom| zoom.rect) else {
        return;
    };
    let Some(index) = state.surfaces.iter().position(|surface| same_rect(&surface.bounds, &rect)) else {
        return;
    };
    let mut surfaces = std::mem::take(&mut state.surfaces);
    refresh_overlay_surface(state, &mut surfaces[index], None);
    state.surfaces = surfaces;
}

/// Pans the zoom to the cursor. Returns true when the zoom consumed the move.
unsafe fn zoom_mouse_move(state: &mut OverlayState) -> bool {
    let Some(zoom) = state.zoom.as_mut() else {
        return false;
    };
    let Some(cursor) = cursor_position() else {
        return true;
    };
    let inside = cursor.x >= zoom.rect.left && cursor.x < zoom.rect.right
        && cursor.y >= zoom.rect.top && cursor.y < zoom.rect.bottom;
    if !inside || (cursor.x == zoom.cursor.x && cursor.y == zoom.cursor.y) {
        return true;
    }
    zoom.cursor = cursor;
    refresh_zoom_surface(state);
    true
}

/// Changes the zoom level by wheel notches. Returns true when the zoom consumed the wheel.
unsafe fn zoom_mouse_wheel(state: &mut OverlayState, delta: i16) -> bool {
    let Some(zoom) = state.zoom.as_mut() else {
        return false;
    };
    let steps = if delta > 0 { 1 } else { -1 };
    let level = next_zoom_level(zoom.level, steps);
    if (level - zoom.level).abs() < f64::EPSILON {
        return true;
    }
    zoom.level = level;
    if let Some(cursor) = cursor_position() {
        let inside = cursor.x >= zoom.rect.left && cursor.x < zoom.rect.right
            && cursor.y >= zoom.rect.top && cursor.y < zoom.rect.bottom;
        if inside {
            zoom.cursor = cursor;
        }
    }
    refresh_zoom_surface(state);
    true
}

/// Fills a surface's pixels with the magnified view when it is the zoomed monitor.
fn render_zoom_surface(state: &OverlayState, bounds: &RECT, pixels: &mut [u32], width: i32, height: i32) -> bool {
    let Some(zoom) = state.zoom.as_ref().filter(|zoom| same_rect(&zoom.rect, bounds)) else {
        return false;
    };
    render_zoom(
        &zoom.pixels,
        width.max(0) as usize,
        height.max(0) as usize,
        f64::from(zoom.cursor.x - zoom.rect.left),
        f64::from(zoom.cursor.y - zoom.rect.top),
        zoom.level,
        pixels,
    );
    true
}

/// Visible top-level windows of this process (key display, cursor highlight, Tauri windows...).
unsafe fn own_top_level_windows() -> Vec<HWND> {
    use windows::Win32::Foundation::{BOOL, LPARAM as EnumParam};
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId};

    unsafe extern "system" fn collect(hwnd: HWND, param: EnumParam) -> BOOL {
        let windows = &mut *(param.0 as *mut Vec<HWND>);
        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        if process_id == GetCurrentProcessId() && IsWindowVisible(hwnd).as_bool() {
            windows.push(hwnd);
        }
        BOOL(1)
    }

    let mut windows: Vec<HWND> = Vec::new();
    EnumWindows(Some(collect), EnumParam(&mut windows as *mut Vec<HWND> as isize));
    windows
}
