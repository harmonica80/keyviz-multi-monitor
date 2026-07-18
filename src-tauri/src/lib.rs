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
    drawing_clear, drawing_set_color, drawing_set_tool, drawing_set_width, drawing_toggle_group,
    drawing_undo, get_cursor_settings, log, set_cursor_settings, set_drawing_shortcuts,
    set_main_window_monitor, set_toggle_shortcut, set_tray_locale, update_overlay_window,
};
use app::event::start_listener;
use app::native_drawing::NativeTool;
use app::state::{AppState, TrayMenuItems};
use app::window::config_window;

fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let webview_url = tauri::WebviewUrl::App("index.html#/settings".into());
    if WebviewWindowBuilder::new(app, "settings", webview_url)
        .title("Keyviz 鍵盤按鍵顯示器與螢幕繪圖")
        .inner_size(800.0, 640.0)
        .min_inner_size(640.0, 480.0)
        .max_inner_size(1000.0, 800.0)
        .maximizable(false)
        .build()
        .is_ok()
    {
        let _ = app.emit_to("main", "settings-window", true);
    }
}

fn set_drawing_window_bounds(
    window: &WebviewWindow,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetAncestor, SetWindowPos, GA_ROOT, HWND_TOPMOST, SWP_SHOWWINDOW,
        };

        let hwnd = HWND(window.hwnd().map_err(|error| error.to_string())?.0 as isize);
        let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
        let target_hwnd = if root_hwnd.0 == 0 { hwnd } else { root_hwnd };
        let result = unsafe {
            SetWindowPos(
                target_hwnd,
                HWND_TOPMOST,
                left,
                top,
                width,
                height,
                SWP_SHOWWINDOW,
            )
        };
        if !result.as_bool() {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        window
            .set_position(tauri::PhysicalPosition { x: left, y: top })
            .map_err(|error| error.to_string())?;
        window
            .set_size(tauri::PhysicalSize {
                width: width as u32,
                height: height as u32,
            })
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn create_drawing_toolbar(
    app: &AppHandle,
    toolbar_x: i32,
    toolbar_y: i32,
    _toolbar_height: u32,
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
        let toolbar_width = (TOOLBAR_WIDTH * toolbar.scale_factor().unwrap_or(1.0)).round() as i32;
        let initial_height = (32.0 * toolbar.scale_factor().unwrap_or(1.0)).round() as i32;
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
            } else {
                SetWindowPos(
                    toolbar_target,
                    HWND_TOPMOST,
                    toolbar_x,
                    toolbar_y,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOSIZE,
                )
            }
        };
        if !result.as_bool() {
            return Err(std::io::Error::last_os_error().to_string());
        }

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

    #[cfg(not(target_os = "windows"))]
    toolbar.show().map_err(|error| error.to_string())?;

    let _ = app.emit_to("drawing-toolbar", "drawing-toolbar-resize-request", ());
    Ok(())
}

fn keep_drawing_toolbar_above_canvas(app: &AppHandle) -> Result<(), String> {
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
    toolbar.show().map_err(|error| error.to_string())?;

    Ok(())
}

fn sync_drawing_toolbar_passthrough(app: &AppHandle) -> Result<(), String> {
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
        app_state.drawing_overlay.set_toolbar_passthrough(Some((
            position.x - padding,
            position.y - padding,
            position.x + size.width as i32 + padding,
            position.y + size.height as i32 + padding,
        )));
    }

    Ok(())
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
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    if monitors.is_empty() {
        return Err("No monitor is available".to_string());
    }

    let left = monitors
        .iter()
        .map(|monitor| monitor.position().x)
        .min()
        .unwrap_or(0);
    let top = monitors
        .iter()
        .map(|monitor| monitor.position().y)
        .min()
        .unwrap_or(0);
    let right = monitors
        .iter()
        .map(|monitor| monitor.position().x + monitor.size().width as i32)
        .max()
        .unwrap_or(1);
    let bottom = monitors
        .iter()
        .map(|monitor| monitor.position().y + monitor.size().height as i32)
        .max()
        .unwrap_or(1);
    let primary = app
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| "No primary monitor is available".to_string())?;
    let drawing_width = right - left;
    let drawing_height = bottom - top;
    let toolbar_height = (primary.size().height.saturating_sub(16)).min(820);
    let toolbar_width = 64;
    let toolbar_x = primary.position().x + primary.size().width as i32 - toolbar_width - 8;
    let toolbar_y = primary.position().y + 8;

    create_drawing_toolbar(app, toolbar_x, toolbar_y, toolbar_height)?;
    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().map_err(|error| error.to_string())?;
    app_state.pressed_keys.clear();
    app_state.drawing_visible = true;
    app_state.drawing_session_id = app_state.drawing_session_id.wrapping_add(1);
    let drawing_session_id = app_state.drawing_session_id;
    app_state.drawing_input_passthrough = false;
    app_state.drawing_pointer_down = false;
    app_state.drawing_last_move = None;
    // Every new drawing session starts in pen mode. This avoids reopening the
    // overlay in pointer mode, which looks as though the Start Drawing button
    // did nothing.
    app_state.drawing_overlay.set_tool(NativeTool::Pen);
    app_state
        .drawing_overlay
        .show(left, top, drawing_width, drawing_height, None);
    app_state.drawing_overlay.set_click_through(false);
    app_state.drawing_overlay.raise();
    drop(app_state);
    let _ = app.emit("drawing-tool-changed", serde_json::json!({ "tool": "pen" }));
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
    });

    Ok(())
}

