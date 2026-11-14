use std::{
    sync::{mpsc, Mutex},
    thread,
    time::{Duration, Instant},
};

use rdev::{listen, Button, EventType};
use serde::Serialize;
use tauri::{menu::MenuItem, AppHandle, Emitter, Manager, Wry};

use crate::app::diagnostics::{record_error, record_event, unix_time_ms};
use crate::app::native_drawing::NativeTool;
use crate::app::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    KeyEvent {
        pressed: bool,
        name: String,
    },
    MouseButtonEvent {
        pressed: bool,
        button: MouseButton,
    },
    MouseMoveEvent {
        x: f64,
        y: f64,
        screen_x: f64,
        screen_y: f64,
    },
    MouseWheelEvent {
        delta_x: i64,
        delta_y: i64,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other,
}

pub fn map_mouse_button(button: Button) -> MouseButton {
    match button {
        Button::Left => MouseButton::Left,
        Button::Right => MouseButton::Right,
        Button::Middle => MouseButton::Middle,
        _ => MouseButton::Other,
    }
}

fn shortcut_key_matches(pressed_key: &str, shortcut_key: &str) -> bool {
    pressed_key == shortcut_key
        || matches!(
            (pressed_key, shortcut_key),
            ("ShiftLeft" | "ShiftRight", "Shift")
                | ("Shift", "ShiftLeft" | "ShiftRight")
                | ("ControlLeft" | "ControlRight", "Control")
                | ("Control", "ControlLeft" | "ControlRight")
                | ("Alt", "AltLeft" | "AltRight")
                | ("MetaLeft" | "MetaRight", "Meta")
                | ("Meta", "MetaLeft" | "MetaRight")
                | ("Num0", "Kp0")
                | ("Kp0", "Num0")
                | ("Num1", "Kp1")
                | ("Kp1", "Num1")
                | ("Num2", "Kp2")
                | ("Kp2", "Num2")
                | ("Num3", "Kp3")
                | ("Kp3", "Num3")
                | ("Num4", "Kp4")
                | ("Kp4", "Num4")
                | ("Num5", "Kp5")
                | ("Kp5", "Num5")
                | ("Num6", "Kp6")
                | ("Kp6", "Num6")
                | ("Num7", "Kp7")
                | ("Kp7", "Num7")
                | ("Num8", "Kp8")
                | ("Kp8", "Num8")
                | ("Num9", "Kp9")
                | ("Kp9", "Num9")
        )
}

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL,
    VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};

use std::sync::atomic::{AtomicU64, Ordering};

/// Both the hook path and the fallback poller below can detect the clear
/// shortcut (Delete by default). Whichever sees it first claims it; the other
/// is suppressed for a short window so the action can never run twice.
static LAST_CLEAR_SHORTCUT_MS: AtomicU64 = AtomicU64::new(0);

static LAST_POINTER_SHORTCUT_MS: AtomicU64 = AtomicU64::new(0);
static LAST_CLOSE_SHORTCUT_MS: AtomicU64 = AtomicU64::new(0);
static LAST_UNDO_SHORTCUT_MS: AtomicU64 = AtomicU64::new(0);

fn claim_shortcut(slot: &AtomicU64) -> bool {
    claim_shortcut_window(slot, 300)
}

/// Like `claim_shortcut` but with a caller-chosen dedup window. Undo/redo uses
/// a shorter window than the other drawing shortcuts: the window only has to
/// cover the gap between the hook path and the poller seeing the same
/// keystroke (tens of ms), and users legitimately press Ctrl+Z several times
/// per second to step back through history.
fn claim_shortcut_window(slot: &AtomicU64, window_ms: u64) -> bool {
    let now = unix_time_ms();
    let last = slot.load(Ordering::Relaxed);
    if now.saturating_sub(last) < window_ms {
        return false;
    }
    slot.store(now, Ordering::Relaxed);
    true
}

fn claim_clear_shortcut() -> bool {
    claim_shortcut(&LAST_CLEAR_SHORTCUT_MS)
}

