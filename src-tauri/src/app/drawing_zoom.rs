// Screen magnifier (ZoomIt-style). Included into `platform` (see native_drawing.rs).
//
// Live mode (default): the Windows full-screen magnifier (Magnification API,
// `MagSetFullscreenTransform`) scales the real, moving desktop - including our drawing overlay and
// toolbar - so every drawing tool keeps working while zoomed: input arrives in normal screen
// coordinates and DWM shows the result magnified. The offset follows the cursor so the point under
// the cursor stays fixed (offset = c * (1 - 1/z)); it is refreshed from the overlay message loop.
//
// Frozen mode (fallback when the Magnification API is unavailable, e.g. another magnifier owns it):
// the monitor under the cursor is captured once (without any Keyviz window), the drawings are
// blended on, and that snapshot is shown magnified on the overlay surface. Drawing is not possible.
//
// The wheel changes the level; the zoom shortcut / magnifier button / Esc leave it, and so does a
// right-click while the pointer tool is active (with a drawing tool right-click keeps its menu).

use crate::app::zoom_math::{next_zoom_level, render_zoom, ZOOM_START};

static ZOOM_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Whether the magnifier is showing; read from the keyboard hook so Esc leaves the zoom
/// instead of closing the whole drawing session.
pub fn zoom_active() -> bool {
    ZOOM_ACTIVE.load(Ordering::Relaxed)
}

struct ZoomSession {
    /// Live full-screen magnification (true) or a frozen snapshot on the overlay (false).
    live: bool,
    /// Monitor rectangle in virtual-screen coordinates.
    rect: RECT,
    /// Frozen mode only: opaque snapshot (screen + drawings), `rect`-sized.
    pixels: Vec<u32>,
    level: f64,
    /// Last cursor position in virtual-screen coordinates.
    cursor: WinPoint,
    /// Input mode to restore when the zoom ends (pointer mode lets clicks through).
    saved_click_through: bool,
    /// Started from the plain desktop: leaving the zoom also ends the drawing session.
    close_on_exit: bool,
    /// Live mode: last transform sent to the magnifier (level, x offset, y offset).
    applied: Option<(f64, i32, i32)>,
}

/// Viewing only (no drawing): frozen snapshots, or the pointer tool while zoomed.
fn zoom_view_only(state: &OverlayState) -> bool {
    state.zoom.as_ref().is_some_and(|zoom| !zoom.live || matches!(state.tool, NativeTool::Pointer))
}

/// Offset of the full-screen magnifier that keeps virtual-screen point `cursor` in place.
/// Offsets are measured from the primary monitor's origin, which is (0,0) in virtual coordinates.
fn live_zoom_offset(cursor: WinPoint, level: f64) -> (i32, i32) {
    let keep = 1.0 - 1.0 / level.max(1.0);
    ((f64::from(cursor.x) * keep).round() as i32, (f64::from(cursor.y) * keep).round() as i32)
}

/// Pushes the live transform for the current level / cursor; false when the API refused it.
unsafe fn apply_live_zoom(zoom: &mut ZoomSession) -> bool {
    use windows::Win32::UI::Magnification::MagSetFullscreenTransform;
    let (x, y) = live_zoom_offset(zoom.cursor, zoom.level);
    if zoom.applied == Some((zoom.level, x, y)) {
        return true;
    }
    if !MagSetFullscreenTransform(zoom.level as f32, x, y).as_bool() {
        return false;
    }
    zoom.applied = Some((zoom.level, x, y));
    true
}

/// Starts the full-screen magnifier; false when this Windows / session cannot do it.
unsafe fn start_live_magnifier(zoom: &mut ZoomSession) -> bool {
    use windows::Win32::UI::Magnification::{MagInitialize, MagUninitialize};
    if !MagInitialize().as_bool() {
        record_error(format!("MagInitialize failed: {}", std::io::Error::last_os_error()));
        return false;
    }
    if apply_live_zoom(zoom) {
        return true;
    }
    record_error(format!("MagSetFullscreenTransform failed: {}", std::io::Error::last_os_error()));
    MagUninitialize();
    false
}

