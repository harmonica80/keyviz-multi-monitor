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
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_RCONTROL,
};

#[cfg(target_os = "windows")]
const VK_0: i32 = 0x30;
#[cfg(target_os = "windows")]
const VK_9: i32 = 0x39;

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

fn is_screen_drawing_shortcut_pressed(pressed_keys: &[String]) -> bool {
    let has_control = pressed_keys
        .iter()
        .any(|key| matches!(key.as_str(), "ControlLeft" | "ControlRight"));
    let has_zero = pressed_keys.iter().any(|key| matches!(key.as_str(), "Num0" | "Kp0"));
    has_control && has_zero
}

fn is_drawing_pointer_shortcut_pressed(pressed_keys: &[String]) -> bool {
    let has_control = pressed_keys
        .iter()
        .any(|key| matches!(key.as_str(), "ControlLeft" | "ControlRight"));
    let has_nine = pressed_keys.iter().any(|key| matches!(key.as_str(), "Num9" | "Kp9"));
    has_control && has_nine
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
                if is_screen_drawing_shortcut_pressed(&app_state.pressed_keys) {
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
                if drawing_visible && is_drawing_pointer_shortcut_pressed(&app_state.pressed_keys) {
                    drop(app_state);
                    if let Err(error) = set_drawing_pointer_mode(&app_handle) {
                        eprintln!("Failed to set drawing pointer shortcut: {error}");
                    }
                    return;
                }
                if drawing_visible && key_name == "Delete" {
                    app_state.drawing_overlay.clear();
                    return;
                }
                // check if toggle shortcut is pressed
                if app_state.toggle_shortcut == app_state.pressed_keys {
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
                let should_send_drawing_move = if app_state.drawing_input_passthrough
                    && app_state.drawing_pointer_down
                {
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
fn control_is_down() -> bool {
    key_is_down(VK_CONTROL.0 as i32)
        || key_is_down(VK_LCONTROL.0 as i32)
        || key_is_down(VK_RCONTROL.0 as i32)
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
        let mut ctrl_0_was_down = false;
        let mut ctrl_9_was_down = false;

        loop {
            thread::sleep(Duration::from_millis(40));

            let ctrl_down = control_is_down();
            let ctrl_0_down = ctrl_down && key_is_down(VK_0);
            let ctrl_9_down = ctrl_down && key_is_down(VK_9);

            if ctrl_0_down && !ctrl_0_was_down {
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

            if ctrl_9_down && !ctrl_9_was_down {
                if let Err(error) = set_drawing_pointer_mode(&app_handle) {
                    eprintln!("Failed to set drawing pointer poller shortcut: {error}");
                }
            }

            ctrl_0_was_down = ctrl_0_down;
            ctrl_9_was_down = ctrl_9_down;
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