#[cfg(target_os = "windows")]
fn key_is_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

#[cfg(target_os = "windows")]
fn raw_key_is_down(raw_key: &str) -> bool {
    match raw_key {
        "Shift" | "ShiftLeft" => key_is_down(VK_SHIFT.0 as i32) || key_is_down(VK_LSHIFT.0 as i32),
        "ShiftRight" => key_is_down(VK_RSHIFT.0 as i32),
        "Control" | "ControlLeft" => {
            key_is_down(VK_CONTROL.0 as i32) || key_is_down(VK_LCONTROL.0 as i32)
        }
        "ControlRight" => key_is_down(VK_RCONTROL.0 as i32),
        "Alt" | "AltLeft" => key_is_down(VK_MENU.0 as i32) || key_is_down(VK_LMENU.0 as i32),
        "AltRight" => key_is_down(VK_RMENU.0 as i32),
        "Meta" | "MetaLeft" => key_is_down(VK_LWIN.0 as i32),
        "MetaRight" => key_is_down(VK_RWIN.0 as i32),
        "Num0" | "Kp0" => key_is_down(0x30) || key_is_down(0x60),
        "Num1" | "Kp1" => key_is_down(0x31) || key_is_down(0x61),
        "Num2" | "Kp2" => key_is_down(0x32) || key_is_down(0x62),
        "Num3" | "Kp3" => key_is_down(0x33) || key_is_down(0x63),
        "Num4" | "Kp4" => key_is_down(0x34) || key_is_down(0x64),
        "Num5" | "Kp5" => key_is_down(0x35) || key_is_down(0x65),
        "Num6" | "Kp6" => key_is_down(0x36) || key_is_down(0x66),
        "Num7" | "Kp7" => key_is_down(0x37) || key_is_down(0x67),
        "Num8" | "Kp8" => key_is_down(0x38) || key_is_down(0x68),
        "Num9" | "Kp9" => key_is_down(0x39) || key_is_down(0x69),
        "KeyA" => key_is_down(0x41),
        "KeyB" => key_is_down(0x42),
        "KeyC" => key_is_down(0x43),
        "KeyD" => key_is_down(0x44),
        "KeyE" => key_is_down(0x45),
        "KeyF" => key_is_down(0x46),
        "KeyG" => key_is_down(0x47),
        "KeyH" => key_is_down(0x48),
        "KeyI" => key_is_down(0x49),
        "KeyJ" => key_is_down(0x4a),
        "KeyK" => key_is_down(0x4b),
        "KeyL" => key_is_down(0x4c),
        "KeyM" => key_is_down(0x4d),
        "KeyN" => key_is_down(0x4e),
        "KeyO" => key_is_down(0x4f),
        "KeyP" => key_is_down(0x50),
        "KeyQ" => key_is_down(0x51),
        "KeyR" => key_is_down(0x52),
        "KeyS" => key_is_down(0x53),
        "KeyT" => key_is_down(0x54),
        "KeyU" => key_is_down(0x55),
        "KeyV" => key_is_down(0x56),
        "KeyW" => key_is_down(0x57),
        "KeyX" => key_is_down(0x58),
        "KeyY" => key_is_down(0x59),
        "KeyZ" => key_is_down(0x5a),
        "F1" => key_is_down(0x70),
        "F2" => key_is_down(0x71),
        "F3" => key_is_down(0x72),
        "F4" => key_is_down(0x73),
        "F5" => key_is_down(0x74),
        "F6" => key_is_down(0x75),
        "F7" => key_is_down(0x76),
        "F8" => key_is_down(0x77),
        "F9" => key_is_down(0x78),
        "F10" => key_is_down(0x79),
        "F11" => key_is_down(0x7a),
        "F12" => key_is_down(0x7b),
        "Delete" => key_is_down(0x2e),
        "Escape" => key_is_down(0x1b),
        "Backspace" => key_is_down(0x08),
        "Tab" => key_is_down(0x09),
        "Return" | "KpReturn" => key_is_down(0x0d),
        "Space" => key_is_down(0x20),
        _ => false,
    }
}

