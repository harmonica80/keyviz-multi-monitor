use crate::app::state::AppState;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayAlignment {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

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
            SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW,
        };

        window
            .set_size(tauri::PhysicalSize {
                width: 1,
                height: 1,
            })
            .map_err(|error| error.to_string())?;
        let hwnd = HWND(window.hwnd().map_err(|error| error.to_string())?.0 as isize);
        unsafe {
            let result = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                position.x,
                position.y,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE | SWP_SHOWWINDOW,
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
        window
            .set_size(tauri::PhysicalSize {
                width: 1,
                height: 1,
            })
            .map_err(|error| error.to_string())?;
    }

    app_state.monitor_name = Some(monitor_identifier(monitor));
    app_state.monitor_scale = monitor.scale_factor();
    app_state.monitor_position = (position.x, position.y);
    app_state.monitor_size = (monitor.size().width, monitor.size().height);

    Ok(())
}

pub fn position_overlay_window(
    window: &tauri::WebviewWindow,
    app_state: &AppState,
    logical_width: f64,
    logical_height: f64,
    alignment: OverlayAlignment,
    margin_x: f64,
    margin_y: f64,
) -> Result<(), String> {
    let scale = app_state.monitor_scale;
    let width = (logical_width.max(1.0) * scale).ceil() as i32;
    let height = (logical_height.max(1.0) * scale).ceil() as i32;
    let margin_x = (margin_x * scale).round() as i32;
    let margin_y = (margin_y * scale).round() as i32;
    let monitor_width = app_state.monitor_size.0 as i32;
    let monitor_height = app_state.monitor_size.1 as i32;

    let left = margin_x;
    let center_x = (monitor_width - width) / 2;
    let right = monitor_width - width - margin_x;
    let top = margin_y;
    let center_y = (monitor_height - height) / 2;
    let bottom = monitor_height - height - margin_y;

    let (relative_x, relative_y) = match alignment {
        OverlayAlignment::TopLeft => (left, top),
        OverlayAlignment::TopCenter => (center_x, top),
        OverlayAlignment::TopRight => (right, top),
        OverlayAlignment::CenterLeft => (left, center_y),
        OverlayAlignment::Center => (center_x, center_y),
        OverlayAlignment::CenterRight => (right, center_y),
        OverlayAlignment::BottomLeft => (left, bottom),
        OverlayAlignment::BottomCenter => (center_x, bottom),
        OverlayAlignment::BottomRight => (right, bottom),
    };

    let x = app_state.monitor_position.0 + relative_x.max(0);
    let y = app_state.monitor_position.1 + relative_y.max(0);

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW,
        };

        window
            .set_size(tauri::PhysicalSize {
                width: width as u32,
                height: height as u32,
            })
            .map_err(|error| error.to_string())?;
        let hwnd = HWND(window.hwnd().map_err(|error| error.to_string())?.0 as isize);
        unsafe {
            let result = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                y,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            if !result.as_bool() {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        window
            .set_position(tauri::PhysicalPosition { x, y })
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

pub fn config_window(window: &tauri::WebviewWindow, app_state: &mut AppState) {
    window
        .set_ignore_cursor_events(true)
        .expect("Failed to set ignore cursor events");

    #[cfg(target_os = "windows")]
    {
        use std::ffi::c_void;
        use std::mem::size_of;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
            DWMWCP_DONOTROUND,
        };

        let hwnd = HWND(
            window
                .hwnd()
                .expect("Failed to get visualization window handle")
                .0 as isize,
        );
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
        set_window_monitor(window, &monitor, app_state)
            .expect("Failed to position visualization window");
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

    window.show().expect("Failed to show window");
}
