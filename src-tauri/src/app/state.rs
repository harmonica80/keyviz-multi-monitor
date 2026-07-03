use serde::Deserialize;
use tauri::{image::Image, include_image, menu::MenuItem, Emitter, Manager, Wry};
use tauri_plugin_store::StoreExt;

use crate::app::native_cursor::NativeCursorOverlay;
use crate::app::native_drawing::NativeDrawingOverlay;

#[derive(Default)]
pub struct AppState {
    pub listening: bool,
    pub pressed_keys: Vec<String>,
    pub toggle_shortcut: Vec<String>,
    pub drawing_toggle_shortcut: Vec<String>,
    pub drawing_pointer_shortcut: Vec<String>,
    pub drawing_clear_shortcut: Vec<String>,
    pub drawing_undo_shortcut: Vec<String>,
    pub drawing_close_shortcut: Vec<String>,

    pub monitor_name: Option<String>,
    pub monitor_scale: f64,
    pub monitor_position: (i32, i32),
    pub monitor_size: (u32, u32),
    pub locale: String,
    pub cursor_keep_highlight: bool,
    pub cursor_size: f64,
    pub cursor_color: String,
    pub cursor_opacity: f64,
    pub cursor_thickness: f64,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub cursor_update_pending: bool,
    pub cursor_window_visible: bool,
    pub cursor_overlay: NativeCursorOverlay,
    pub drawing_overlay: NativeDrawingOverlay,
    pub drawing_visible: bool,
    pub drawing_input_passthrough: bool,
    pub drawing_pointer_down: bool,
    pub drawing_last_move: Option<std::time::Instant>,
}

pub struct TrayMenuItems {
    pub toggle: MenuItem<Wry>,
    pub drawing: MenuItem<Wry>,
    pub settings: MenuItem<Wry>,
    pub quit: MenuItem<Wry>,
}

