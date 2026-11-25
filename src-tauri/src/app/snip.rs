//! Screen snipping ("scissors + pin"): the toolbar button freezes the screen under the toolbar
//! (drawings included), a full-screen selection window lets the user drag / adjust a region, and
//! the region can be saved as PNG / JSON, copied, or pinned as an always-on-top sticky note.
//!
//! Platform-independent: the Windows overlay only fills in the frozen pixels (`set_session`);
//! cropping, encoding, clipboard, notes and windows live here so they can be unit tested.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::Instant,
};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{
    ipc::Response, AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::app::export_io;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static NEXT_NOTE: AtomicU32 = AtomicU32::new(1);
/// Milliseconds (since `clock()` started) until which a just-closed selection still owns Esc.
static ESC_UNTIL: AtomicU64 = AtomicU64::new(0);

fn clock() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// True while the selection window is open, and for a moment after it closed: Esc must cancel
/// the selection only, not also end the drawing mode (the key is still down when it closes).
pub fn snip_active() -> bool {
    ACTIVE.load(Ordering::Relaxed) || clock() < ESC_UNTIL.load(Ordering::Relaxed)
}

fn is_open() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// One drawn object, in monitor-local coordinates, already serialised for the JSON file.
pub struct SnipItem {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub json: Value,
}

/// The frozen screen. Pixels are `0x??RRGGBB`, row by row, `width * height` long.
pub struct SnipSession {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    /// What the user saw, drawings included.
    pub composite: Vec<u32>,
    /// The screen without the drawings (what a JSON file stores under the drawing objects).
    pub raw: Vec<u32>,
    pub items: Vec<SnipItem>,
    pub dir: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

static SESSION: Mutex<Option<SnipSession>> = Mutex::new(None);

pub fn set_session(session: SnipSession) {
    if let Ok(mut slot) = SESSION.lock() {
        *slot = Some(session);
    }
}

fn clear_session() {
    if let Ok(mut slot) = SESSION.lock() {
        *slot = None;
    }
}

fn rgba_of(pixels: &[u32], width: u32, region: Region) -> Vec<u8> {
    let mut out = Vec::with_capacity(region.w as usize * region.h as usize * 4);
    for y in region.y..region.y + region.h {
        let row = y as usize * width as usize;
        for x in region.x..region.x + region.w {
            let p = pixels[row + x as usize];
            out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
        }
    }
    out
}

impl SnipSession {
    /// Rounds the selection (image pixels, may be fractional or negative) outward and clamps it.
    pub fn clamp(&self, x: f64, y: f64, w: f64, h: f64) -> Result<Region, String> {
        let x0 = x.floor().max(0.0);
        let y0 = y.floor().max(0.0);
        let x1 = (x + w).ceil().min(f64::from(self.width));
        let y1 = (y + h).ceil().min(f64::from(self.height));
        if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
            return Err("The selection is empty".into());
        }
        Ok(Region { x: x0 as u32, y: y0 as u32, w: (x1 - x0) as u32, h: (y1 - y0) as u32 })
    }

    pub fn full(&self) -> Region {
        Region { x: 0, y: 0, w: self.width, h: self.height }
    }

    /// RGBA of the region as the user saw it (drawings included).
    pub fn region_rgba(&self, region: Region) -> Vec<u8> {
        rgba_of(&self.composite, self.width, region)
    }

    /// Keyviz drawing file for the region: the plain screen crop plus the drawn objects, so it
    /// reopens in the drawing mode for editing. `region` tells the importer where the crop sits.
    pub fn region_json(&self, region: Region) -> Result<Vec<u8>, String> {
        let raw = rgba_of(&self.raw, self.width, region);
        let png = export_io::encode_png(region.w, region.h, &raw)?;
        let (x0, y0) = (region.x as i32, region.y as i32);
        let (x1, y1) = ((region.x + region.w) as i32, (region.y + region.h) as i32);
        let drawings: Vec<Value> = self
            .items
            .iter()
            .filter(|item| item.left < x1 && item.right > x0 && item.top < y1 && item.bottom > y0)
            .map(|item| item.json.clone())
            .collect();
        let document = json!({
            "format": export_io::JSON_FORMAT,
            "version": export_io::JSON_VERSION,
            "monitor": { "width": self.width, "height": self.height },
            "image": {
                "format": "png",
                "data": export_io::png_to_base64(&png),
                "region": { "x": region.x, "y": region.y, "width": region.w, "height": region.h },
            },
            "drawings": drawings,
        });
        serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())
    }
}