#[cfg(target_os = "windows")]
fn shortcut_is_down(shortcut: &[String]) -> bool {
    !shortcut.is_empty() && shortcut.iter().all(|key| raw_key_is_down(key))
}


/// Fallback for the drawing shortcuts (return to pointer mode, clear, close)
/// that reads the real keyboard state instead of relying on low-level hook
/// events. Before v1.0.27 this was the only path that handled them on Windows;
/// the hook can miss events (elevated windows, hook timeouts, IME interference).
/// The hook path and this poller share a claim window per action, so an action
/// never runs twice. It sleeps while screen drawing is not visible.
#[cfg(target_os = "windows")]
fn start_clear_shortcut_poller(app_handle: AppHandle) {
    thread::spawn(move || {
        let (mut was_pointer, mut was_clear, mut was_close, mut was_undo) =
            (false, false, false, false);
        loop {
            let (visible, tool_active, pointer, clear, close, undo) = {
                let state = app_handle.state::<Mutex<AppState>>();
                let snapshot = match state.lock() {
                    Ok(app_state) => (
                        app_state.drawing_visible,
                        app_state.drawing_tool != "pointer",
                        app_state.drawing_pointer_shortcut.clone(),
                        app_state.drawing_clear_shortcut.clone(),
                        app_state.drawing_close_shortcut.clone(),
                        app_state.drawing_undo_shortcut.clone(),
                    ),
                    Err(_) => (false, false, Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                };
                snapshot
            };
            if !visible {
                (was_pointer, was_clear, was_close, was_undo) = (false, false, false, false);
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            let pointer_down = shortcut_is_down(&pointer);
            if pointer_down && !was_pointer && claim_shortcut(&LAST_POINTER_SHORTCUT_MS) {
                if let Err(error) = set_drawing_pointer_mode(&app_handle) {
                    record_error(format!("Failed to set drawing pointer shortcut: {error}"));
                }
            }
            was_pointer = pointer_down;

            let clear_down = tool_active && shortcut_is_down(&clear);
            if clear_down && !was_clear && claim_clear_shortcut() {
                let state = app_handle.state::<Mutex<AppState>>();
                let guard = state.lock();
                if let Ok(app_state) = guard {
                    if app_state.drawing_visible && app_state.drawing_tool != "pointer" {
                        app_state.drawing_overlay.delete_selection_or_clear();
                    }
                }
            }
            was_clear = clear_down;

            // Undo / redo (Ctrl+Z / Ctrl+Shift+Z by default). Same fallback as
            // the shortcuts above: the hook can miss keystrokes, so read the
            // real keyboard state. Edge-triggered, deduped with the hook path
            // via LAST_UNDO_SHORTCUT_MS.
            let undo_down = tool_active && shortcut_is_down(&undo);
            if undo_down && !was_undo && claim_shortcut_window(&LAST_UNDO_SHORTCUT_MS, 150) {
                let undo_has_shift = undo.iter().any(|key| key.starts_with("Shift"));
                let redo = !undo_has_shift && raw_key_is_down("Shift");
                let state = app_handle.state::<Mutex<AppState>>();
                let guard = state.lock();
                if let Ok(app_state) = guard {
                    if app_state.drawing_visible && app_state.drawing_tool != "pointer" {
                        app_state.drawing_overlay.history_shortcut(redo);
                    }
                }
            }
            was_undo = undo_down;

            let close_down = shortcut_is_down(&close);
            if close_down
                && !was_close
                && (tool_active || foreground_is_own_window())
                && !crate::app::native_drawing::radial_consumes_close()
                && claim_shortcut(&LAST_CLOSE_SHORTCUT_MS)
            {
                if let Err(error) = crate::close_screen_drawing_impl(app_handle.clone()) {
                    record_error(format!("Failed to close screen drawing shortcut: {error}"));
                }
            }
            was_close = close_down;
            thread::sleep(Duration::from_millis(if tool_active { 40 } else { 80 }));
        }
    });
}

/// Whether the key is physically held down right now (Windows only; other
/// platforms report `true` so nothing is ever pruned there).
#[cfg(windows)]
fn key_physically_down(vk: u32) -> bool {
    unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk as i32) < 0 }
}

