use std::{
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use rdev::{listen, Button, EventType};
use serde::Serialize;
use tauri::{menu::MenuItem, AppHandle, Emitter, Manager, Wry};

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

pub fn start_listener(app_handle: AppHandle, toggle_menu_item: MenuItem<Wry>) {
    start_cursor_updater(app_handle.clone());

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
                app_state.pressed_keys.push(key_name);
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
