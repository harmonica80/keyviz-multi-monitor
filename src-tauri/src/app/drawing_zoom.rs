// Screen magnifier (ZoomIt-style). Included into `platform` (see native_drawing.rs).
//
// Live mode (default): the Windows full-screen magnifier (Magnification API,
// `MagSetFullscreenTransform`) scales the real, moving desktop - including our drawing overlay and
// toolbar. Input keeps its normal screen coordinates and Windows draws the pointer at its magnified
// position, so drawing tools work and, with the pointer tool, the computer can be used normally
// while zoomed. With level z and offset o, screen point q shows desktop point o + q/z.
//   * Edge panning (default): the view stays still and glides (eased) only when the magnified
//     pointer nears the monitor edge. "Follow" keeps the pointer's real point fixed instead.
//   * The wheel changes the level when a drawing tool owns the mouse; Ctrl+wheel always does
//     (swallowed by a small low-level mouse hook so the app underneath does not zoom too).
// The Windows full-screen magnifier always scales every monitor; we keep the view inside the
// monitor under the pointer.
//
// Frozen mode (fallback when the Magnification API is unavailable, e.g. another magnifier owns it,
// or while an opened JSON shows its own picture): the monitor under the cursor is captured once
// (without any Keyviz window), the drawings are blended on, and that snapshot is shown magnified on
// the overlay surface. It owns the mouse and cannot be drawn on.
//
// The zoom shortcut / magnifier button / Esc leave the zoom; in frozen mode a right-click does too.

use crate::app::zoom_math::{
    clamp_offset, ease_offset, edge_target, follow_offset, next_zoom_level, render_zoom,
    rezoom_offset, ZOOM_START,
};
use std::sync::atomic::{AtomicI32, AtomicU32};

static ZOOM_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Live zoom running: the Ctrl+wheel hook acts only then.
static LIVE_ZOOM: AtomicBool = AtomicBool::new(false);
/// Edge panning (true, default) or follow-the-pointer (false); set from the settings.
static ZOOM_EDGE_PAN: AtomicBool = AtomicBool::new(true);
/// Wheel delta collected by the Ctrl+wheel hook, consumed by the overlay loop.
static ZOOM_WHEEL_PENDING: AtomicI32 = AtomicI32::new(0);
static WHEEL_HOOK_THREAD: AtomicU32 = AtomicU32::new(0);

/// Whether the magnifier is showing; read from the keyboard hook so Esc leaves the zoom
/// instead of closing the whole drawing session.
pub fn zoom_active() -> bool {
    ZOOM_ACTIVE.load(Ordering::Relaxed)
}

/// Settings: true glides only near the edges, false keeps the pointer's point fixed.
pub fn set_zoom_edge_pan(edge: bool) {
    ZOOM_EDGE_PAN.store(edge, Ordering::Relaxed);
}

struct ZoomSession {
    /// Live full-screen magnification (true) or a frozen snapshot on the overlay (false).
    live: bool,
    /// Monitor being viewed, in virtual-screen coordinates (live: follows the pointer).
    rect: RECT,
    /// Frozen mode only: opaque snapshot (screen + drawings), `rect`-sized.
    pixels: Vec<u32>,
    level: f64,
    /// Last cursor position in virtual-screen coordinates.
    cursor: WinPoint,
    /// Frozen mode: input mode to restore (the snapshot owns the mouse while shown).
    saved_click_through: bool,
    /// Started from the plain desktop: leaving the zoom also ends the drawing session.
    close_on_exit: bool,
    /// Live mode: current and target magnifier offsets (x, y).
    offset: (f64, f64),
    target: (f64, f64),
    /// Live mode: last transform sent to the magnifier (level, x offset, y offset).
    applied: Option<(f64, i32, i32)>,
}

/// Frozen snapshots own the mouse and cannot be drawn on. A live zoom never blocks input.
fn zoom_view_only(state: &OverlayState) -> bool {
    state.zoom.as_ref().is_some_and(|zoom| !zoom.live)
}

fn live_zoom_running(state: &OverlayState) -> bool {
    state.zoom.as_ref().is_some_and(|zoom| zoom.live)
}