#[cfg(not(windows))]
fn key_physically_down(_vk: u32) -> bool {
    true
}

/// Low-level hooks can miss a key-up event (IME switching hotkeys such as
/// Ctrl+Space / Ctrl+Shift / Win+Space, elevated or secure windows taking the
/// key-up, ...). A modifier left "pressed" in our bookkeeping makes the
/// frontend's modifier filter let every later key through. Before handling a
/// new key press, drop any tracked key that the OS says is no longer down and
/// return the dropped names so the frontend can be told they were released.
fn prune_stale_keys(app_state: &mut AppState, current: &str) -> Vec<String> {
    let grace = Duration::from_millis(300);
    let stale: Vec<String> = app_state
        .pressed_keys
        .iter()
        .filter(|name| name.as_str() != current)
        .filter(|name| {
            app_state
                .pressed_key_meta
                .get(*name)
                .map(|(vk, since)| since.elapsed() > grace && !key_physically_down(*vk))
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    for name in &stale {
        app_state.pressed_keys.retain(|k| k != name);
        app_state.pressed_key_meta.remove(name);
    }
    stale
}

fn shortcut_pressed(pressed_keys: &[String], shortcut: &[String]) -> bool {
    !shortcut.is_empty()
        && shortcut.iter().all(|shortcut_key| {
            pressed_keys
                .iter()
                .any(|pressed| shortcut_key_matches(pressed, shortcut_key))
        })
}

pub fn start_listener(app_handle: AppHandle, toggle_menu_item: MenuItem<Wry>) {
    start_cursor_updater(app_handle.clone());
    #[cfg(target_os = "windows")]
    start_clear_shortcut_poller(app_handle.clone());

    // The low-level hook callback must stay fast and must never block:
    // Windows silently removes a hook whose callback exceeds the
    // LowLevelHooksTimeout budget, which permanently kills key display until
    // the app restarts. The callback therefore only forwards the event into a
    // channel; all state updates, shortcut handling and UI work happen on the
    // worker thread below.
    let (event_sender, event_receiver) = mpsc::channel::<rdev::Event>();

    let worker_app_handle = app_handle.clone();
    thread::spawn(move || {
        for event in event_receiver {
            process_input_event(&worker_app_handle, &toggle_menu_item, event);
        }
    });

    thread::spawn(move || {
        println!("Starting global input listener...");

        let status_app_handle = app_handle.clone();
        if let Ok(mut app_state) = status_app_handle.state::<Mutex<AppState>>().lock() {
            app_state.mouse_hook_status = "running".to_string();
            app_state.mouse_hook_last_error = None;
        }

        if let Err(err) = listen(move |event| {
            let _ = event_sender.send(event);
        }) {
            let message = format!("rdev listen failed: {err:?}");
            record_error(message.clone());
            if let Ok(mut app_state) = status_app_handle.state::<Mutex<AppState>>().lock() {
                app_state.mouse_hook_status = "failed".to_string();
                app_state.mouse_hook_last_error = Some(message);
            }
        }
    });
}

/// True when the window the user is currently working in belongs to this
/// process (drawing canvas, toolbar, settings). Used to scope the drawing
/// close shortcut (Esc by default): while annotations are merely displayed in
/// pointer mode, Esc belongs to whatever application the user is actually
/// using (ending a slideshow, closing a dialog) and must not tear down the
/// drawing session behind their back.
#[cfg(target_os = "windows")]
fn foreground_is_own_window() -> bool {
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return false;
        }
        let mut process_id = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        process_id != 0 && process_id == GetCurrentProcessId()
    }
}

#[cfg(not(target_os = "windows"))]
fn foreground_is_own_window() -> bool {
    true
}

