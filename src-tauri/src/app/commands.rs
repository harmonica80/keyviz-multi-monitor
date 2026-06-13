use std::sync::Mutex;

use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_store::StoreExt;

use crate::app::state::{AppState, TrayMenuItems};
use crate::app::window::{
    monitor_identifier, position_cursor_window, position_overlay_window, set_window_monitor,
    OverlayAlignment,
};

#[tauri::command]
pub fn log(message: String) {
    println!("[LOG] {}", message);
}

#[tauri::command]
pub fn set_toggle_shortcut(app: tauri::AppHandle, shortcut: Vec<String>) {
    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().unwrap();
    app_state.toggle_shortcut = shortcut;
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSettingsPayload {
    show_clicks: bool,
    keep_highlight: bool,
    size: f64,
    color: String,
}

#[tauri::command]
pub fn get_cursor_settings(app: tauri::AppHandle) -> Result<CursorSettingsPayload, String> {
    let state = app.state::<Mutex<AppState>>();
    let app_state = state.lock().map_err(|error| error.to_string())?;
    Ok(CursorSettingsPayload {
        show_clicks: app_state.cursor_show_clicks,
        keep_highlight: app_state.cursor_keep_highlight,
        size: app_state.cursor_size,
        color: app_state.cursor_color.clone(),
    })
}

#[tauri::command]
pub fn set_tray_locale(app: tauri::AppHandle, locale: String) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().map_err(|error| error.to_string())?;
    app_state.locale = locale.clone();

    let tray = app.state::<TrayMenuItems>();
    let is_chinese = locale == "zh-TW";
    tray.toggle
        .set_text(if app_state.listening {
            if is_chinese {
                "\u{505c}\u{6b62}\u{986f}\u{793a}"
            } else {
                "Stop"
            }
        } else if is_chinese {
            "\u{958b}\u{59cb}\u{986f}\u{793a}"
        } else {
            "Start"
        })
        .map_err(|error| error.to_string())?;
    tray.settings
        .set_text(if is_chinese {
            "\u{8a2d}\u{5b9a}"
        } else {
            "Settings"
        })
        .map_err(|error| error.to_string())?;
    tray.quit
        .set_text(if is_chinese {
            "\u{7d50}\u{675f}\u{7a0b}\u{5f0f}"
        } else {
            "Quit"
        })
        .map_err(|error| error.to_string())?;

    let store = app.store("store.json").map_err(|error| error.to_string())?;
    store.set("tray_locale", serde_json::Value::String(locale));
    store.save().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_overlay_window(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
    alignment: OverlayAlignment,
    margin_x: f64,
    margin_y: f64,
) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let app_state = state.lock().map_err(|error| error.to_string())?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Visualization window is unavailable".to_string())?;

    position_overlay_window(
        &window, &app_state, width, height, alignment, margin_x, margin_y,
    )
}

#[tauri::command]
pub fn update_cursor_window(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    visible: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("cursor")
        .ok_or_else(|| "Cursor window is unavailable".to_string())?;

    position_cursor_window(&window, x, y, width.min(height) - 24.0, visible)
}

#[tauri::command]
pub fn set_cursor_settings(
    app: tauri::AppHandle,
    show_clicks: bool,
    keep_highlight: bool,
    size: f64,
    color: String,
) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let (x, y, visible) = {
        let mut app_state = state.lock().map_err(|error| error.to_string())?;
        app_state.cursor_show_clicks = show_clicks;
        app_state.cursor_keep_highlight = keep_highlight;
        app_state.cursor_size = size;
        app_state.cursor_color = color.clone();
        app_state.cursor_update_pending = true;
        (app_state.cursor_x, app_state.cursor_y, keep_highlight)
    };

    if let Some(window) = app.get_webview_window("cursor") {
        position_cursor_window(&window, x, y, size, visible)?;
        let mut app_state = state.lock().map_err(|error| error.to_string())?;
        app_state.cursor_window_visible = visible;
        app_state.cursor_update_pending = false;
    }

    app.emit_to(
        "cursor",
        "cursor-settings",
        CursorSettingsPayload {
            show_clicks,
            keep_highlight,
            size,
            color,
        },
    )
    .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn set_main_window_monitor(
    app: tauri::AppHandle,
    monitor_selector: String,
) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().unwrap();

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Visualization window is unavailable".to_string())?;
    let monitors = window
        .available_monitors()
        .map_err(|error| error.to_string())?;

    let target_monitor = monitors
        .iter()
        .find(|monitor| monitor_identifier(monitor) == monitor_selector)
        .cloned()
        .or_else(|| window.primary_monitor().ok().flatten())
        .or_else(|| monitors.into_iter().next())
        .ok_or_else(|| "No monitor is available".to_string())?;

    set_window_monitor(&window, &target_monitor, &mut app_state)
}
