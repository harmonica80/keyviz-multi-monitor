use std::sync::Mutex;
use tauri::{
    image::Image,
    include_image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewWindow, WebviewWindowBuilder,
};

mod app;
use app::commands::{
    drawing_clear, drawing_redo, drawing_recover, drawing_probe_layer, drawing_set_color, drawing_set_tool, drawing_set_width, drawing_toggle_group,
    drawing_undo, get_cursor_settings, hide_native_key_overlay, log, set_cursor_settings,
    set_drawing_shortcuts, set_main_window_monitor, set_toggle_shortcut, set_tray_locale,
    update_native_key_overlay,
};
use app::diagnostics::{export_diagnostics, record_error};
use app::event::start_listener;
use app::native_drawing::NativeTool;
use app::state::{AppState, TrayMenuItems};
use app::window::config_window;

const APP_DISPLAY_NAME: &str = "Keyviz 鍵盤按鍵顯示器與螢幕繪圖";

fn app_window_title(app: &AppHandle) -> String {
    format!("{APP_DISPLAY_NAME} v{}", app.package_info().version)
}

fn build_settings_window(
    app: &AppHandle,
    visible: bool,
    always_on_top: bool,
) -> tauri::Result<WebviewWindow> {
    let webview_url = tauri::WebviewUrl::App("index.html#/settings".into());
    WebviewWindowBuilder::new(app, "settings", webview_url)
        .title(app_window_title(app))
        .inner_size(800.0, 640.0)
        .min_inner_size(640.0, 480.0)
        .max_inner_size(1000.0, 800.0)
        .maximizable(false)
        .always_on_top(always_on_top)
        .visible(visible)
        .build()
}

fn show_settings_window(app: &AppHandle) {
    request_settings_transition(app, true);
}

fn request_settings_transition(app: &AppHandle, show: bool) {
    {
        let state = app.state::<Mutex<AppState>>();
        let Ok(mut state) = state.lock() else { return; };
        if state.settings_opening { return; }
        if !show && (!state.drawing_visible || state.drawing_tool == "pointer") { return; }
        state.settings_opening = true;
    }
    // Native tool changes can send window messages to the main thread. Wait for
    // input release on a worker so neither UI thread blocks the other.
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(error) = prepare_drawing_for_settings(&app) {
            record_error(format!("Failed to release drawing input for settings: {error}"));
            if let Ok(mut state) = app.state::<Mutex<AppState>>().lock() { state.settings_opening = false; }
            return;
        }
        let handle = app.clone();
        if let Err(error) = app.run_on_main_thread(move || {
            if show { show_settings_window_ready(&handle); }
            if let Ok(mut state) = handle.state::<Mutex<AppState>>().lock() { state.settings_opening = false; };
        }) {
            record_error(format!("Failed to schedule settings window: {error}"));
            if let Ok(mut state) = app.state::<Mutex<AppState>>().lock() { state.settings_opening = false; };
        }
    });
}

fn prepare_drawing_for_settings(app: &AppHandle) -> Result<(), String> {
    let drawing = {
        let state = app.state::<Mutex<AppState>>();
        let state = state.lock().map_err(|error| error.to_string())?;
        state.drawing_visible.then(|| (state.drawing_overlay.clone(), state.drawing_session_id))
    };
    if let Some((overlay, session_id)) = drawing {
        overlay.prepare_for_settings()?;
        {
            let state = app.state::<Mutex<AppState>>();
            let mut state = state.lock().map_err(|error| error.to_string())?;
            if !state.drawing_visible || state.drawing_session_id != session_id {
                return Err("Drawing session changed while opening settings".into());
            }
            state.drawing_tool = "pointer".into();
            state.drawing_input_passthrough = true;
            state.drawing_pointer_down = false;
            state.drawing_last_move = None;
        }
        app.emit("drawing-tool-changed", serde_json::json!({ "tool": "pointer" }))
            .map_err(|error| error.to_string())?;
        app::diagnostics::record_event("Drawing input released before opening settings");
    }
    Ok(())
}

fn show_settings_window_ready(app: &AppHandle) {
    let drawing_visible = app
        .try_state::<Mutex<AppState>>()
        .and_then(|state| state.lock().ok().map(|state| state.drawing_visible))
        .unwrap_or(false);

    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_title(&app_window_title(app));
        let _ = window.set_always_on_top(drawing_visible);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit_to("main", "settings-window", true);
        return;
    }

    match build_settings_window(app, true, drawing_visible) {
        Ok(window) => {
            let _ = window.set_focus();
            let _ = app.emit_to("main", "settings-window", true);
        }
        Err(error) => {
            record_error(format!("Failed to create settings window: {error}"));
        }
    }
}