fn process_input_event(
    app_handle: &AppHandle,
    toggle_menu_item: &MenuItem<Wry>,
    event: rdev::Event,
) {
    {
            // get app state
            let state = app_handle.state::<Mutex<AppState>>();
            let Ok(mut app_state) = state.lock() else {
                record_error("Input state is unavailable");
                return;
            };
            app_state.mouse_hook_event_count = app_state.mouse_hook_event_count.wrapping_add(1);
            app_state.mouse_hook_last_event_unix_ms = Some(unix_time_ms());

            // track pressed keys
            if let EventType::KeyPress(key) = event.event_type {
                let key_name = format!("{:?}", key);
                // If the name contains parenthesis (like "RawKey(123)", "Unknown()"), ignore it.
                if key_name.contains('(') {
                    return;
                }
                // forget keys whose release was never delivered (e.g. around IME switching)
                let stale_keys = prune_stale_keys(&mut app_state, &key_name);
                if !stale_keys.is_empty() {
                    record_event(format!("Released stale keys: {}", stale_keys.join(", ")));
                    if app_state.listening {
                        for name in &stale_keys {
                            if let Err(error) = app_handle.emit_to(
                                "main",
                                "input-event",
                                InputEvent::KeyEvent { pressed: false, name: name.clone() },
                            ) {
                                record_error(format!("Failed to emit stale key release: {error}"));
                            }
                        }
                    }
                }
                // if key is already marked as pressed, ignore repeat
                if app_state.pressed_keys.contains(&key_name) {
                    return;
                }
                // record key as pressed
                app_state.pressed_keys.push(key_name.clone());
                #[cfg(windows)]
                if let Some(vk) = rdev::win_code_from_key(key) {
                    app_state
                        .pressed_key_meta
                        .insert(key_name.clone(), (vk, Instant::now()));
                }
                // Drawing shortcuts are handled here, in the single hook event
                // stream, on every platform. (A separate GetAsyncKeyState
                // poller used to handle them on Windows; two concurrent
                // detection paths could race and toggle a session twice.)
                if shortcut_pressed(&app_state.pressed_keys, &app_state.drawing_toggle_shortcut)
                {
                    let drawing_visible = app_state.drawing_visible;
                    app_state.pressed_keys.clear();
                    drop(app_state);
                    let result = if drawing_visible {
                        crate::close_screen_drawing_impl(app_handle.clone())
                    } else {
                        crate::show_drawing_window(app_handle)
                    };
                    if let Err(error) = result {
                        record_error(format!("Failed to toggle screen drawing shortcut: {error}"));
                    }
                    return;
                }
                let drawing_visible = app_state.drawing_visible;
                let drawing_tool_active = drawing_visible && app_state.drawing_tool != "pointer";
                if drawing_visible
                    && shortcut_pressed(
                        &app_state.pressed_keys,
                        &app_state.drawing_pointer_shortcut,
                    )
                {
                    drop(app_state);
                    if claim_shortcut(&LAST_POINTER_SHORTCUT_MS) {
                        if let Err(error) = set_drawing_pointer_mode(app_handle) {
                            record_error(format!("Failed to set drawing pointer shortcut: {error}"));
                        }
                    }
                    return;
                }
                if drawing_tool_active
                    && shortcut_pressed(&app_state.pressed_keys, &app_state.drawing_clear_shortcut)
                {
                    if claim_clear_shortcut() {
                        app_state.drawing_overlay.delete_selection_or_clear();
                    }
                    return;
                }
                if drawing_tool_active {
                    // Redo is the undo shortcut plus Shift (Ctrl+Shift+Z for the
                    // default Ctrl+Z). Shortcut matching ignores extra pressed
                    // keys, so check redo before undo.
                    let undo_has_shift = app_state
                        .drawing_undo_shortcut
                        .iter()
                        .any(|key| key.starts_with("Shift"));
                    let shift_down = app_state
                        .pressed_keys
                        .iter()
                        .any(|key| key.starts_with("Shift"));
                    if shortcut_pressed(&app_state.pressed_keys, &app_state.drawing_undo_shortcut) {
                        let redo = !undo_has_shift && shift_down;
                        if claim_shortcut_window(&LAST_UNDO_SHORTCUT_MS, 150) {
                            app_state.drawing_overlay.history_shortcut(redo);
                        }
                        return;
                    }
                }
                // Scope for the close shortcut (Esc by default): while a
                // drawing tool is active our input layer owns the keyboard, so
                // Esc clearly targets the drawing session. In pointer mode the
                // user is working in another application; only honor Esc when
                // one of our own windows is in the foreground, and never while
                // the radial menu is using it.
                if drawing_visible
                    && shortcut_pressed(&app_state.pressed_keys, &app_state.drawing_close_shortcut)
                    && (drawing_tool_active || foreground_is_own_window())
                    && !crate::app::native_drawing::radial_consumes_close()
                {
                    app_state.pressed_keys.clear();
                    drop(app_state);
                    if claim_shortcut(&LAST_CLOSE_SHORTCUT_MS) {
                        if let Err(error) = crate::close_screen_drawing_impl(app_handle.clone()) {
                            record_error(format!("Failed to close screen drawing shortcut: {error}"));
                        }
                    }
                    return;
                }
                // check if toggle shortcut is pressed
                if shortcut_pressed(&app_state.pressed_keys, &app_state.toggle_shortcut) {
                    app_state.toggle_listener(app_handle, toggle_menu_item);

                    if !app_state.listening {
                        // emit key releases for all pressed keys
                        for key_name in &app_state.pressed_keys {
                            if let Err(error) = app_handle.emit_to(
                                "main",
                                "input-event",
                                InputEvent::KeyEvent {
                                    pressed: false,
                                    name: key_name.clone(),
                                },
                            ) {
                                record_error(format!("Failed to emit key release: {error}"));
                            }
                        }
                    }
                }
            } else if let EventType::KeyRelease(key) = event.event_type {
                let key_name = format!("{:?}", key);
                if key_name.contains('(') {
                    return;
                }
                // remove key from pressed keys
                app_state.pressed_keys.retain(|k| k != &key_name);
                app_state.pressed_key_meta.remove(&key_name);
            }

            if let EventType::MouseMove { x, y } = event.event_type {
                app_state.cursor_x = x;
                app_state.cursor_y = y;
                app_state.cursor_update_pending = true;
                let should_send_drawing_move =
                    if app_state.drawing_input_passthrough && app_state.drawing_pointer_down {
                        let now = Instant::now();
                        let due = app_state
                            .drawing_last_move
                            .map(|last| now.duration_since(last) >= Duration::from_millis(16))
                            .unwrap_or(true);
                        if due {
                            app_state.drawing_last_move = Some(now);
                        }
                        due
                    } else {
                        false
                    };
                if should_send_drawing_move {
                    app_state
                        .drawing_overlay
                        .pointer_move(x.round() as i32, y.round() as i32);
                }
            }

            match event.event_type {
                EventType::ButtonPress(Button::Left) => {
                    if app_state.drawing_input_passthrough {
                        let x = app_state.cursor_x.round() as i32;
                        let y = app_state.cursor_y.round() as i32;
                        app_state.drawing_pointer_down = true;
                        app_state.drawing_last_move = Some(Instant::now());
                        app_state.drawing_overlay.pointer_down(x, y);
                    }
                }
                EventType::ButtonRelease(Button::Left) => {
                    if app_state.drawing_input_passthrough {
                        let x = app_state.cursor_x.round() as i32;
                        let y = app_state.cursor_y.round() as i32;
                        app_state.drawing_overlay.pointer_up(x, y);
                    }
                    app_state.drawing_pointer_down = false;
                    app_state.drawing_last_move = None;
                }
                _ => {}
            }

            // emit event if listening
            if !app_state.listening {
                return;
            }

            let input_event = match event.event_type {
                EventType::KeyPress(key) => Some(InputEvent::KeyEvent {
                    pressed: true,
                    name: format!("{:?}", key),
                }),
                EventType::KeyRelease(key) => Some(InputEvent::KeyEvent {
                    pressed: false,
                    name: format!("{:?}", key),
                }),
                EventType::ButtonPress(button) => Some(InputEvent::MouseButtonEvent {
                    pressed: true,
                    button: map_mouse_button(button),
                }),
                EventType::ButtonRelease(button) => Some(InputEvent::MouseButtonEvent {
                    button: map_mouse_button(button),
                    pressed: false,
                }),
                EventType::MouseMove { .. } => return,
                EventType::Wheel { delta_x, delta_y } => {
                    Some(InputEvent::MouseWheelEvent { delta_x, delta_y })
                }
            };

            // Only the visualization window ("main") listens to input events;
            // a broadcast would wake every webview for every keystroke.
            if let Err(error) = app_handle.emit_to("main", "input-event", input_event) {
                record_error(format!("Failed to emit input event: {error}"));
            }
    }
}