fn point_in(rect: &RECT, point: WinPoint) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

/// Offset that shows `zoom.cursor` at its own place, clamped to the monitor.
fn live_follow_offset(zoom: &ZoomSession) -> (f64, f64) {
    let (rect, level) = (zoom.rect, zoom.level);
    (
        clamp_offset(follow_offset(f64::from(zoom.cursor.x), level), f64::from(rect.left), f64::from(rect.right), level),
        clamp_offset(follow_offset(f64::from(zoom.cursor.y), level), f64::from(rect.top), f64::from(rect.bottom), level),
    )
}

/// Pushes the live transform for the current level / offset; false when the API refused it.
unsafe fn apply_live_zoom(zoom: &mut ZoomSession) -> bool {
    use windows::Win32::UI::Magnification::MagSetFullscreenTransform;
    let (x, y) = (zoom.offset.0.round() as i32, zoom.offset.1.round() as i32);
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
    zoom.offset = live_follow_offset(zoom);
    zoom.target = zoom.offset;
    if apply_live_zoom(zoom) {
        ZOOM_WHEEL_PENDING.store(0, Ordering::Relaxed);
        LIVE_ZOOM.store(true, Ordering::Relaxed);
        start_wheel_hook();
        return true;
    }
    record_error(format!("MagSetFullscreenTransform failed: {}", std::io::Error::last_os_error()));
    MagUninitialize();
    false
}

unsafe fn stop_live_magnifier() {
    use windows::Win32::UI::Magnification::{MagSetFullscreenTransform, MagUninitialize};
    // The wheel hook stays installed but is inert without a live zoom.
    LIVE_ZOOM.store(false, Ordering::Relaxed);
    if !MagSetFullscreenTransform(1.0, 0, 0).as_bool() {
        record_error(format!("Resetting the magnifier failed: {}", std::io::Error::last_os_error()));
    }
    MagUninitialize();
}

/// Changes a live zoom's level by wheel notches, keeping the magnified pointer in place.
unsafe fn live_zoom_level(zoom: &mut ZoomSession, steps: i32) {
    let level = next_zoom_level(zoom.level, steps);
    if (level - zoom.level).abs() < f64::EPSILON {
        return;
    }
    let (cx, cy) = (f64::from(zoom.cursor.x), f64::from(zoom.cursor.y));
    let rect = zoom.rect;
    let x = clamp_offset(rezoom_offset(cx, zoom.offset.0, zoom.level, level), f64::from(rect.left), f64::from(rect.right), level);
    let y = clamp_offset(rezoom_offset(cy, zoom.offset.1, zoom.level, level), f64::from(rect.top), f64::from(rect.bottom), level);
    zoom.level = level;
    zoom.offset = (x, y);
    zoom.target = (x, y);
    apply_live_zoom(zoom);
}

/// Called from the overlay message loop (~60 Hz) while a live zoom runs.
unsafe fn zoom_tick(state: &mut OverlayState) {
    let monitors: Vec<RECT> = state.surfaces.iter().map(|surface| surface.bounds).collect();
    let Some(zoom) = state.zoom.as_mut().filter(|zoom| zoom.live) else {
        return;
    };
    // Ctrl+wheel notches collected by the hook.
    let pending = ZOOM_WHEEL_PENDING.load(Ordering::Relaxed);
    let steps = pending / 120;
    if steps != 0 {
        ZOOM_WHEEL_PENDING.fetch_sub(steps * 120, Ordering::Relaxed);
        if let Some(cursor) = cursor_position() {
            zoom.cursor = cursor;
        }
        live_zoom_level(zoom, steps);
    }
    let Some(cursor) = cursor_position() else {
        return;
    };
    zoom.cursor = cursor;
    if !point_in(&zoom.rect, cursor) {
        // The pointer moved onto another monitor: view that one, starting around the pointer.
        if let Some(rect) = monitors.iter().find(|rect| point_in(rect, cursor)) {
            zoom.rect = *rect;
            zoom.offset = live_follow_offset(zoom);
            zoom.target = zoom.offset;
            apply_live_zoom(zoom);
        }
        return;
    }
    let rect = zoom.rect;
    if ZOOM_EDGE_PAN.load(Ordering::Relaxed) {
        let margin_x = (f64::from(rect.right - rect.left) * 0.12).max(40.0);
        let margin_y = (f64::from(rect.bottom - rect.top) * 0.12).max(40.0);
        zoom.target = (
            edge_target(f64::from(cursor.x), zoom.target.0, f64::from(rect.left), f64::from(rect.right), zoom.level, margin_x),
            edge_target(f64::from(cursor.y), zoom.target.1, f64::from(rect.top), f64::from(rect.bottom), zoom.level, margin_y),
        );
        zoom.offset = (
            ease_offset(zoom.offset.0, zoom.target.0, 0.22),
            ease_offset(zoom.offset.1, zoom.target.1, 0.22),
        );
    } else {
        zoom.offset = live_follow_offset(zoom);
        zoom.target = zoom.offset;
    }
    apply_live_zoom(zoom);
}