unsafe fn stop_live_magnifier() {
    use windows::Win32::UI::Magnification::{MagSetFullscreenTransform, MagUninitialize};
    if !MagSetFullscreenTransform(1.0, 0, 0).as_bool() {
        record_error(format!("Resetting the magnifier failed: {}", std::io::Error::last_os_error()));
    }
    MagUninitialize();
}

/// Called from the overlay message loop while a live zoom runs: follow the cursor.
unsafe fn zoom_tick(state: &mut OverlayState) {
    let Some(zoom) = state.zoom.as_mut().filter(|zoom| zoom.live) else {
        return;
    };
    if let Some(cursor) = cursor_position() {
        zoom.cursor = cursor;
        apply_live_zoom(zoom);
    }
}

fn live_zoom_running(state: &OverlayState) -> bool {
    state.zoom.as_ref().is_some_and(|zoom| zoom.live)
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
    let cursor = cursor_position().unwrap_or(WinPoint {
        x: (rect.left + rect.right) / 2,
        y: (rect.top + rect.bottom) / 2,
    });
    let mut session = ZoomSession {
        live: true,
        rect,
        pixels: Vec::new(),
        level: ZOOM_START,
        cursor,
        saved_click_through: state.click_through,
        close_on_exit,
        applied: None,
    };
    // An opened JSON shows a frozen picture anyway; magnify that snapshot instead.
    let use_live = state.background.is_none();
    if use_live && start_live_magnifier(&mut session) {
        state.zoom = Some(session);
        ZOOM_ACTIVE.store(true, Ordering::Relaxed);
        finish_zoom_entry(state, "live");
        return Ok(());
    }
    if use_live {
        let _ = state.app.emit_to("drawing-toolbar", "drawing-zoom-error",
            "Live zoom is unavailable on this computer; showing a frozen snapshot instead");
    }
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
    session.live = false;
    session.pixels = pixels;
    state.zoom = Some(session);
    ZOOM_ACTIVE.store(true, Ordering::Relaxed);
    finish_zoom_entry(state, "frozen");
    Ok(())
}

unsafe fn finish_zoom_entry(state: &mut OverlayState, mode: &str) {
    // The magnifier needs the mouse (wheel = level, and in pointer mode move = pan) even when
    // the pointer tool normally lets clicks through.
    state.click_through = false;
    sync_input_surface_visibility(state);
    if zoom_view_only(state) {
        SetCursor(LoadCursorW(None, IDC_ARROW).unwrap_or_default());
    }
    refresh_overlay(state);
    raise_toolbar(&state.app);
    let _ = state.app.emit("drawing-zoom-changed", true);
    if let Some(zoom) = state.zoom.as_ref() {
        record_event(format!(
            "Zoom started ({mode}) on monitor ({},{},{},{})",
            zoom.rect.left, zoom.rect.top, zoom.rect.right, zoom.rect.bottom
        ));
    }
}

/// Leaves the magnifier. `allow_close`: a zoom started from the desktop also ends the session.
unsafe fn exit_zoom(state: &mut OverlayState, allow_close: bool) {
    let Some(zoom) = state.zoom.take() else {
        return;
    };
    ZOOM_ACTIVE.store(false, Ordering::Relaxed);
    if zoom.live {
        stop_live_magnifier();
    }
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

/// Drops the zoom without other side effects (session hidden).
unsafe fn discard_zoom(state: &mut OverlayState) {
    if let Some(zoom) = state.zoom.take() {
        ZOOM_ACTIVE.store(false, Ordering::Relaxed);
        if zoom.live {
            stop_live_magnifier();
        }
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
    let view_only = zoom_view_only(state);
    let Some(zoom) = state.zoom.as_mut() else {
        return false;
    };
    if zoom.live {
        // Follow the cursor right away (the loop tick also does); drawing tools keep the move.
        if let Some(cursor) = cursor_position() {
            zoom.cursor = cursor;
            apply_live_zoom(zoom);
        }
        return view_only;
    }
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
    if zoom.live {
        if let Some(cursor) = cursor_position() {
            zoom.cursor = cursor;
        }
        apply_live_zoom(zoom);
        return true;
    }
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
    let Some(zoom) = state.zoom.as_ref().filter(|zoom| !zoom.live && same_rect(&zoom.rect, bounds)) else {
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
