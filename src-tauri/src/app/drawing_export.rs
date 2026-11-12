// Saving the drawing screen as PNG / JSON and re-opening a JSON for editing.
// Included into `platform` (see native_drawing.rs) so it can use the private drawing model.
//
// Coordinates: drawings live in "union" coordinates (relative to `state.bounds`). Files use
// monitor-local coordinates (0,0 = top-left of the saved monitor) so they are portable.

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExportKind {
    Png,
    Json,
}

/// A frozen screenshot shown under the drawings (set by opening a JSON file).
struct ScreenBackground {
    /// Monitor rectangle in virtual-screen coordinates.
    rect: RECT,
    /// Opaque 0xffRRGGBB pixels, `rect`-sized, never pure black / the transparency key.
    pixels: Vec<u32>,
}

impl ScreenBackground {
    fn pixel(&self, x: i32, y: i32) -> Option<u32> {
        if x < self.rect.left || x >= self.rect.right || y < self.rect.top || y >= self.rect.bottom {
            return None;
        }
        let width = (self.rect.right - self.rect.left) as usize;
        self.pixels
            .get((y - self.rect.top) as usize * width + (x - self.rect.left) as usize)
            .copied()
    }
}

/// Makes a screenshot pixel opaque and keeps it away from the overlay's transparent keys.
fn opaque_visible(pixel: u32) -> u32 {
    let rgb = pixel & 0x00ff_ffff;
    let rgb = if rgb == 0 {
        0x0001_0101
    } else if rgb == TRANSPARENT_PIXEL {
        0x0001_0204
    } else {
        rgb
    };
    0xff00_0000 | rgb
}

struct DibSurface {
    dc: CreatedHDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut c_void,
    width: i32,
    height: i32,
}

impl DibSurface {
    unsafe fn new(width: i32, height: i32) -> Result<Self, String> {
        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            return Err("GetDC failed".into());
        }
        let dc = CreateCompatibleDC(screen_dc);
        if dc.0 == 0 {
            ReleaseDC(HWND(0), screen_dc);
            return Err("CreateCompatibleDC failed".into());
        }
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &mut info, DIB_RGB_COLORS, &mut bits, HANDLE(0), 0);
        ReleaseDC(HWND(0), screen_dc);
        let Ok(bitmap) = bitmap else {
            DeleteDC(dc);
            return Err(format!("CreateDIBSection failed for {width}x{height}"));
        };
        if bits.is_null() {
            DeleteObject(bitmap);
            DeleteDC(dc);
            return Err("CreateDIBSection returned no pixels".into());
        }
        let old = SelectObject(dc, bitmap);
        Ok(Self { dc, bitmap, old, bits, width, height })
    }

    fn hdc(&self) -> HDC {
        HDC(self.dc.0)
    }

    unsafe fn pixels(&self) -> &mut [u32] {
        std::slice::from_raw_parts_mut(self.bits as *mut u32, self.width as usize * self.height as usize)
    }

    unsafe fn free(self) {
        SelectObject(self.dc, self.old);
        DeleteObject(self.bitmap);
        DeleteDC(self.dc);
    }
}

fn today_stamp() -> String {
    let now = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}{:02}{:02}", now.wYear, now.wMonth, now.wDay)
}


/// Monitor (surface) under the pointer, falling back to the first one.
unsafe fn target_surface(state: &OverlayState) -> Option<usize> {
    let mut cursor = WinPoint::default();
    if windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor).as_bool() {
        if let Some(index) = state.surfaces.iter().position(|s| {
            cursor.x >= s.bounds.left && cursor.x < s.bounds.right
                && cursor.y >= s.bounds.top && cursor.y < s.bounds.bottom
        }) {
            return Some(index);
        }
    }
    if state.surfaces.is_empty() { None } else { Some(0) }
}