/// Low-level mouse hook (own thread) that turns Ctrl+wheel into zoom steps while a live zoom runs.
unsafe extern "system" fn zoom_wheel_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, HHOOK, MSLLHOOKSTRUCT, WM_MOUSEWHEEL as WHEEL};
    if code >= 0 && wparam.0 as u32 == WHEEL && LIVE_ZOOM.load(Ordering::Relaxed) && GetAsyncKeyState(0x11) < 0 {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let delta = ((info.mouseData >> 16) & 0xffff) as u16 as i16;
        ZOOM_WHEEL_PENDING.fetch_add(i32::from(delta), Ordering::Relaxed);
        return LRESULT(1);
    }
    CallNextHookEx(HHOOK(0), code, wparam, lparam)
}

fn start_wheel_hook() {
    // 1 = being installed, otherwise the hook thread id.
    if WHEEL_HOOK_THREAD.compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed).is_err() {
        return;
    }
    thread::spawn(|| unsafe {
        use windows::Win32::System::Threading::GetCurrentThreadId;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_MOUSE_LL,
        };
        let module = GetModuleHandleW(None).unwrap_or_default();
        let hook = match SetWindowsHookExW(WH_MOUSE_LL, Some(zoom_wheel_hook), module, 0) {
            Ok(hook) => hook,
            Err(error) => {
                record_error(format!("Zoom wheel hook failed: {error}"));
                WHEEL_HOOK_THREAD.store(0, Ordering::Relaxed);
                return;
            }
        };
        WHEEL_HOOK_THREAD.store(GetCurrentThreadId(), Ordering::Relaxed);
        // Installed once for the app's lifetime; it only acts while a live zoom runs.
        let mut message = MSG::default();
        while GetMessageW(&mut message, HWND(0), 0, 0).as_bool() {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        UnhookWindowsHookEx(hook);
        WHEEL_HOOK_THREAD.store(0, Ordering::Relaxed);
    });
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
        offset: (0.0, 0.0),
        target: (0.0, 0.0),
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
    if zoom_view_only(state) {
        // A frozen snapshot needs the mouse (move = pan, wheel = level) even in pointer mode.
        state.click_through = false;
        sync_input_surface_visibility(state);
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
    } else {
        state.click_through = zoom.saved_click_through;
        sync_input_surface_visibility(state);
    }
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
    let Some(zoom) = state.zoom.as_mut() else {
        return false;
    };
    if zoom.live {
        // Panning runs on the loop tick; drawing tools keep the move.
        return false;
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
    if zoom.live {
        if let Some(cursor) = cursor_position() {
            zoom.cursor = cursor;
        }
        live_zoom_level(zoom, steps);
        return true;
    }
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

/// The mouse highlight circle window (see native_cursor.rs).
unsafe fn is_cursor_highlight_window(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut buffer = [0u16; 64];
    let length = GetClassNameW(hwnd, &mut buffer).max(0) as usize;
    String::from_utf16_lossy(&buffer[..length]) == "KeyvizNativeCursorOverlay"
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
