use std::time::Instant;

use serde::Deserialize;
use tauri::{image::Image, include_image, menu::MenuItem, Emitter, Wry};
use tauri_plugin_store::StoreExt;

#[derive(Default)]
pub struct AppState {
    pub listening: bool,
    pub pressed_keys: Vec<String>,
    pub toggle_shortcut: Vec<String>,

    pub monitor_name: Option<String>,
    pub monitor_scale: f64,
    pub monitor_position: (i32, i32),
    pub monitor_size: (u32, u32),
    pub locale: String,
    pub cursor_show_clicks: bool,
    pub cursor_keep_highlight: bool,
    pub cursor_size: f64,
    pub cursor_color: String,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub cursor_pressed: bool,
    pub cursor_click_until: Option<Instant>,
    pub cursor_update_pending: bool,
    pub cursor_window_visible: bool,
}

pub struct TrayMenuItems {
    pub toggle: MenuItem<Wry>,
    pub settings: MenuItem<Wry>,
    pub quit: MenuItem<Wry>,
}

impl AppState {
    pub fn new(app: &tauri::AppHandle) -> Self {
        let mut toggle_shortcut = vec!["Shift".to_string(), "F10".to_string()];
        let mut locale = "en".to_string();
        let mut cursor_show_clicks = false;
        let mut cursor_keep_highlight = false;
        let mut cursor_size = 150.0;
        let mut cursor_color = "#009dff".to_string();

        // load saved config from store
        if let Ok(store) = app.store("store.json") {
            if let Some(value) = store.get("key_event_store") {
                // the value comes in as a String: "{\"state\": ...}"
                if let Some(json_str) = value.as_str() {
                    // parse the inner string
                    match serde_json::from_str::<KeyEventStore>(json_str) {
                        Ok(parsed) => {
                            toggle_shortcut = parsed.state.toggle_shortcut;
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
                        cursor_show_clicks = parsed.state.mouse.show_clicks;
                        cursor_keep_highlight = parsed.state.mouse.keep_highlight;
                        cursor_size = parsed.state.mouse.size;
                        cursor_color = parsed.state.mouse.color;
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
            monitor_name: None,
            monitor_scale: 1.0,
            monitor_position: (0, 0),
            monitor_size: (1, 1),
            locale,
            cursor_show_clicks,
            cursor_keep_highlight,
            cursor_size,
            cursor_color,
            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_pressed: false,
            cursor_click_until: None,
            cursor_update_pending: false,
            cursor_window_visible: false,
        }
    }
    pub fn toggle_listener(&mut self, app: &tauri::AppHandle, toggle: &MenuItem<Wry>) {
        self.listening = !self.listening;

        if self.listening {
            println!("Listening enabled");
            toggle
                .set_text(if self.locale == "zh-TW" {
                    "\u{505c}\u{6b62}\u{986f}\u{793a}"
                } else {
                    "Stop"
                })
                .unwrap();
            app.tray_by_id("keyviz-tray")
                .unwrap()
                .set_icon(Some(Image::from(include_image!("icons/tray.png"))))
                .unwrap();
        } else {
            println!("Listening disabled");
            toggle
                .set_text(if self.locale == "zh-TW" {
                    "\u{958b}\u{59cb}\u{986f}\u{793a}"
                } else {
                    "Start"
                })
                .unwrap();
            app.tray_by_id("keyviz-tray")
                .unwrap()
                .set_icon(Some(Image::from(include_image!("icons/tray-disabled.png"))))
                .unwrap();
        }

        app.emit_to("main", "listening-toggle", self.listening)
            .unwrap();
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
    pub toggle_shortcut: Vec<String>,
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
    show_clicks: bool,
    keep_highlight: bool,
    size: f64,
    color: String,
}