/// Grabs the real screen (without any of our own overlay windows) as 0x??RRGGBB pixels.
unsafe fn capture_screen(state: &OverlayState, rect: RECT) -> Result<Vec<u32>, String> {
    use windows::Win32::Graphics::Gdi::{BitBlt, ROP_CODE, CAPTUREBLT, SRCCOPY};
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WINDOW_DISPLAY_AFFINITY};
    use windows::Win32::System::Threading::Sleep;

    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);
    let mut targets: Vec<(HWND, bool)> = Vec::new();
    for surface in &state.surfaces {
        targets.push((surface.display_hwnd, true));
        targets.push((surface.input_hwnd, false));
    }
    for label in ["drawing-toolbar", "drawing"] {
        let hwnd = state.app.get_webview_window(label)
            .and_then(|window| window.hwnd().ok()).map(|hwnd| hwnd.0 as isize).unwrap_or(0);
        if hwnd != 0 {
            targets.push((HWND(hwnd), false));
        }
    }
    // WDA_EXCLUDEFROMCAPTURE hides our windows from the capture only; they stay visible on screen.
    let mut excluded = Vec::new();
    let mut display_failed = false;
    for (hwnd, required) in &targets {
        if SetWindowDisplayAffinity(*hwnd, WINDOW_DISPLAY_AFFINITY(0x11)).as_bool() {
            excluded.push(*hwnd);
        } else if *required {
            display_failed = true;
        }
    }
    let restore = |excluded: &[HWND]| {
        for hwnd in excluded {
            SetWindowDisplayAffinity(*hwnd, WINDOW_DISPLAY_AFFINITY(0));
        }
    };
    if display_failed {
        restore(&excluded);
        return Err("Screen capture needs Windows 10 version 2004 or newer".into());
    }
    Sleep(180); // let the compositor apply the exclusion before grabbing
    let result = (|| {
        let surface = DibSurface::new(width, height)?;
        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            surface.free();
            return Err("GetDC failed".to_string());
        }
        let copied = BitBlt(
            surface.hdc(), 0, 0, width, height, screen_dc, rect.left, rect.top,
            ROP_CODE(SRCCOPY.0 | CAPTUREBLT.0),
        );
        ReleaseDC(HWND(0), screen_dc);
        GdiFlush();
        let pixels = if copied.as_bool() { Ok(surface.pixels().to_vec()) } else { Err("BitBlt failed".to_string()) };
        surface.free();
        pixels
    })();
    restore(&excluded);
    result
}

fn color_hex(color: COLORREF) -> String {
    format!("#{:02x}{:02x}{:02x}", color.0 & 0xff, (color.0 >> 8) & 0xff, (color.0 >> 16) & 0xff)
}

fn drawings_to_json(drawings: &[DrawingItem], dx: i32, dy: i32) -> Vec<serde_json::Value> {
    use serde_json::json;
    let pt = |p: &Point| json!([p.x + dx, p.y + dy]);
    drawings.iter().map(|item| match item {
        DrawingItem::Stroke { points, color, width, erase, rotation, group } => json!({
            "type": "stroke", "points": points.iter().map(pt).collect::<Vec<_>>(),
            "color": color_hex(*color), "width": width, "erase": erase, "rotation": rotation, "group": group,
        }),
        DrawingItem::Shape { tool, start, end, color, width, rotation, group } => json!({
            "type": "shape", "tool": tool.as_str(), "start": pt(start), "end": pt(end),
            "color": color_hex(*color), "width": width, "rotation": rotation, "group": group,
        }),
        DrawingItem::Text { start, text, color, width, rotation, group } => json!({
            "type": "text", "start": pt(start), "text": text,
            "color": color_hex(*color), "width": width, "rotation": rotation, "group": group,
        }),
        DrawingItem::Number { center, value, color, width, rotation, group } => json!({
            "type": "number", "center": pt(center), "value": value,
            "color": color_hex(*color), "width": width, "rotation": rotation, "group": group,
        }),
    }).collect()
}