fn create_drawing_toolbar(
    app: &AppHandle,
    toolbar_x: i32,
    toolbar_y: i32,
    _toolbar_height: u32,
    reposition: bool,
) -> Result<(), String> {
    const TOOLBAR_WIDTH: f64 = 48.0;
    let (toolbar, is_new) = if let Some(toolbar) = app.get_webview_window("drawing-toolbar") {
        (toolbar, false)
    } else {
        let toolbar_url = tauri::WebviewUrl::App("index.html?mode=drawing-toolbar".into());
        (
            WebviewWindowBuilder::new(app, "drawing-toolbar", toolbar_url)
                .title("Keyviz Drawing Toolbar")
                .position(toolbar_x as f64, toolbar_y as f64)
                .inner_size(TOOLBAR_WIDTH, 32.0)
                .resizable(false)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .shadow(false)
                .visible(false)
                .build()
                .map_err(|error| error.to_string())?,
            true,
        )
    };

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetAncestor, SetWindowPos, GA_ROOT, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE,
        };

        let toolbar_hwnd = HWND(toolbar.hwnd().map_err(|error| error.to_string())?.0 as isize);
        let toolbar_root = unsafe { GetAncestor(toolbar_hwnd, GA_ROOT) };
        let toolbar_target = if toolbar_root.0 == 0 {
            toolbar_hwnd
        } else {
            toolbar_root
        };
        // The toolbar opens at the edge of the primary monitor, so size it by
        // the primary monitor's DPI. (A freshly created webview can still
        // report scale 1.0 at this point, which would mis-size the toolbar on
        // high-DPI screens.)
        let scale = app
            .primary_monitor()
            .ok()
            .flatten()
            .map(|monitor| monitor.scale_factor())
            .or_else(|| toolbar.scale_factor().ok())
            .unwrap_or(1.0);
        let toolbar_width = (TOOLBAR_WIDTH * scale).round() as i32;
        let initial_height = (32.0 * scale).round() as i32;
        use windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE;
        let result = unsafe {
            if is_new {
                SetWindowPos(
                    toolbar_target,
                    HWND_TOPMOST,
                    toolbar_x,
                    toolbar_y,
                    toolbar_width,
                    initial_height,
                    SWP_NOACTIVATE,
                )
            } else if reposition {
                SetWindowPos(
                    toolbar_target,
                    HWND_TOPMOST,
                    toolbar_x,
                    toolbar_y,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOSIZE,
                )
            } else {
                // Re-opening an existing toolbar: keep the position the user
                // dragged it to, only re-assert topmost.
                SetWindowPos(
                    toolbar_target,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                )
            }
        };
        if !result.as_bool() {
            return Err(std::io::Error::last_os_error().to_string());
        }

        if is_new {
            let toolbar_handle = toolbar_target.0;
            std::thread::spawn(move || {
                for delay in [100, 400] {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    unsafe {
                        let _ = SetWindowPos(
                            HWND(toolbar_handle),
                            HWND_TOPMOST,
                            toolbar_x,
                            toolbar_y,
                            toolbar_width,
                            initial_height,
                            SWP_NOACTIVATE | SWP_NOSIZE,
                        );
                    }
                }
            });
        }
    }

    let _ = app.emit_to("drawing-toolbar", "drawing-toolbar-resize-request", ());
    Ok(())
}

fn drawing_toolbar_open_position(app: &AppHandle, side: &str) -> Result<(i32, i32, u32), String> {
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let primary = app
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| "No primary monitor is available".to_string())?;
    let margin = 8;
    let toolbar_width = (48.0 * primary.scale_factor()).round().max(1.0) as i32;
    let toolbar_x = if side == "left" {
        primary.position().x + margin
    } else {
        primary.position().x + primary.size().width as i32 - toolbar_width - margin
    };
    let toolbar_y = primary.position().y + margin;
    let toolbar_height = (primary.size().height.saturating_sub(16)).min(820);
    Ok((toolbar_x, toolbar_y, toolbar_height))
}

fn keep_drawing_toolbar_above_canvas(app: &AppHandle) -> Result<(), String> {
    if !drawing_toolbar_visible(app)? {
        return Ok(());
    }
    let toolbar = app
        .get_webview_window("drawing-toolbar")
        .ok_or_else(|| "Drawing toolbar is unavailable".to_string())?;

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetAncestor, SetWindowPos, GA_ROOT, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE,
        };

        let toolbar_hwnd = HWND(toolbar.hwnd().map_err(|error| error.to_string())?.0 as isize);
        let toolbar_root = unsafe { GetAncestor(toolbar_hwnd, GA_ROOT) };
        let toolbar_target = if toolbar_root.0 == 0 {
            toolbar_hwnd
        } else {
            toolbar_root
        };

        let result = unsafe {
            SetWindowPos(
                toolbar_target,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            )
        };
        if !result.as_bool() {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }

    #[cfg(not(target_os = "windows"))]
    toolbar.set_always_on_top(true).map_err(|error| error.to_string())?;

    Ok(())
}

