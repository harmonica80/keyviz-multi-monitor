use crate::app::diagnostics::record_error;
use crate::app::state::AppState;

pub fn monitor_identifier(monitor: &tauri::Monitor) -> String {
    monitor.name().cloned().unwrap_or_else(|| {
        let position = monitor.position();
        format!("position:{},{}", position.x, position.y)
    })
}

pub fn set_window_monitor(
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
    app_state: &mut AppState,
) -> Result<(), String> {
    let position = monitor.position();
    window
        .set_fullscreen(false)
        .map_err(|error| error.to_string())?;

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE,
        };

        let hwnd = HWND(window.hwnd().map_err(|error| error.to_string())?.0 as isize);
        unsafe {
            let result = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                position.x,
                position.y,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE,
            );

            if !result.as_bool() {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        window
            .set_position(tauri::PhysicalPosition {
                x: position.x,
                y: position.y,
            })
            .map_err(|error| error.to_string())?;
    }

    app_state.monitor_name = Some(monitor_identifier(monitor));
    app_state.monitor_scale = monitor.scale_factor();
    app_state.monitor_position = (position.x, position.y);
    app_state.monitor_size = (monitor.size().width, monitor.size().height);

    Ok(())
}

pub fn config_window(window: &tauri::WebviewWindow, app_state: &mut AppState) {
    // A failure in any of these steps degrades the overlay (e.g. it could
    // intercept clicks) but must not abort the whole app at startup.
    if let Err(error) = window.set_ignore_cursor_events(true) {
        record_error(format!("Failed to set ignore cursor events: {error}"));
    }

    #[cfg(target_os = "windows")]
    {
        use std::ffi::c_void;
        use std::mem::size_of;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
            DWMWCP_DONOTROUND,
        };

        let Ok(hwnd) = window.hwnd().map(|handle| HWND(handle.0 as isize)) else {
            record_error("Failed to get visualization window handle");
            return;
        };
        let no_border: u32 = 0xFFFF_FFFE;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &no_border as *const u32 as *const c_void,
                size_of::<u32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &DWMWCP_DONOTROUND as *const _ as *const c_void,
                size_of_val(&DWMWCP_DONOTROUND) as u32,
            );
        }
    }

    let initial_monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.available_monitors().ok()?.into_iter().next());

    if let Some(monitor) = initial_monitor {
        if let Err(error) = set_window_monitor(window, &monitor, app_state) {
            record_error(format!("Failed to position visualization window: {error}"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        use cocoa::appkit::{NSWindow, NSWindowCollectionBehavior};
        use cocoa::base::id;

        unsafe {
            let ns_window = window.ns_window().unwrap() as id;
            ns_window.setLevel_(1000);

            ns_window.setCollectionBehavior_(
                NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces,
            );
        }
    }

    if let Err(error) = park_overlay_window(window) {
        record_error(format!("Failed to park visualization window: {error}"));
    }
}

pub fn park_overlay_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    // Keep the WebView running for input events without leaving it on a captured monitor.
    let monitors = window
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let parked_x = monitors
        .iter()
        .map(|monitor| {
            monitor
                .position()
                .x
                .saturating_add(monitor.size().width as i32)
        })
        .max()
        .unwrap_or(0)
        .saturating_add(64);
    let parked_y = monitors
        .iter()
        .map(|monitor| {
            monitor
                .position()
                .y
                .saturating_add(monitor.size().height as i32)
        })
        .max()
        .unwrap_or(0)
        .saturating_add(64);

    window
        .set_position(tauri::PhysicalPosition {
            x: parked_x,
            y: parked_y,
        })
        .map_err(|error| error.to_string())?;
    // Preserve the last rendered content size while parked. Shrinking to 1x1
    // creates a stale expansion frame, while a large fixed surface is outlined
    // by some browser recording APIs.
    window.show().map_err(|error| error.to_string())
}