/// `sx`/`sy` rescale stored coordinates when the file was saved on a different monitor size.
fn drawings_from_json(values: &[serde_json::Value], dx: i32, dy: i32, sx: f64, sy: f64) -> Vec<DrawingItem> {
    let point = |value: &serde_json::Value| -> Option<Point> {
        let pair = value.as_array()?;
        Some(Point {
            x: (pair.first()?.as_f64()? * sx).round() as i32 + dx,
            y: (pair.get(1)?.as_f64()? * sy).round() as i32 + dy,
        })
    };
    let color = |value: &serde_json::Value| parse_color(value["color"].as_str().unwrap_or("#ef2b2d"));
    let width = |value: &serde_json::Value| (value["width"].as_i64().unwrap_or(5) as i32).clamp(1, 100);
    let rotation = |value: &serde_json::Value| value["rotation"].as_f64().filter(|r| r.is_finite()).unwrap_or(0.0);
    let group = |value: &serde_json::Value| value["group"].as_u64();
    values.iter().filter_map(|value| match value["type"].as_str()? {
        "stroke" => {
            let points: Vec<Point> = value["points"].as_array()?.iter().filter_map(&point).collect();
            if points.is_empty() { return None; }
            Some(DrawingItem::Stroke {
                points, color: color(value), width: width(value),
                erase: value["erase"].as_bool().unwrap_or(false),
                rotation: rotation(value), group: group(value),
            })
        }
        "shape" => Some(DrawingItem::Shape {
            tool: parse_tool(value["tool"].as_str()?)?,
            start: point(&value["start"])?, end: point(&value["end"])?,
            color: color(value), width: width(value), rotation: rotation(value), group: group(value),
        }),
        "text" => Some(DrawingItem::Text {
            start: point(&value["start"])?, text: value["text"].as_str()?.to_string(),
            color: color(value), width: width(value), rotation: rotation(value), group: group(value),
        }),
        "number" => Some(DrawingItem::Number {
            center: point(&value["center"])?, value: value["value"].as_u64()? as u32,
            color: color(value), width: width(value), rotation: rotation(value), group: group(value),
        }),
        _ => None,
    }).collect()
}

/// Draws every item on a transparent layer exactly like the overlay does and returns the layer.
unsafe fn render_drawings_layer(state: &OverlayState, rect: RECT) -> Result<DibSurface, String> {
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);
    let layer = DibSurface::new(width, height)?;
    layer.pixels().fill(0);
    let mut old_origin = WinPoint::default();
    SetViewportOrgEx(layer.hdc(), state.bounds.left - rect.left, state.bounds.top - rect.top, Some(&mut old_origin));
    SetBkMode(layer.hdc(), TRANSPARENT);
    for drawing in &state.drawings {
        draw_item(layer.hdc(), drawing);
    }
    SetViewportOrgEx(layer.hdc(), old_origin.x, old_origin.y, None);
    GdiFlush();
    Ok(layer)
}

unsafe fn export_screen(state: &mut OverlayState, kind: ExportKind, dir: Option<&str>) -> Result<String, String> {
    if !state.visible {
        return Err("Open the drawing mode before saving".into());
    }
    commit_text_editor(state);
    finish_gesture_history(state);
    let index = target_surface(state).ok_or("No drawing surface is available")?;
    let rect = state.surfaces[index].bounds;
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);

    // Under an opened JSON the frozen image is the "screen"; otherwise grab the real one.
    let base: Vec<u32> = match &state.background {
        Some(background) if same_rect(&background.rect, &rect) => background.pixels.clone(),
        _ => capture_screen(state, rect)?,
    };
    let to_rgba = |pixels: &[u32]| -> Vec<u8> {
        pixels.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8, 255]).collect()
    };
    let directory = crate::app::export_io::export_directory(&state.app, dir);
    let date = today_stamp();
    let path = match kind {
        ExportKind::Png => {
            let layer = render_drawings_layer(state, rect)?;
            let mut composed = base;
            for (target, source) in composed.iter_mut().zip(layer.pixels().iter()) {
                *target = blend_layer_pixel(*target, *source);
            }
            layer.free();
            let png = crate::app::export_io::encode_png(width as u32, height as u32, &to_rgba(&composed))?;
            crate::app::export_io::write_numbered(&directory, &date, "png", &png)?
        }
        ExportKind::Json => {
            let png = crate::app::export_io::encode_png(width as u32, height as u32, &to_rgba(&base))?;
            let document = serde_json::json!({
                "format": crate::app::export_io::JSON_FORMAT,
                "version": crate::app::export_io::JSON_VERSION,
                "monitor": { "width": width, "height": height },
                "image": { "format": "png", "data": crate::app::export_io::png_to_base64(&png) },
                "drawings": drawings_to_json(&state.drawings, state.bounds.left - rect.left, state.bounds.top - rect.top),
            });
            let text = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
            crate::app::export_io::write_numbered(&directory, &date, "json", &text)?
        }
    };
    record_event(format!("Saved screen to {}", path.display()));
    Ok(path.to_string_lossy().into_owned())
}