fn sync_drawing_toolbar_passthrough(app: &AppHandle) -> Result<(), String> {
    if !drawing_toolbar_visible(app)? {
        return Ok(());
    }
    let toolbar = app
        .get_webview_window("drawing-toolbar")
        .ok_or_else(|| "Drawing toolbar is unavailable".to_string())?;

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::{HWND, RECT};
        use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GetWindowRect, GA_ROOT};

        let toolbar_hwnd = HWND(toolbar.hwnd().map_err(|error| error.to_string())?.0 as isize);
        let toolbar_root = unsafe { GetAncestor(toolbar_hwnd, GA_ROOT) };
        let toolbar_target = if toolbar_root.0 == 0 {
            toolbar_hwnd
        } else {
            toolbar_root
        };
        let mut rect = RECT::default();
        let result = unsafe { GetWindowRect(toolbar_target, &mut rect) };
        if !result.as_bool() {
            return Err(std::io::Error::last_os_error().to_string());
        }

        let padding = 12;
        let state = app.state::<Mutex<AppState>>();
        let app_state = state.lock().map_err(|error| error.to_string())?;
        if !app_state.drawing_visible || app_state.drawing_toolbar_hidden {
            app_state.drawing_overlay.set_toolbar_passthrough(None);
            return Ok(());
        }
        app_state.drawing_overlay.set_toolbar_passthrough(Some((
            rect.left - padding,
            rect.top - padding,
            rect.right + padding,
            rect.bottom + padding,
        )));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let position = toolbar
            .outer_position()
            .map_err(|error| error.to_string())?;
        let size = toolbar.outer_size().map_err(|error| error.to_string())?;
        let padding = 12;
        let state = app.state::<Mutex<AppState>>();
        let app_state = state.lock().map_err(|error| error.to_string())?;
        if !app_state.drawing_visible || app_state.drawing_toolbar_hidden {
            app_state.drawing_overlay.set_toolbar_passthrough(None);
            return Ok(());
        }
        app_state.drawing_overlay.set_toolbar_passthrough(Some((
            position.x - padding,
            position.y - padding,
            position.x + size.width as i32 + padding,
            position.y + size.height as i32 + padding,
        )));
    }

    Ok(())
}

fn drawing_toolbar_visible(app: &AppHandle) -> Result<bool, String> {
    let state = app.state::<Mutex<AppState>>();
    let state = state.lock().map_err(|error| error.to_string())?;
    Ok(state.drawing_visible && !state.drawing_toolbar_hidden)
}

fn set_drawing_toolbar_visible(app: &AppHandle, visible: bool) -> Result<(), String> {
    let toolbar = app.get_webview_window("drawing-toolbar")
        .ok_or_else(|| "Drawing toolbar is unavailable".to_string())?;
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetAncestor, ShowWindow, GA_ROOT, SW_HIDE, SW_SHOWNOACTIVATE,
        };
        let hwnd = HWND(toolbar.hwnd().map_err(|error| error.to_string())?.0 as isize);
        let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
        let target = if root.0 == 0 { hwnd } else { root };
        unsafe { let _ = ShowWindow(target, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE }); }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if visible { toolbar.show() } else { toolbar.hide() }
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn hide_drawing_toolbar(app: AppHandle) -> Result<(), String> {
    {
        let state = app.state::<Mutex<AppState>>();
        let mut state = state.lock().map_err(|error| error.to_string())?;
        if !state.drawing_visible { return Ok(()); }
        state.drawing_toolbar_hidden = true;
        state.drawing_overlay.set_toolbar_hidden(true);
        state.drawing_overlay.set_toolbar_passthrough(None);
    }
    set_drawing_toolbar_visible(&app, false)
}

pub(crate) fn show_drawing_toolbar(app: &AppHandle) -> Result<(), String> {
    {
        let state = app.state::<Mutex<AppState>>();
        let mut state = state.lock().map_err(|error| error.to_string())?;
        if !state.drawing_visible { return Ok(()); }
        state.drawing_toolbar_hidden = false;
        state.drawing_overlay.set_toolbar_hidden(false);
    }
    set_drawing_toolbar_visible(app, true)?;
    keep_drawing_toolbar_above_canvas(app)?;
    sync_drawing_toolbar_passthrough(app)?;
    app.emit_to("drawing-toolbar", "drawing-toolbar-resize-request", ())
        .map_err(|error| error.to_string())
}

pub(crate) fn toggle_drawing_toolbar(app: &AppHandle) -> Result<(), String> {
    if drawing_toolbar_visible(app)? {
        hide_drawing_toolbar(app.clone())
    } else {
        show_drawing_toolbar(app)
    }
}

fn schedule_drawing_toolbar_passthrough_sync(app: &AppHandle) {
    let app_handle = app.clone();
    std::thread::spawn(move || {
        for delay in [80, 200, 500, 1000] {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            let _ = sync_drawing_toolbar_passthrough(&app_handle);
            let _ = keep_drawing_toolbar_above_canvas(&app_handle);
        }
    });
}

pub(crate) fn show_drawing_window(app: &AppHandle) -> Result<(), String> {
    initialize_drawing_session(app, false)
}