#[tauri::command]
fn open_screen_drawing(app: AppHandle) -> Result<(), String> {
    std::thread::spawn(move || {
        let app_handle = app;
        if let Err(error) = show_drawing_window(&app_handle) {
            eprintln!("Failed to open screen drawing from settings: {error}");
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

pub(crate) fn close_screen_drawing_impl(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let mut app_state = state.lock().map_err(|error| error.to_string())?;
    app_state.pressed_keys.clear();
    app_state.drawing_visible = false;
    app_state.drawing_session_id = app_state.drawing_session_id.wrapping_add(1);
    app_state.drawing_input_passthrough = false;
    app_state.drawing_pointer_down = false;
    app_state.drawing_last_move = None;
    app_state.drawing_overlay.hide();
    drop(app_state);

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
    keep_drawing_toolbar_above_canvas(&app)
}

#[tauri::command]
fn activate_drawing_toolbar(app: AppHandle) -> Result<(), String> {
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
    let drawing_visible = app
        .state::<Mutex<AppState>>()
        .lock()
        .map_err(|error| error.to_string())?
        .drawing_visible;
    if !drawing_visible {
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
            SWP_SHOWWINDOW,
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
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_SHOWWINDOW,
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

    keep_drawing_toolbar_above_canvas(&app)?;
    sync_drawing_toolbar_passthrough(&app)
}

#[tauri::command]
fn resize_drawing_window(app: AppHandle) -> Result<(), String> {
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let left = monitors
        .iter()
        .map(|monitor| monitor.position().x)
        .min()
        .unwrap_or(0);
    let top = monitors
        .iter()
        .map(|monitor| monitor.position().y)
        .min()
        .unwrap_or(0);
    let right = monitors
        .iter()
        .map(|monitor| monitor.position().x + monitor.size().width as i32)
        .max()
        .unwrap_or(1);
    let bottom = monitors
        .iter()
        .map(|monitor| monitor.position().y + monitor.size().height as i32)
        .max()
        .unwrap_or(1);
    let state = app.state::<Mutex<AppState>>();
    let app_state = state.lock().map_err(|error| error.to_string())?;
    app_state
        .drawing_overlay
        .resize(left, top, right - left, bottom - top);
    Ok(())
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
            let mut app_state = AppState::new(&app_handle);

            // prepare window
            if let Some(window) = app.get_webview_window("main") {
                config_window(&window, &mut app_state);
            }
            // manage app state
            app.manage(Mutex::new(app_state));

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
                settings: settings_item.clone(),
                quit: quit_item.clone(),
            });

            // start global input listener
            start_listener(app_handle.clone(), toggle_item.clone());

            // setup tray menu
            let menu = Menu::with_items(
                app,
                &[&toggle_item, &drawing_item, &settings_item, &quit_item],
            )?;
            let _ = TrayIconBuilder::with_id("keyviz-tray")
                .icon(Image::from(include_image!("icons/tray.png")))
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "toggle" => {
                        let state = app.state::<Mutex<AppState>>();
                        let mut app_state = state.lock().unwrap();
                        app_state.toggle_listener(app, &toggle_item);
                    }
                    "settings" => {
                        show_settings_window(app);
                    }
                    "drawing" => {
                        if let Err(error) = show_drawing_window(app) {
                            eprintln!("Failed to open screen drawing: {error}");
                        }
                    }
                    "quit" => std::process::exit(0),
                    _ => println!("um... what?"),
                })
                .build(app);

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "settings" {
                return;
            }
            match event {
                tauri::WindowEvent::CloseRequested { .. } => {
                    window
                        .app_handle()
                        .emit_to("main", "settings-window", false)
                        .unwrap();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            log,
            set_toggle_shortcut,
            set_drawing_shortcuts,
            set_main_window_monitor,
            set_tray_locale,
            update_overlay_window,
            set_cursor_settings,
            get_cursor_settings,
            open_screen_drawing,
            close_screen_drawing,
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
            drawing_toggle_group,
            drawing_undo
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