unsafe fn import_json(state: &mut OverlayState, path: &str) -> Result<String, String> {
    if !state.visible {
        return Err("Open the drawing mode before opening a file".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read file: {e}"))?;
    let document: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {e}"))?;
    if document["format"].as_str() != Some(crate::app::export_io::JSON_FORMAT) {
        return Err("This is not a Keyviz drawing file".into());
    }
    if document["version"].as_u64().unwrap_or(0) > u64::from(crate::app::export_io::JSON_VERSION) {
        return Err("This file was saved by a newer Keyviz version".into());
    }
    commit_text_editor(state);
    finish_gesture_history(state);
    let index = target_surface(state).ok_or("No drawing surface is available")?;
    let rect = state.surfaces[index].bounds;
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);

    let saved_width = document["monitor"]["width"].as_i64().unwrap_or(0).max(1) as f64;
    let saved_height = document["monitor"]["height"].as_i64().unwrap_or(0).max(1) as f64;
    let (sx, sy) = if document["monitor"].is_object() {
        (f64::from(width) / saved_width, f64::from(height) / saved_height)
    } else {
        (1.0, 1.0)
    };

    let background = match document["image"]["data"].as_str() {
        Some(data) => {
            let bytes = crate::app::export_io::base64_to_bytes(data)?;
            let (image_width, image_height, rgba) = crate::app::export_io::decode_png(&bytes)?;
            let mut pixels = Vec::with_capacity(width as usize * height as usize);
            for y in 0..height as usize {
                let source_y = (y * image_height as usize / height as usize).min(image_height as usize - 1);
                for x in 0..width as usize {
                    let source_x = (x * image_width as usize / width as usize).min(image_width as usize - 1);
                    let p = (source_y * image_width as usize + source_x) * 4;
                    let value = (u32::from(rgba[p]) << 16) | (u32::from(rgba[p + 1]) << 8) | u32::from(rgba[p + 2]);
                    pixels.push(opaque_visible(value));
                }
            }
            Some(ScreenBackground { rect, pixels })
        }
        None => None,
    };
    let values = document["drawings"].as_array().cloned().unwrap_or_default();
    // Stored coordinates are monitor-local; convert to union coordinates.
    let mut loaded = drawings_from_json(&values, rect.left - state.bounds.left, rect.top - state.bounds.top, sx, sy);
    if loaded.is_empty() && background.is_none() {
        return Err("The file contains no drawings or image".into());
    }

    let before = state.drawings.clone();
    // Group ids from the file must not collide with groups created later.
    let offset = state.next_group_id;
    for item in &mut loaded {
        if let Some(group) = drawing_group(item) {
            set_drawing_group(item, Some(group.saturating_add(offset)));
        }
    }
    state.drawings = loaded;
    state.next_group_id = state.drawings.iter().filter_map(drawing_group).max().unwrap_or(0).saturating_add(1);
    state.background = background;
    state.active = None;
    state.selected.clear();
    state.selection = None;
    sync_next_number(state);
    record_change(state, before);
    emit_selection_state(state);
    refresh_overlay(state);
    record_event(format!("Opened drawing file {path}"));
    Ok(path.to_string())
}

fn same_rect(a: &RECT, b: &RECT) -> bool {
    a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
}

/// Same rule as the on-screen overlay (`UpdateLayeredWindow` with AC_SRC_ALPHA): layer pixels
/// carry premultiplied alpha, so anti-aliased edges must be blended, not forced opaque
/// (forcing them opaque produced a dark outline around every object).
fn blend_layer_pixel(base: u32, layer: u32) -> u32 {
    let rgb = layer & 0x00ff_ffff;
    let alpha = layer >> 24;
    if rgb == TRANSPARENT_PIXEL || (rgb == 0 && alpha == 0) {
        return base;
    }
    if alpha == 0 || alpha == 255 {
        return layer | 0xff00_0000;
    }
    let channel = |shift: u32| -> u32 {
        let source = (layer >> shift) & 0xff;
        let below = (base >> shift) & 0xff;
        (source + below * (255 - alpha) / 255).min(255)
    };
    0xff00_0000 | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

#[cfg(test)]
mod export_blend_tests {
    use super::*;

    #[test]
    fn blend_keeps_edges_soft_instead_of_black() {
        let base = 0x00ff_ffff;
        assert_eq!(blend_layer_pixel(base, 0), base);
        assert_eq!(blend_layer_pixel(base, 0xffef_2b2d), 0xffef_2b2d);
        // Half-transparent premultiplied red over white stays light, never near black.
        let edge = blend_layer_pixel(base, 0x8077_1516);
        assert!((edge >> 16) & 0xff > 0xc0);
    }
}