pub(crate) fn start_pointer_radial(app: &AppHandle) -> Result<(), String> {
    initialize_drawing_session(app, true)?;
    let state = app.state::<Mutex<AppState>>();
    let state = state.lock().map_err(|error| error.to_string())?;
    if state.settings_opening {
        return Err("Wait for settings to finish opening".into());
    }
    // Keep this behind Show/SetTool in the native queue, without waiting on the UI.
    state.drawing_overlay.request_pointer_radial();
    Ok(())
}

fn initialize_drawing_session(app: &AppHandle, hidden_pointer: bool) -> Result<(), String> {
    {
        let state = app.state::<Mutex<AppState>>();
        let state = state.lock().map_err(|error| error.to_string())?;
        if state.settings_opening {
            return Err("Wait for settings to finish opening".into());
        }
        if hidden_pointer && state.drawing_visible { return Ok(()); }
    }
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    if monitors.is_empty() {
        return Err("No monitor is available".to_string());
    }

    let drawing_monitors = monitors
        .iter()
        .map(|monitor| {
            (
                monitor.position().x,
                monitor.position().y,
                monitor.size().width as i32,
                monitor.size().height as i32,
            )
        })
        .collect();
    let drawing_toolbar_side = app
        .state::<Mutex<AppState>>()
        .lock()
        .map_err(|error| error.to_string())?
        .drawing_toolbar_side
        .clone();
    let (toolbar_x, toolbar_y, toolbar_height) =
        drawing_toolbar_open_position(app, &drawing_toolbar_side)?;

    create_drawing_toolbar(app, toolbar_x, toolbar_y, toolbar_height, false)?;
    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().map_err(|error| error.to_string())?;
    if app_state.settings_opening { return Err("Wait for settings to finish opening".into()); }
    if hidden_pointer && app_state.drawing_visible { return Ok(()); }
    let tool = if hidden_pointer { "pointer" } else { "pen" };
    app_state.pressed_keys.clear();
    app_state.drawing_visible = true;
    app_state.drawing_toolbar_hidden = hidden_pointer;
    app_state.drawing_overlay.set_toolbar_hidden(hidden_pointer);
    app_state.drawing_tool = tool.to_string();
    app_state.drawing_session_id = app_state.drawing_session_id.wrapping_add(1);
    let drawing_session_id = app_state.drawing_session_id;
    app_state.drawing_input_passthrough = hidden_pointer;
    app_state.drawing_pointer_down = false;
    app_state.drawing_last_move = None;
    // F8 starts with a pen; the global pointer gesture must not capture input.
    app_state.drawing_overlay.set_tool(if hidden_pointer { NativeTool::Pointer } else { NativeTool::Pen });
    app_state.drawing_overlay.show(drawing_monitors, None);
    app_state.drawing_overlay.set_click_through(hidden_pointer);
    app_state.drawing_overlay.raise();
    drop(app_state);
    set_drawing_toolbar_visible(app, !hidden_pointer)?;
    let _ = app.emit("drawing-tool-changed", serde_json::json!({ "tool": tool }));
    let _ = app.emit_to("drawing-toolbar", "drawing-toolbar-resize-request", ());
    keep_drawing_toolbar_above_canvas(app)?;
    sync_drawing_toolbar_passthrough(app)?;
    let app_handle = app.clone();
    std::thread::spawn(move || {
        for delay in [150, 500, 1000] {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            let should_raise = if let Ok(app_state) = app_handle.state::<Mutex<AppState>>().lock() {
                if !app_state.drawing_visible || app_state.drawing_session_id != drawing_session_id
                {
                    false
                } else {
                    app_state.drawing_overlay.raise();
                    true
                }
            } else {
                false
            };
            if !should_raise {
                break;
            }
            let _ = sync_drawing_toolbar_passthrough(&app_handle);
            let _ = keep_drawing_toolbar_above_canvas(&app_handle);
        }

        // Watchdog for the drawing session: other always-on-top tools (screen
        // recorders, capture utilities) can push our surfaces down the z-order.
        // RaiseIfDemoted only issues a z-order change when a surface actually
        // lost TOPMOST, so when nothing interferes this loop does no window
        // operations at all - unconditionally re-raising every second made
        // Keyviz fight other topmost windows (see docs/issues PixPin report).
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let settings_visible = app_handle
                .get_webview_window("settings")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false);
            let should_continue = if let Ok(app_state) =
                app_handle.state::<Mutex<AppState>>().lock()
            {
                if !app_state.drawing_visible || app_state.drawing_session_id != drawing_session_id
                {
                    false
                } else {
                    if !settings_visible {
                        app_state.drawing_overlay.raise_if_demoted();
                    }
                    true
                }
            } else {
                false
            };
            if !should_continue {
                break;
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn open_screen_drawing(app: AppHandle) -> Result<(), String> {
    std::thread::spawn(move || {
        let app_handle = app;
        if let Err(error) = show_drawing_window(&app_handle) {
            record_error(format!(
                "Failed to open screen drawing from settings: {error}"
            ));
            return;
        }
        if let Some(settings) = app_handle.get_webview_window("settings") {
            let _ = settings.hide();
        }
    });
    Ok(())
}

#[tauri::command]
fn close_screen_drawing(app: AppHandle) -> Result<(), String> {
    close_screen_drawing_impl(app)
}

#[tauri::command]
fn get_key_display_enabled(app: AppHandle) -> Result<bool, String> {
    app.state::<Mutex<AppState>>()
        .lock()
        .map(|state| state.listening)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn toggle_key_display(app: AppHandle) -> Result<bool, String> {
    let toggle_item = app.state::<TrayMenuItems>().toggle.clone();
    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().map_err(|error| error.to_string())?;
    app_state.toggle_listener(&app, &toggle_item);
    Ok(app_state.listening)
}

fn show_drawing_toolbar_label(is_chinese: bool) -> &'static str {
    if is_chinese {
        "\u{986f}\u{793a}\u{7e6a}\u{5716}\u{5de5}\u{5177}\u{5217}"
    } else {
        "Show Drawing Toolbar"
    }
}

#[tauri::command]
fn open_settings_window(app: AppHandle) -> Result<(), String> {
    let app_handle = app.clone();
    app.run_on_main_thread(move || show_settings_window(&app_handle))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_drawing_hover_edit(app: AppHandle, preferences: std::collections::HashMap<String, bool>) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let app_state = state.lock().map_err(|error| error.to_string())?;
    app_state.drawing_overlay.set_hover_edit(preferences);
    Ok(())
}

#[tauri::command]
fn set_drawing_radial_menu(app: AppHandle, shortcut: Vec<String>, tools: Vec<String>, right_click: bool, style: Option<String>, colors: Option<Vec<String>>) -> Result<(), String> {
    let overlay = app.state::<Mutex<AppState>>()
        .lock().map_err(|error| error.to_string())?.drawing_overlay.clone();
    overlay.configure_radial(shortcut, tools, right_click, style.unwrap_or_else(|| "pearl".into()), colors.unwrap_or_default())
}

#[tauri::command]
fn drawing_radial_menu_action(app: AppHandle, window: tauri::WebviewWindow, cancel: bool) -> Result<(), String> {
    if window.label() != "drawing-toolbar" {
        return Err("Radial menu input is only accepted from the drawing toolbar".into());
    }
    let state = app.state::<Mutex<AppState>>();
    let state = state.lock().map_err(|error| error.to_string())?;
    if !state.drawing_visible || state.drawing_tool == "pointer" { return Ok(()); }
    state.drawing_overlay.radial_menu_action(cancel)
}

#[tauri::command]
fn set_drawing_toolbar_side(app: AppHandle, side: String) -> Result<(), String> {
    let normalized_side = if side == "left" { "left" } else { "right" }.to_string();
    let drawing_visible = {
        let state = app.state::<Mutex<AppState>>();
        let mut app_state = state.lock().map_err(|error| error.to_string())?;
        app_state.drawing_toolbar_side.clone_from(&normalized_side);
        app_state.drawing_visible && !app_state.drawing_toolbar_hidden
    };

    if drawing_visible {
        let (toolbar_x, toolbar_y, toolbar_height) =
            drawing_toolbar_open_position(&app, &normalized_side)?;
        create_drawing_toolbar(&app, toolbar_x, toolbar_y, toolbar_height, true)?;
        keep_drawing_toolbar_above_canvas(&app)?;
    }

    Ok(())
}

pub(crate) fn close_screen_drawing_impl(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().map_err(|error| error.to_string())?;
    app_state.pressed_keys.clear();
    app_state.drawing_visible = false;
    app_state.drawing_toolbar_hidden = false;
    app_state.drawing_overlay.set_toolbar_hidden(false);
    app_state.drawing_overlay.set_toolbar_passthrough(None);
    app_state.drawing_session_id = app_state.drawing_session_id.wrapping_add(1);
    app_state.drawing_input_passthrough = false;
    app_state.drawing_pointer_down = false;
    app_state.drawing_last_move = None;
    app_state.drawing_overlay.hide();
    drop(app_state);

    if let Some(settings) = app.get_webview_window("settings") {
        let _ = settings.set_always_on_top(false);
    }

    #[cfg(target_os = "windows")]
    if let Some(toolbar) = app.get_webview_window("drawing-toolbar") {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetAncestor, SetWindowLongPtrW, ShowWindow, GA_ROOT, GWLP_HWNDPARENT, SW_HIDE,
        };

        let toolbar_hwnd = HWND(toolbar.hwnd().map_err(|error| error.to_string())?.0 as isize);
        let toolbar_root = unsafe { GetAncestor(toolbar_hwnd, GA_ROOT) };
        let toolbar_target = if toolbar_root.0 == 0 {
            toolbar_hwnd
        } else {
            toolbar_root
        };
        unsafe {
            SetWindowLongPtrW(toolbar_target, GWLP_HWNDPARENT, 0);
            let _ = ShowWindow(toolbar_target, SW_HIDE);
        }
    }

    #[cfg(not(target_os = "windows"))]
    if let Some(window) = app.get_webview_window("drawing-toolbar") {
        let _ = window.hide();
    }

    Ok(())
}

#[tauri::command]
fn set_drawing_click_through(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().map_err(|error| error.to_string())?;
    app_state.drawing_input_passthrough = enabled;
    if !enabled {
        app_state.drawing_pointer_down = false;
        app_state.drawing_last_move = None;
    }
    app_state.drawing_overlay.set_click_through(enabled);
    drop(app_state);
    keep_drawing_toolbar_above_canvas(&app)
}

#[tauri::command]
fn activate_drawing_toolbar(app: AppHandle) -> Result<(), String> {
    if !drawing_toolbar_visible(&app)? { return Ok(()); }
    keep_drawing_toolbar_above_canvas(&app)?;
    let toolbar = app
        .get_webview_window("drawing-toolbar")
        .ok_or_else(|| "Drawing toolbar is unavailable".to_string())?;
    toolbar.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
fn activate_drawing_canvas(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let app_state = state.lock().map_err(|error| error.to_string())?;
    app_state.drawing_overlay.focus();
    drop(app_state);
    keep_drawing_toolbar_above_canvas(&app)
}

#[tauri::command]
fn start_drawing_toolbar_drag(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("drawing-toolbar")
        .ok_or_else(|| "Drawing toolbar is unavailable".to_string())?;

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetAncestor, SendMessageW, GA_ROOT, HTCAPTION, WM_NCLBUTTONDOWN,
        };

        let hwnd = HWND(window.hwnd().map_err(|error| error.to_string())?.0 as isize);
        let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
        let target = if root.0 == 0 { hwnd } else { root };
        unsafe {
            let _ = ReleaseCapture();
            SendMessageW(
                target,
                WM_NCLBUTTONDOWN,
                WPARAM(HTCAPTION as usize),
                LPARAM(0),
            );
        }
        schedule_drawing_toolbar_passthrough_sync(&app);
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    window.start_dragging().map_err(|error| error.to_string())
}

#[tauri::command]
fn resize_drawing_toolbar(app: AppHandle, height: f64) -> Result<(), String> {
    if !drawing_toolbar_visible(&app)? {
        return Ok(());
    }

    let window = app
        .get_webview_window("drawing-toolbar")
        .ok_or_else(|| "Drawing toolbar is unavailable".to_string())?;

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetAncestor, SetWindowPos, GA_ROOT, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
        };

        let hwnd = HWND(window.hwnd().map_err(|error| error.to_string())?.0 as isize);
        let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
        let target = if root.0 == 0 { hwnd } else { root };
        let scale = window.scale_factor().unwrap_or(1.0);
        let physical_width = (48.0 * scale).round().max(1.0) as i32;
        let physical_height = (height.clamp(32.0, 820.0) * scale).ceil().max(1.0) as i32;
        let result = unsafe {
            SetWindowPos(
                target,
                HWND_TOPMOST,
                0,
                0,
                physical_width,
                physical_height,
                SWP_NOACTIVATE | SWP_NOMOVE,
            )
        };
        if !result.as_bool() {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }

    #[cfg(not(target_os = "windows"))]
    window
        .set_size(tauri::LogicalSize {
            width: 48.0,
            height: height.clamp(32.0, 820.0),
        })
        .map_err(|error| error.to_string())?;

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn};
        use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GA_ROOT};

        let hwnd = HWND(window.hwnd().map_err(|error| error.to_string())?.0 as isize);
        let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
        let target = if root.0 == 0 { hwnd } else { root };
        let size = window.inner_size().map_err(|error| error.to_string())?;
        let diameter = (48.0 * window.scale_factor().unwrap_or(1.0)).round() as i32;
        unsafe {
            // Clip the opaque WebView window, without enabling layered transparency.
            let region = CreateRoundRectRgn(
                0,
                0,
                size.width as i32 + 1,
                size.height as i32 + 1,
                diameter,
                diameter,
            );
            if region.0 == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            // Windows owns the region only when SetWindowRgn succeeds.
            if SetWindowRgn(target, region, true) == 0 {
                let error = std::io::Error::last_os_error().to_string();
                let _ = DeleteObject(region);
                return Err(error);
            }
        }
    }

    keep_drawing_toolbar_above_canvas(&app)?;
    sync_drawing_toolbar_passthrough(&app)
}