fn set_drawing_pointer_mode(app_handle: &AppHandle) -> Result<(), String> {
    let state = app_handle.state::<Mutex<AppState>>();
    let mut app_state = state.lock().map_err(|error| error.to_string())?;
    if !app_state.drawing_visible {
        return Ok(());
    }
    app_state.pressed_keys.clear();
    app_state.drawing_input_passthrough = true;
    app_state.drawing_pointer_down = false;
    app_state.drawing_last_move = None;
    app_state.drawing_overlay.set_tool(NativeTool::Pointer);
    app_state.drawing_tool = "pointer".to_string();
    app_state.drawing_overlay.set_click_through(true);
    drop(app_state);
    app_handle
        .emit(
            "drawing-tool-changed",
            serde_json::json!({ "tool": "pointer" }),
        )
        .map_err(|error| error.to_string())
}

fn start_cursor_updater(app_handle: AppHandle) {
    let mut last_refresh = Instant::now() - Duration::from_secs(1);
    // ~30 fps while the cursor highlight is in use; a slow heartbeat when the
    // feature is off and the mouse is idle, so an idle app stays quiet.
    let mut idle = false;

    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(if idle { 150 } else { 33 }));

        let update = {
            let state = app_handle.state::<Mutex<AppState>>();
            let Ok(mut app_state) = state.lock() else {
                continue;
            };
            let now = Instant::now();
            let refresh_due = app_state.cursor_keep_highlight
                && now.duration_since(last_refresh) >= Duration::from_millis(250);

            if !app_state.cursor_update_pending && !refresh_due {
                idle = !app_state.cursor_keep_highlight;
                continue;
            }
            idle = false;

            app_state.cursor_update_pending = false;
            let visible = app_state.cursor_keep_highlight;
            let visibility_changed = visible != app_state.cursor_window_visible;
            app_state.cursor_window_visible = visible;
            let (offset_x, offset_y) = app_state.monitor_position;
            Some((
                app_state.cursor_overlay.clone(),
                app_state.cursor_x,
                app_state.cursor_y,
                app_state.cursor_size,
                app_state.cursor_color.clone(),
                app_state.cursor_opacity,
                app_state.cursor_thickness,
                visible,
                visibility_changed,
                offset_x,
                offset_y,
            ))
        };

        let Some((
            cursor_overlay,
            x,
            y,
            size,
            color,
            opacity,
            thickness,
            visible,
            visibility_changed,
            offset_x,
            offset_y,
        )) = update
        else {
            continue;
        };

        if visible || visibility_changed {
            cursor_overlay.update(x, y, size, &color, opacity, thickness, visible);
            last_refresh = Instant::now();
        }

        let _ = app_handle.emit_to(
            "main",
            "input-event",
            InputEvent::MouseMoveEvent {
                x: x - offset_x as f64,
                y: y - offset_y as f64,
                screen_x: x,
                screen_y: y,
            },
        );
    });
}