fn with_session<T>(f: impl FnOnce(&SnipSession) -> Result<T, String>) -> Result<T, String> {
    let slot = SESSION.lock().map_err(|e| e.to_string())?;
    let session = slot.as_ref().ok_or("There is no screenshot to work on")?;
    f(session)
}

pub fn copy_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| format!("Clipboard is unavailable: {e}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Owned(rgba),
        })
        .map_err(|e| format!("Cannot copy the image: {e}"))
}

// ---------------------------------------------------------------- selection window

fn close_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.destroy();
    }
}

/// Closes the selection window and drops the frozen screen (also used when the drawing mode ends).
pub fn close_selection(app: &AppHandle) {
    if is_open() {
        ESC_UNTIL.store(clock() + 500, Ordering::Relaxed);
    }
    ACTIVE.store(false, Ordering::Relaxed);
    clear_session();
    close_window(app, "snip");
}

#[cfg(target_os = "windows")]
fn raise_topmost(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetAncestor, SetWindowPos, GA_ROOT, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };
    if let Ok(hwnd) = window.hwnd() {
        let hwnd = HWND(hwnd.0 as isize);
        unsafe {
            let root = GetAncestor(hwnd, GA_ROOT);
            let target = if root.0 == 0 { hwnd } else { root };
            let _ = SetWindowPos(target, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn raise_topmost(_window: &tauri::WebviewWindow) {}

fn open_selection_window(app: &AppHandle, left: i32, top: i32, width: u32, height: u32) -> Result<(), String> {
    close_window(app, "snip");
    let window = WebviewWindowBuilder::new(app, "snip", WebviewUrl::App("index.html?mode=snip".into()))
        .title("Keyviz Snip")
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    // Move first so the window is on the right monitor (and DPI) before it is sized.
    for _ in 0..2 {
        let _ = window.set_position(PhysicalPosition::new(left, top));
        let _ = window.set_size(PhysicalSize::new(width, height));
    }
    window.show().map_err(|e| e.to_string())?;
    raise_topmost(&window);
    let _ = window.set_focus();
    Ok(())
}

#[tauri::command]
pub async fn snip_start(app: AppHandle, dir: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if is_open() {
            return Ok(());
        }
        let overlay = {
            let state = app.state::<Mutex<crate::app::state::AppState>>();
            let state = state.lock().map_err(|e| e.to_string())?;
            state.drawing_overlay.clone()
        };
        let (left, top, width, height) = overlay.snip_capture()?;
        if let Ok(mut slot) = SESSION.lock() {
            if let Some(session) = slot.as_mut() {
                session.dir = dir;
            }
        }
        ACTIVE.store(true, Ordering::Relaxed);
        if let Err(error) = open_selection_window(&app, left, top, width, height) {
            close_selection(&app);
            return Err(error);
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The frozen screen (PNG) for the selection window to show.
#[tauri::command]
pub fn snip_get_image() -> Result<Response, String> {
    let png = with_session(|session| {
        export_io::encode_png(session.width, session.height, &session.region_rgba(session.full()))
    })?;
    Ok(Response::new(png))
}

#[tauri::command]
pub fn snip_cancel(app: AppHandle) {
    close_selection(&app);
}

#[derive(Serialize)]
pub struct SnipResult {
    pub path: Option<String>,
}

/// `kind`: "png" | "json" | "copy" | "pin". The region is in image pixels.
#[tauri::command]
pub async fn snip_action(app: AppHandle, kind: String, x: f64, y: f64, w: f64, h: f64) -> Result<SnipResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (region, left, top, dir) = with_session(|session| {
            Ok((session.clamp(x, y, w, h)?, session.left, session.top, session.dir.clone()))
        })?;
        match kind.as_str() {
            "png" => {
                let rgba = with_session(|session| Ok(session.region_rgba(region)))?;
                let path = save_png(&app, dir.as_deref(), region.w, region.h, &rgba)?;
                Ok(SnipResult { path: Some(path) })
            }
            "json" => {
                let bytes = with_session(|session| session.region_json(region))?;
                let directory = export_io::export_directory(&app, dir.as_deref());
                let path = export_io::write_numbered(&directory, &crate::app::native_drawing::snip_date(), "json", &bytes)?;
                Ok(SnipResult { path: Some(path.to_string_lossy().into_owned()) })
            }
            "copy" => {
                let rgba = with_session(|session| Ok(session.region_rgba(region)))?;
                copy_rgba(region.w, region.h, rgba)?;
                close_selection(&app);
                Ok(SnipResult { path: None })
            }
            "pin" => {
                let rgba = with_session(|session| Ok(session.region_rgba(region)))?;
                create_note(&app, left + region.x as i32, top + region.y as i32, region.w, region.h, rgba, dir)?;
                close_selection(&app);
                Ok(SnipResult { path: None })
            }
            _ => Err("Unknown snip action".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

fn save_png(app: &AppHandle, dir: Option<&str>, w: u32, h: u32, rgba: &[u8]) -> Result<String, String> {
    let png = export_io::encode_png(w, h, rgba)?;
    let directory = export_io::export_directory(app, dir);
    let path = export_io::write_numbered(&directory, &crate::app::native_drawing::snip_date(), "png", &png)?;
    crate::app::diagnostics::record_event(format!("Saved screenshot to {}", path.display()));
    Ok(path.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------- sticky notes

struct Note {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    png: Vec<u8>,
    dir: Option<String>,
}

fn notes() -> &'static Mutex<HashMap<u32, Note>> {
    static NOTES: OnceLock<Mutex<HashMap<u32, Note>>> = OnceLock::new();
    NOTES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn note_label(id: u32) -> String {
    format!("pin-{id}")
}

fn create_note(app: &AppHandle, left: i32, top: i32, w: u32, h: u32, rgba: Vec<u8>, dir: Option<String>) -> Result<(), String> {
    let png = export_io::encode_png(w, h, &rgba)?;
    let id = NEXT_NOTE.fetch_add(1, Ordering::Relaxed);
    notes()
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id, Note { width: w, height: h, rgba, png, dir });
    let url = format!("index.html?mode=pin&id={id}");
    let built = WebviewWindowBuilder::new(app, note_label(id), WebviewUrl::App(url.into()))
        .title("Keyviz Note")
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(true)
        .visible(false)
        .build();
    let window = match built {
        Ok(window) => window,
        Err(error) => {
            let _ = notes().lock().map(|mut map| map.remove(&id));
            return Err(error.to_string());
        }
    };
    for _ in 0..2 {
        let _ = window.set_position(PhysicalPosition::new(left, top));
        let _ = window.set_size(PhysicalSize::new(w, h));
    }
    window.show().map_err(|e| e.to_string())?;
    raise_topmost(&window);
    Ok(())
}

#[tauri::command]
pub fn snip_note_image(id: u32) -> Result<Response, String> {
    let map = notes().lock().map_err(|e| e.to_string())?;
    let note = map.get(&id).ok_or("This note no longer exists")?;
    Ok(Response::new(note.png.clone()))
}

#[tauri::command]
pub fn snip_note_close(app: AppHandle, id: u32) {
    if let Ok(mut map) = notes().lock() {
        map.remove(&id);
    }
    close_window(&app, &note_label(id));
}

#[tauri::command]
pub fn snip_note_copy(id: u32) -> Result<(), String> {
    let (w, h, rgba) = {
        let map = notes().lock().map_err(|e| e.to_string())?;
        let note = map.get(&id).ok_or("This note no longer exists")?;
        (note.width, note.height, note.rgba.clone())
    };
    copy_rgba(w, h, rgba)
}

#[tauri::command]
pub async fn snip_note_save(app: AppHandle, id: u32) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (w, h, rgba, dir) = {
            let map = notes().lock().map_err(|e| e.to_string())?;
            let note = map.get(&id).ok_or("This note no longer exists")?;
            (note.width, note.height, note.rgba.clone(), note.dir.clone())
        };
        save_png(&app, dir.as_deref(), w, h, &rgba)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Lets the user drag the note: the page calls this on mouse down.
#[tauri::command]
pub fn snip_note_drag(app: AppHandle, id: u32) {
    if let Some(window) = app.get_webview_window(&note_label(id)) {
        let _ = window.start_dragging();
    }
}

/// New width for one wheel step: 10 % per notch, between 64 px and four times the original
/// (and never wider than 8000 px).
pub fn zoomed_width(current: f64, original: f64, steps: f64) -> f64 {
    let min = 64.0_f64.min(original);
    let max = (original * 4.0).min(8000.0).max(min);
    (current * 1.1_f64.powf(steps)).clamp(min, max)
}

/// Mouse wheel over a note: resizes it, keeping the point under the pointer fixed.
/// `steps` > 0 grows; `x`/`y` are the pointer position inside the page (CSS pixels).
#[tauri::command]
pub fn snip_note_zoom(app: AppHandle, id: u32, steps: f64, x: f64, y: f64) -> Result<(), String> {
    let (original_w, original_h) = {
        let map = notes().lock().map_err(|e| e.to_string())?;
        let note = map.get(&id).ok_or("This note no longer exists")?;
        (f64::from(note.width), f64::from(note.height))
    };
    let window = app.get_webview_window(&note_label(id)).ok_or("This note is closed")?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let position = window.outer_position().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let new_w = zoomed_width(f64::from(size.width), original_w, steps).round();
    let ratio = new_w / f64::from(size.width.max(1));
    if (ratio - 1.0).abs() < 1e-6 {
        return Ok(());
    }
    let new_h = (original_h * new_w / original_w).round().max(1.0);
    // Pointer position in screen pixels relative to the window, kept fixed while scaling.
    let (px, py) = (x * scale, y * scale);
    let new_x = f64::from(position.x) + px - px * ratio;
    let new_y = f64::from(position.y) + py - py * ratio;
    let _ = window.set_position(PhysicalPosition::new(new_x.round() as i32, new_y.round() as i32));
    let _ = window.set_size(PhysicalSize::new(new_w as u32, new_h as u32));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SnipSession {
        // 4 x 3 screen, pixel value = 0x00RRGGBB with R = x, G = y, B = 9.
        let mut composite = Vec::new();
        let mut raw = Vec::new();
        for y in 0..3u32 {
            for x in 0..4u32 {
                composite.push((x << 16) | (y << 8) | 9);
                raw.push((x << 16) | (y << 8) | 7);
            }
        }
        SnipSession {
            left: 100,
            top: 50,
            width: 4,
            height: 3,
            composite,
            raw,
            items: vec![
                SnipItem { left: 0, top: 0, right: 1, bottom: 1, json: json!({"id": "inside"}) },
                SnipItem { left: 3, top: 2, right: 4, bottom: 3, json: json!({"id": "outside"}) },
            ],
            dir: None,
        }
    }

    #[test]
    fn clamp_rounds_outward_and_limits_to_the_screen() {
        let s = session();
        assert_eq!(s.clamp(0.4, 0.6, 1.2, 1.1).unwrap(), Region { x: 0, y: 0, w: 2, h: 2 });
        assert_eq!(s.clamp(-5.0, -5.0, 100.0, 100.0).unwrap(), s.full());
        assert!(s.clamp(10.0, 10.0, 5.0, 5.0).is_err());
        assert!(s.clamp(1.0, 1.0, 0.0, 0.0).is_err());
    }

    #[test]
    fn region_rgba_crops_the_composite() {
        let s = session();
        let rgba = s.region_rgba(Region { x: 1, y: 1, w: 2, h: 1 });
        assert_eq!(rgba, vec![1, 1, 9, 255, 2, 1, 9, 255]);
    }

    #[test]
    fn region_json_keeps_raw_pixels_and_only_nearby_objects() {
        let s = session();
        let bytes = s.region_json(Region { x: 0, y: 0, w: 2, h: 2 }).unwrap();
        let document: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(document["format"], export_io::JSON_FORMAT);
        assert_eq!(document["monitor"]["width"], 4);
        assert_eq!(document["image"]["region"], json!({"x": 0, "y": 0, "width": 2, "height": 2}));
        assert_eq!(document["drawings"].as_array().unwrap().len(), 1);
        assert_eq!(document["drawings"][0]["id"], "inside");
        let png = export_io::base64_to_bytes(document["image"]["data"].as_str().unwrap()).unwrap();
        let (w, h, rgba) = export_io::decode_png(&png).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(&rgba[0..4], &[0, 0, 7, 255]);
    }

    #[test]
    fn note_zoom_steps_and_limits() {
        assert!((zoomed_width(100.0, 100.0, 1.0) - 110.0).abs() < 1e-9);
        assert!(zoomed_width(100.0, 100.0, -1.0) < 100.0);
        assert_eq!(zoomed_width(390.0, 100.0, 5.0), 400.0);
        assert_eq!(zoomed_width(70.0, 100.0, -10.0), 64.0);
        // A note smaller than the minimum never grows its lower limit above its own size.
        assert_eq!(zoomed_width(40.0, 40.0, -3.0), 40.0);
    }
}