#[tauri::command]
fn resize_drawing_window(app: AppHandle) -> Result<(), String> {
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let drawing_monitors = monitors
        .iter()
        .map(|monitor| {
            (
                monitor.position().x,
                monitor.position().y,
                monitor.size().width as i32,
                monitor.size().height as i32,
            )
        })
        .collect();
    let state = app.state::<Mutex<AppState>>();
    let app_state = state.lock().map_err(|error| error.to_string())?;
    app_state.drawing_overlay.resize(drawing_monitors);
    Ok(())
}

/// React to a change in the monitor configuration (projector plugged in,
/// display unplugged, resolution or DPI change).
///
/// Called from the native drawing surfaces' WM_DISPLAYCHANGE handler and from
/// the fallback monitor watcher below. Re-parks the off-screen visualization
/// window (Windows may have dragged it back into view), refreshes the stored
/// monitor geometry used for cursor coordinates, and rebuilds the drawing
/// surfaces so they cover the new layout.
pub(crate) fn handle_display_change(app: &AppHandle) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST_RUN_MS: AtomicU64 = AtomicU64::new(0);
    let now = app::diagnostics::unix_time_ms();
    let last = LAST_RUN_MS.swap(now, Ordering::SeqCst);
    if now.saturating_sub(last) < 500 {
        return;
    }

    let monitors = match app.available_monitors() {
        Ok(monitors) if !monitors.is_empty() => monitors,
        _ => return,
    };
    let drawing_monitors: Vec<(i32, i32, i32, i32)> = monitors
        .iter()
        .map(|monitor| {
            (
                monitor.position().x,
                monitor.position().y,
                monitor.size().width as i32,
                monitor.size().height as i32,
            )
        })
        .collect();

    let app_handle = app.clone();
    let result = app.run_on_main_thread(move || {
        {
            let state = app_handle.state::<Mutex<AppState>>();
            let Ok(mut app_state) = state.lock() else {
                return;
            };
            // Rebuild drawing surfaces for the new layout (no-op when the
            // layout is unchanged; keeps annotations if a session is open).
            app_state.drawing_overlay.resize(drawing_monitors);

            // Re-attach the visualization window to its monitor: prefer the
            // one the user selected, fall back to the primary.
            if let Some(window) = app_handle.get_webview_window("main") {
                let selected = app_state.monitor_name.clone();
                let monitors = window.available_monitors().unwrap_or_default();
                let target = selected
                    .and_then(|name| {
                        monitors
                            .iter()
                            .find(|monitor| app::window::monitor_identifier(monitor) == name)
                            .cloned()
                    })
                    .or_else(|| window.primary_monitor().ok().flatten())
                    .or_else(|| monitors.into_iter().next());
                if let Some(monitor) = target {
                    let _ =
                        app::window::set_window_monitor(&window, &monitor, &mut app_state);
                }
                let key_overlay_visible = app_state.key_overlay_window_visible;
                drop(app_state);
                if !key_overlay_visible {
                    let _ = app::window::park_overlay_window(&window);
                }
            }
        }
        // Let the frontend re-query monitor-dependent state.
        let _ = app_handle.emit_to("main", "display-configuration-changed", ());
    });
    if let Err(error) = result {
        record_error(format!("Failed to apply display change: {error}"));
    } else {
        app::diagnostics::record_event("Display configuration change applied");
    }
}

