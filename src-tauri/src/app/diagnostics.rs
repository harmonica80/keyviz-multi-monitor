use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::app::native_drawing::NativeDrawingSnapshot;
use crate::app::state::AppState;

const MAX_RECENT_ERRORS: usize = 100;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticError {
    timestamp_unix_ms: u64,
    message: String,
}

static RECENT_ERRORS: OnceLock<Mutex<VecDeque<DiagnosticError>>> = OnceLock::new();

fn error_log() -> &'static Mutex<VecDeque<DiagnosticError>> {
    RECENT_ERRORS.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_RECENT_ERRORS)))
}

pub fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub fn record_error(message: impl Into<String>) {
    let message = message.into();
    eprintln!("{message}");
    if let Ok(mut errors) = error_log().lock() {
        if errors.len() == MAX_RECENT_ERRORS {
            errors.pop_front();
        }
        errors.push_back(DiagnosticError {
            timestamp_unix_ms: unix_time_ms(),
            message,
        });
    }
}

fn recent_errors() -> Vec<DiagnosticError> {
    error_log()
        .lock()
        .map(|errors| errors.iter().cloned().collect())
        .unwrap_or_default()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorDiagnostic {
    index: usize,
    name: Option<String>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale_factor: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DrawingStateDiagnostic {
    visible: bool,
    current_tool: String,
    input_passthrough: bool,
    pointer_down: bool,
    session_id: u64,
    toolbar_side: String,
    native: NativeDrawingSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MouseHookDiagnostic {
    status: String,
    event_count: u64,
    last_event_unix_ms: Option<u64>,
    last_error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationStateDiagnostic {
    key_display_enabled: bool,
    pressed_key_count: usize,
    key_overlay_visible: bool,
    cursor_overlay_visible: bool,
    selected_key_monitor: Option<String>,
    drawing: DrawingStateDiagnostic,
    mouse_hook: MouseHookDiagnostic,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowDiagnostic {
    source: String,
    role: String,
    hwnd: i64,
    exists: bool,
    visible: bool,
    topmost: bool,
    layered: bool,
    transparent_input: bool,
    no_activate: bool,
    no_redirection_bitmap: bool,
    rect: Option<[i32; 4]>,
    expected_rect: Option<[i32; 4]>,
    canvas_size: Option<[i32; 2]>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticReport {
    schema_version: u32,
    generated_at_unix_ms: u64,
    app_version: String,
    process_id: u32,
    platform: String,
    architecture: String,
    monitors: Vec<MonitorDiagnostic>,
    application_state: ApplicationStateDiagnostic,
    windows: Vec<WindowDiagnostic>,
    recent_errors: Vec<DiagnosticError>,
}

#[cfg(target_os = "windows")]
fn inspect_window(
    source: &str,
    role: &str,
    raw_hwnd: i64,
    expected_rect: Option<[i32; 4]>,
    canvas_size: Option<[i32; 2]>,
) -> WindowDiagnostic {
    use windows::Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::{
            GetWindowLongPtrW, GetWindowRect, IsWindow, IsWindowVisible, GWL_EXSTYLE,
            WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOPMOST,
            WS_EX_TRANSPARENT,
        },
    };

    let hwnd = HWND(raw_hwnd as isize);
    let exists = unsafe { IsWindow(hwnd).as_bool() };
    let mut rect = RECT::default();
    let rect = if exists && unsafe { GetWindowRect(hwnd, &mut rect).as_bool() } {
        Some([rect.left, rect.top, rect.right, rect.bottom])
    } else {
        None
    };
    let ex_style = if exists {
        unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 }
    } else {
        0
    };
    let has_style = |style: u32| ex_style & style != 0;

    WindowDiagnostic {
        source: source.to_string(),
        role: role.to_string(),
        hwnd: raw_hwnd,
        exists,
        visible: exists && unsafe { IsWindowVisible(hwnd).as_bool() },
        topmost: has_style(WS_EX_TOPMOST.0),
        layered: has_style(WS_EX_LAYERED.0),
        transparent_input: has_style(WS_EX_TRANSPARENT.0),
        no_activate: has_style(WS_EX_NOACTIVATE.0),
        no_redirection_bitmap: has_style(WS_EX_NOREDIRECTIONBITMAP.0),
        rect,
        expected_rect,
        canvas_size,
    }
}

#[cfg(not(target_os = "windows"))]
fn inspect_window(
    source: &str,
    role: &str,
    raw_hwnd: i64,
    expected_rect: Option<[i32; 4]>,
    canvas_size: Option<[i32; 2]>,
) -> WindowDiagnostic {
    WindowDiagnostic {
        source: source.to_string(),
        role: role.to_string(),
        hwnd: raw_hwnd,
        exists: false,
        visible: false,
        topmost: false,
        layered: false,
        transparent_input: false,
        no_activate: false,
        no_redirection_bitmap: false,
        rect: None,
        expected_rect,
        canvas_size,
    }
}

fn export_directory() -> PathBuf {
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let downloads = PathBuf::from(profile).join("Downloads");
        if downloads.is_dir() {
            return downloads;
        }
    }
    std::env::temp_dir()
}

#[tauri::command]
pub fn export_diagnostics(app: AppHandle) -> Result<String, String> {
    let monitors = app
        .available_monitors()
        .map_err(|error| {
            let message = format!("Failed to enumerate monitors for diagnostics: {error}");
            record_error(message.clone());
            message
        })?
        .into_iter()
        .enumerate()
        .map(|(index, monitor)| MonitorDiagnostic {
            index: index + 1,
            name: monitor.name().map(ToOwned::to_owned),
            x: monitor.position().x,
            y: monitor.position().y,
            width: monitor.size().width,
            height: monitor.size().height,
            scale_factor: monitor.scale_factor(),
        })
        .collect();

    let (application_state, drawing_snapshot, cursor_hwnds, key_hwnds) = {
        let state = app.state::<Mutex<AppState>>();
        let app_state = state.lock().map_err(|error| {
            let message = format!("Failed to lock application state for diagnostics: {error}");
            record_error(message.clone());
            message
        })?;
        let drawing_snapshot = app_state.drawing_overlay.diagnostics();
        let cursor_hwnds = app_state.cursor_overlay.diagnostic_hwnds();
        let key_hwnds = app_state.key_overlay.diagnostic_hwnds();
        let application_state = ApplicationStateDiagnostic {
            key_display_enabled: app_state.listening,
            pressed_key_count: app_state.pressed_keys.len(),
            key_overlay_visible: app_state.key_overlay_window_visible,
            cursor_overlay_visible: app_state.cursor_window_visible,
            selected_key_monitor: app_state.monitor_name.clone(),
            drawing: DrawingStateDiagnostic {
                visible: app_state.drawing_visible,
                current_tool: app_state.drawing_tool.clone(),
                input_passthrough: app_state.drawing_input_passthrough,
                pointer_down: app_state.drawing_pointer_down,
                session_id: app_state.drawing_session_id,
                toolbar_side: app_state.drawing_toolbar_side.clone(),
                native: drawing_snapshot.clone(),
            },
            mouse_hook: MouseHookDiagnostic {
                status: app_state.mouse_hook_status.clone(),
                event_count: app_state.mouse_hook_event_count,
                last_event_unix_ms: app_state.mouse_hook_last_event_unix_ms,
                last_error: app_state.mouse_hook_last_error.clone(),
            },
        };
        (application_state, drawing_snapshot, cursor_hwnds, key_hwnds)
    };

    let mut windows = Vec::new();
    for (label, window) in app.webview_windows() {
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = window.hwnd() {
            windows.push(inspect_window("tauri", &label, hwnd.0 as i64, None, None));
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = window;
        }
    }
    for drawing_window in drawing_snapshot.windows {
        windows.push(inspect_window(
            "native-drawing",
            &drawing_window.role,
            drawing_window.hwnd,
            Some(drawing_window.bounds),
            drawing_window.canvas_size,
        ));
    }
    for hwnd in cursor_hwnds {
        windows.push(inspect_window(
            "native-cursor",
            "cursor-highlight",
            hwnd,
            None,
            None,
        ));
    }
    for hwnd in key_hwnds {
        windows.push(inspect_window(
            "native-key",
            "keyboard-visualizer",
            hwnd,
            None,
            None,
        ));
    }

    let report = DiagnosticReport {
        schema_version: 1,
        generated_at_unix_ms: unix_time_ms(),
        app_version: app.package_info().version.to_string(),
        process_id: std::process::id(),
        platform: std::env::consts::OS.to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        monitors,
        application_state,
        windows,
        recent_errors: recent_errors(),
    };
    let json = serde_json::to_string_pretty(&report).map_err(|error| {
        let message = format!("Failed to serialize diagnostics: {error}");
        record_error(message.clone());
        message
    })?;

    let path = export_directory().join(format!(
        "keyviz-diagnostics-{}.json",
        report.generated_at_unix_ms
    ));
    fs::write(&path, json).map_err(|error| {
        let message = format!("Failed to write diagnostics to {}: {error}", path.display());
        record_error(message.clone());
        message
    })?;

    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_timestamp_is_available() {
        assert!(unix_time_ms() > 1_700_000_000_000);
    }
}
