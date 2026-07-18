use std::{
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use rdev::{listen, Button, EventType};
use serde::Serialize;
use tauri::{menu::MenuItem, AppHandle, Emitter, Manager, Wry};

use crate::app::native_drawing::NativeTool;
use crate::app::state::AppState;

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL,
    VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};

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
    start_drawing_shortcut_poller(app_handle.clone());

    thread::spawn(move || {
        println!("Starting global input listener...");

        if let Err(err) = listen(move |event| {
            // get app state
            let state = app_handle.state::<Mutex<AppState>>();
            let mut app_state = state.lock().unwrap();

            // track pressed keys
            if let EventType::KeyPress(key) = event.event_type {
                let key_name = format!("{:?}", key);
                // If the name contains parenthesis (like "RawKey(123)", "Unknown()"), ignore it.
                if key_name.contains('(') {
                    return;
                }
                // if key is already marked as pressed, ignore repeat
                if app_state.pressed_keys.contains(&key_name) {
                    return;
                }
                // record key as pressed
                app_state.pressed_keys.push(key_name.clone());
                // Windows drawing shortcuts are handled by the raw-key poller below.
                // Processing them here as well can toggle the same session twice.
                if !cfg!(target_os = "windows")
                    && shortcut_pressed(&app_state.pressed_keys, &app_state.drawing_toggle_shortcut)
                {
                    let drawing_visible = app_state.drawing_visible;
                    app_state.pressed_keys.clear();
                    drop(app_state);
                    let result = if drawing_visible {
                        crate::close_screen_drawing_impl(app_handle.clone())
                    } else {
                        crate::show_drawing_window(&app_handle)
                    };
                    if let Err(error) = result {
                        eprintln!("Failed to toggle screen drawing shortcut: {error}");
                    }
                    return;
                }
                let drawing_visible = app_state.drawing_visible;
                if !cfg!(target_os = "windows")
                    && drawing_visible
                    && shortcut_pressed(
                        &app_state.pressed_keys,
                        &app_state.drawing_pointer_shortcut,
                    )
                {
                    drop(app_state);
                    if let Err(error) = set_drawing_pointer_mode(&app_handle) {
                        eprintln!("Failed to set drawing pointer shortcut: {error}");
                    }
                    return;
                }
                if !cfg!(target_os = "windows")
                    && drawing_visible
                    && shortcut_pressed(&app_state.pressed_keys, &app_state.drawing_clear_shortcut)
                {
                    app_state.drawing_overlay.delete_selection_or_clear();
                    return;
                }
                if !cfg!(target_os = "windows")
                    && drawing_visible
                    && shortcut_pressed(&app_state.pressed_keys, &app_state.drawing_undo_shortcut)
                {
                    app_state.drawing_overlay.undo();
                    return;
                }
                if !cfg!(target_os = "windows")
                    && drawing_visible
                    && shortcut_pressed(&app_state.pressed_keys, &app_state.drawing_close_shortcut)
                {
                    app_state.pressed_keys.clear();
                    drop(app_state);
                    if let Err(error) = crate::close_screen_drawing_impl(app_handle.clone()) {
                        eprintln!("Failed to close screen drawing shortcut: {error}");
                    }
                    return;
                }
                // check if toggle shortcut is pressed
                if shortcut_pressed(&app_state.pressed_keys, &app_state.toggle_shortcut) {
                    app_state.toggle_listener(&app_handle, &toggle_menu_item);

                    if !app_state.listening {
                        // emit key releases for all pressed keys
                        for key_name in &app_state.pressed_keys {
                            app_handle
                                .emit_to(
                                    "main",
                                    "input-event",
                                    InputEvent::KeyEvent {
                                        pressed: false,
                                        name: key_name.clone(),
                                    },
                                )
                                .unwrap()
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

            app_handle.emit("input-event", input_event).unwrap();
        }) {
            eprintln!("rdev listen failed: {:?}", err);
        }
    });
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

#[cfg(target_os = "windows")]
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
    app_state.drawing_overlay.set_click_through(true);
    drop(app_state);
    app_handle
        .emit(
            "drawing-tool-changed",
            serde_json::json!({ "tool": "pointer" }),
        )
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn start_drawing_shortcut_poller(app_handle: AppHandle) {
    thread::spawn(move || {
        let mut toggle_was_down = false;
        let mut pointer_was_down = false;
        let mut clear_was_down = false;
        let mut undo_was_down = false;
        let mut close_was_down = false;

        loop {
            thread::sleep(Duration::from_millis(40));

            let (toggle_shortcut, pointer_shortcut, clear_shortcut, undo_shortcut, close_shortcut) = {
                let state = app_handle.state::<Mutex<AppState>>();
                let shortcuts = match state.lock() {
                    Ok(app_state) => (
                        app_state.drawing_toggle_shortcut.clone(),
                        app_state.drawing_pointer_shortcut.clone(),
                        app_state.drawing_clear_shortcut.clone(),
                        app_state.drawing_undo_shortcut.clone(),
                        app_state.drawing_close_shortcut.clone(),
                    ),
                    Err(_) => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                };
                shortcuts
            };

            let toggle_down = shortcut_is_down(&toggle_shortcut);
            let pointer_down = shortcut_is_down(&pointer_shortcut);
            let clear_down = shortcut_is_down(&clear_shortcut);
            let undo_down = shortcut_is_down(&undo_shortcut);
            let close_down = shortcut_is_down(&close_shortcut);

            if toggle_down && !toggle_was_down {
                let drawing_visible = {
                    let state = app_handle.state::<Mutex<AppState>>();
                    state
                        .lock()
                        .map(|app_state| app_state.drawing_visible)
                        .unwrap_or(false)
                };
                let result = if drawing_visible {
                    crate::close_screen_drawing_impl(app_handle.clone())
                } else {
                    crate::show_drawing_window(&app_handle)
                };
                if let Err(error) = result {
                    eprintln!("Failed to toggle screen drawing poller shortcut: {error}");
                }
            }

            if clear_down && !clear_was_down {
                let state = app_handle.state::<Mutex<AppState>>();
                if let Ok(mut app_state) = state.lock() {
                    if app_state.drawing_visible {
                        app_state.drawing_overlay.delete_selection_or_clear();
                        app_state.pressed_keys.clear();
                    }
                };
            }

            if undo_down && !undo_was_down {
                let state = app_handle.state::<Mutex<AppState>>();
                if let Ok(mut app_state) = state.lock() {
                    if app_state.drawing_visible {
                        app_state.drawing_overlay.undo();
                        app_state.pressed_keys.clear();
                    }
                };
            }

            if close_down && !close_was_down {
                let drawing_visible = {
                    let state = app_handle.state::<Mutex<AppState>>();
                    state
                        .lock()
                        .map(|app_state| app_state.drawing_visible)
                        .unwrap_or(false)
                };
                if drawing_visible {
                    if let Err(error) = crate::close_screen_drawing_impl(app_handle.clone()) {
                        eprintln!("Failed to close screen drawing poller shortcut: {error}");
                    }
                }
            }

            if pointer_down && !pointer_was_down {
                if let Err(error) = set_drawing_pointer_mode(&app_handle) {
                    eprintln!("Failed to set drawing pointer poller shortcut: {error}");
                }
            }

            toggle_was_down = toggle_down;
            pointer_was_down = pointer_down;
            clear_was_down = clear_down;
            undo_was_down = undo_down;
            close_was_down = close_down;
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn start_drawing_shortcut_poller(_app_handle: AppHandle) {}

fn start_cursor_updater(app_handle: AppHandle) {
    let mut last_refresh = Instant::now() - Duration::from_secs(1);

    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(33));

        let update = {
            let state = app_handle.state::<Mutex<AppState>>();
            let Ok(mut app_state) = state.lock() else {
                continue;
            };
            let now = Instant::now();
            let refresh_due = app_state.cursor_keep_highlight
                && now.duration_since(last_refresh) >= Duration::from_millis(250);

            if !app_state.cursor_update_pending && !refresh_due {
                continue;
            }

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

        let _ = app_handle.emit(
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