/// Fallback watcher for monitor layout changes. The WM_DISPLAYCHANGE handler
/// on the drawing surfaces reacts instantly while a drawing session exists;
/// this low-frequency comparison covers the time before the first session.
fn start_monitor_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let snapshot = |app: &AppHandle| -> Option<Vec<(i32, i32, u32, u32)>> {
            let monitors = app.available_monitors().ok()?;
            Some(
                monitors
                    .iter()
                    .map(|monitor| {
                        (
                            monitor.position().x,
                            monitor.position().y,
                            monitor.size().width,
                            monitor.size().height,
                        )
                    })
                    .collect(),
            )
        };
        let mut previous = snapshot(&app);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let current = snapshot(&app);
            if current.is_some() && previous.is_some() && current != previous {
                app::diagnostics::record_event("Monitor layout changed (watcher)");
                handle_display_change(&app);
            }
            if current.is_some() {
                previous = current;
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_settings_window(app);
        }))
        .plugin(tauri_plugin_prevent_default::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle();
            app::diagnostics::init_file_logging(app_handle);
            start_monitor_watcher(app_handle.clone());
            let mut app_state = AppState::new(&app_handle);

            // prepare window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title(&app_window_title(app_handle));
                config_window(&window, &mut app_state);
            }
            // manage app state
            app.manage(Mutex::new(app_state));

            // Initialize WebView2 before the drawing overlays are active. Opening an
            // uninitialized settings webview from the drawing toolbar can otherwise
            // leave a blank modal-looking window behind the topmost overlay stack.
            if let Err(error) = build_settings_window(app_handle, false, false) {
                record_error(format!("Failed to preload settings window: {error}"));
            }

            // tray actions
            let is_chinese = app
                .state::<Mutex<AppState>>()
                .lock()
                .map(|state| state.locale == "zh-TW")
                .unwrap_or(false);
            let toggle_item = MenuItem::with_id(
                app,
                "toggle",
                if is_chinese {
                    "\u{505c}\u{6b62}\u{986f}\u{793a}"
                } else {
                    "Stop"
                },
                true,
                None::<&str>,
            )?;
            let settings_item = MenuItem::with_id(
                app,
                "settings",
                if is_chinese {
                    "\u{8a2d}\u{5b9a}"
                } else {
                    "Settings"
                },
                true,
                None::<&str>,
            )?;
            let drawing_item = MenuItem::with_id(
                app,
                "drawing",
                if is_chinese {
                    "\u{87a2}\u{5e55}\u{7e6a}\u{5716}"
                } else {
                    "Screen Drawing"
                },
                true,
                None::<&str>,
            )?;
            let show_toolbar_item = MenuItem::with_id(
                app,
                "show-drawing-toolbar",
                show_drawing_toolbar_label(is_chinese),
                true,
                None::<&str>,
            )?;
            let quit_item = MenuItem::with_id(
                app,
                "quit",
                if is_chinese {
                    "\u{7d50}\u{675f}\u{7a0b}\u{5f0f}"
                } else {
                    "Quit"
                },
                true,
                None::<&str>,
            )?;
            app.manage(TrayMenuItems {
                toggle: toggle_item.clone(),
                drawing: drawing_item.clone(),
                show_drawing_toolbar: show_toolbar_item.clone(),
                settings: settings_item.clone(),
                quit: quit_item.clone(),
            });

            // start global input listener
            start_listener(app_handle.clone(), toggle_item.clone());

            // setup tray menu
            let menu = Menu::with_items(
                app,
                &[&toggle_item, &drawing_item, &show_toolbar_item, &settings_item, &quit_item],
            )?;
            let _ = TrayIconBuilder::with_id("keyviz-tray")
                .icon(Image::from(include_image!("icons/tray.png")))
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "toggle" => {
                        let state = app.state::<Mutex<AppState>>();
                        if let Ok(mut app_state) = state.lock() {
                            app_state.toggle_listener(app, &toggle_item);
                        };
                    }
                    "settings" => {
                        show_settings_window(app);
                    }
                    "drawing" => {
                        if let Err(error) = show_drawing_window(app) {
                            record_error(format!("Failed to open screen drawing: {error}"));
                        }
                    }
                    "show-drawing-toolbar" => {
                        let app = app.clone();
                        std::thread::spawn(move || {
                            let active = match app.state::<Mutex<AppState>>().lock() {
                                Ok(state) => state.drawing_visible,
                                Err(error) => {
                                    record_error(format!("Failed to read drawing state: {error}"));
                                    return;
                                }
                            };
                            let result = if active {
                                show_drawing_toolbar(&app)
                            } else {
                                show_drawing_window(&app)
                            };
                            if let Err(error) = result {
                                record_error(format!("Failed to show drawing toolbar: {error}"));
                            }
                        });
                    }
                    // app.exit runs the normal Tauri shutdown path, which lets
                    // plugins (e.g. the settings store) flush before the
                    // process ends; std::process::exit would skip all of it.
                    "quit" => app.exit(0),
                    _ => println!("um... what?"),
                })
                .build(app);

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "settings" {
                return;
            }

            if let tauri::WindowEvent::Focused(true) = event {
                request_settings_transition(window.app_handle(), false);
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Keep the tray application alive until the user explicitly chooses to quit.
                api.prevent_close();

                #[cfg(target_os = "windows")]
                {
                    use windows::core::w;
                    use windows::Win32::Foundation::HWND;
                    use windows::Win32::UI::WindowsAndMessaging::{
                        MessageBoxW, IDNO, IDYES, MB_DEFBUTTON2, MB_ICONQUESTION, MB_YESNOCANCEL,
                    };

                    let hwnd = window
                        .hwnd()
                        .map(|handle| HWND(handle.0 as isize))
                        .unwrap_or(HWND(0));
                    let choice = unsafe {
                        MessageBoxW(
                            hwnd,
                            w!("您要結束 Keyviz 程式，還是僅關閉設定視窗？\n\n選擇「是」：結束程式\n選擇「否」：僅關閉視窗（程式仍在系統匣執行）"),
                            w!("關閉 Keyviz"),
                            MB_YESNOCANCEL | MB_ICONQUESTION | MB_DEFBUTTON2,
                        )
                    };

                    if choice == IDYES {
                        window.app_handle().exit(0);
                    } else if choice == IDNO {
                        let _ = window.hide();
                        let _ = window
                            .app_handle()
                            .emit_to("main", "settings-window", false);
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    let _ = window.hide();
                    let _ = window
                        .app_handle()
                        .emit_to("main", "settings-window", false);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            log,
            set_toggle_shortcut,
            set_drawing_shortcuts,
            set_main_window_monitor,
            set_tray_locale,
            update_native_key_overlay,
            hide_native_key_overlay,
            set_cursor_settings,
            get_cursor_settings,
            open_screen_drawing,
            get_key_display_enabled,
            toggle_key_display,
            open_settings_window,
            set_drawing_radial_menu,
            drawing_radial_menu_action,
            set_drawing_toolbar_side,
            set_drawing_hover_edit,
            close_screen_drawing,
            hide_drawing_toolbar,
            set_drawing_click_through,
            activate_drawing_toolbar,
            activate_drawing_canvas,
            start_drawing_toolbar_drag,
            resize_drawing_toolbar,
            resize_drawing_window,
            drawing_set_tool,
            drawing_set_color,
            drawing_set_width,
            drawing_clear,
            drawing_redo,
            drawing_recover,
            drawing_probe_layer,
            drawing_toggle_group,
            drawing_undo,
            export_diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