impl AppState {
    pub fn new(app: &tauri::AppHandle) -> Self {
        let mut toggle_shortcut = default_toggle_shortcut();
        let mut drawing_toggle_shortcut = default_drawing_toggle_shortcut();
        let mut drawing_pointer_shortcut = default_drawing_pointer_shortcut();
        let mut drawing_clear_shortcut = default_drawing_clear_shortcut();
        let mut drawing_undo_shortcut = default_drawing_undo_shortcut();
        let mut drawing_close_shortcut = default_drawing_close_shortcut();
        let mut locale = "zh-TW".to_string();
        let mut cursor_keep_highlight = true;
        let mut cursor_size = 80.0;
        let mut cursor_color = "#ff0000".to_string();
        let mut cursor_opacity = 50.0;
        let mut cursor_thickness = 6.0;

        // load saved config from store
        if let Ok(store) = app.store("store.json") {
            if let Some(value) = store.get("key_event_store") {
                // the value comes in as a String: "{\"state\": ...}"
                if let Some(json_str) = value.as_str() {
                    // parse the inner string
                    match serde_json::from_str::<KeyEventStore>(json_str) {
                        Ok(parsed) => {
                            toggle_shortcut = parsed.state.toggle_shortcut;
                            drawing_toggle_shortcut = parsed.state.drawing_toggle_shortcut;
                            drawing_pointer_shortcut = parsed.state.drawing_pointer_shortcut;
                            drawing_clear_shortcut = parsed.state.drawing_clear_shortcut;
                            drawing_undo_shortcut = parsed.state.drawing_undo_shortcut;
                            drawing_close_shortcut = parsed.state.drawing_close_shortcut;
                        }
                        Err(e) => eprintln!("Failed to parse inner config JSON: {}", e),
                    }
                }
            }
        }

        if let Ok(store) = app.store("store.json") {
            if let Some(value) = store.get("key_style_store") {
                if let Some(json_str) = value.as_str() {
                    if let Ok(parsed) = serde_json::from_str::<KeyStyleStore>(json_str) {
                        cursor_keep_highlight = parsed.state.mouse.keep_highlight;
                        cursor_size = parsed.state.mouse.size;
                        cursor_color = parsed.state.mouse.color;
                        cursor_opacity = parsed.state.mouse.opacity;
                        cursor_thickness = parsed.state.mouse.thickness;
                    }
                }
            }
        }

        if let Ok(store) = app.store("store.json") {
            if let Some(value) = store.get("tray_locale") {
                if let Some(saved_locale) = value.as_str() {
                    locale = saved_locale.to_string();
                }
            }
        }

        Self {
            listening: true,
            pressed_keys: vec![],
            toggle_shortcut,
            drawing_toggle_shortcut,
            drawing_pointer_shortcut,
            drawing_clear_shortcut,
            drawing_undo_shortcut,
            drawing_close_shortcut,
            monitor_name: None,
            monitor_scale: 1.0,
            monitor_position: (0, 0),
            monitor_size: (1, 1),
            locale,
            cursor_keep_highlight,
            cursor_size,
            cursor_color,
            cursor_opacity,
            cursor_thickness,
            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_update_pending: false,
            cursor_window_visible: false,
            cursor_overlay: NativeCursorOverlay::new(),
            drawing_overlay: NativeDrawingOverlay::new(app),
            drawing_visible: false,
            drawing_input_passthrough: false,
            drawing_pointer_down: false,
            drawing_last_move: None,
        }
    }
    pub fn toggle_listener(&mut self, app: &tauri::AppHandle, toggle: &MenuItem<Wry>) {
        self.listening = !self.listening;

        if self.listening {
            println!("Listening enabled");
            let _ = toggle.set_text(if self.locale == "zh-TW" {
                "\u{505c}\u{6b62}\u{986f}\u{793a}"
            } else {
                "Stop"
            });
            if let Some(tray) = app.tray_by_id("keyviz-tray") {
                let _ = tray.set_icon(Some(Image::from(include_image!("icons/tray.png"))));
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
        } else {
            println!("Listening disabled");
            let _ = toggle.set_text(if self.locale == "zh-TW" {
                "\u{958b}\u{59cb}\u{986f}\u{793a}"
            } else {
                "Start"
            });
            if let Some(tray) = app.tray_by_id("keyviz-tray") {
                let _ = tray.set_icon(Some(Image::from(include_image!("icons/tray-disabled.png"))));
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }

        let _ = app.emit_to("main", "listening-toggle", self.listening);
    }
}

#[derive(Debug, Deserialize)]
struct KeyEventStore {
    pub state: KeyEventState,
    // pub version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyEventState {
    // pub drag_threshold: u32,
    // pub filter_hotkeys: bool,
    // pub ignore_modifiers: Vec<String>,
    // pub show_event_history: bool,
    // pub max_history: u32,
    // pub linger_duration_ms: u32,
    // pub show_mouse_events: bool,
    #[serde(default = "default_toggle_shortcut")]
    pub toggle_shortcut: Vec<String>,
    #[serde(default = "default_drawing_toggle_shortcut")]
    pub drawing_toggle_shortcut: Vec<String>,
    #[serde(default = "default_drawing_pointer_shortcut")]
    pub drawing_pointer_shortcut: Vec<String>,
    #[serde(default = "default_drawing_clear_shortcut")]
    pub drawing_clear_shortcut: Vec<String>,
    #[serde(default = "default_drawing_undo_shortcut")]
    pub drawing_undo_shortcut: Vec<String>,
    #[serde(default = "default_drawing_close_shortcut")]
    pub drawing_close_shortcut: Vec<String>,
}

fn default_toggle_shortcut() -> Vec<String> {
    vec!["ShiftLeft".to_string(), "F10".to_string()]
}

fn default_drawing_toggle_shortcut() -> Vec<String> {
    vec!["ControlLeft".to_string(), "Num0".to_string()]
}

fn default_drawing_pointer_shortcut() -> Vec<String> {
    vec!["ControlLeft".to_string(), "Num9".to_string()]
}

fn default_drawing_clear_shortcut() -> Vec<String> {
    vec!["Delete".to_string()]
}

fn default_drawing_undo_shortcut() -> Vec<String> {
    vec!["ControlLeft".to_string(), "KeyZ".to_string()]
}

fn default_drawing_close_shortcut() -> Vec<String> {
    vec!["Escape".to_string()]
}

#[derive(Debug, Deserialize)]
struct KeyStyleStore {
    state: KeyStyleState,
}

#[derive(Debug, Deserialize)]
struct KeyStyleState {
    mouse: CursorStyleState,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorStyleState {
    keep_highlight: bool,
    size: f64,
    color: String,
    #[serde(default = "default_cursor_opacity")]
    opacity: f64,
    #[serde(default = "default_cursor_thickness")]
    thickness: f64,
}

fn default_cursor_opacity() -> f64 {
    50.0
}

fn default_cursor_thickness() -> f64 {
    6.0
}
