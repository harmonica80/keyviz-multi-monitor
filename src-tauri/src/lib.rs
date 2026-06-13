use std::sync::Mutex;

use tauri::{
    image::Image,
    include_image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewWindowBuilder,
};

mod app;
use app::commands::{
    get_cursor_settings, log, set_cursor_settings, set_main_window_monitor, set_toggle_shortcut,
    set_tray_locale, update_cursor_window, update_overlay_window,
};
use app::event::start_listener;
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
        .title("Keyviz 鍵盤按鍵顯示器（支援多螢幕）")
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
            if let Some(window) = app.get_webview_window("cursor") {
                window.set_ignore_cursor_events(true)?;
                window.eval("window.location.hash = '/cursor';")?;
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
                settings: settings_item.clone(),
                quit: quit_item.clone(),
            });

            // start global input listener
            start_listener(app_handle.clone(), toggle_item.clone());

            // setup tray menu
            let menu = Menu::with_items(app, &[&toggle_item, &settings_item, &quit_item])?;
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
            set_main_window_monitor,
            set_tray_locale,
            update_overlay_window,
            update_cursor_window,
            set_cursor_settings,
            get_cursor_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
