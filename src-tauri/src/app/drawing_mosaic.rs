// Mosaic tool (pixelates what is under the dragged rectangle). Included into `platform`.
//
// The desktop under the overlay cannot be read back from the layered window, so the first mosaic
// of a drawing session freezes the screen (without our own windows) once; every mosaic object
// pixelates that frozen picture at its current rectangle, so it can be moved and resized later.

struct MosaicSource {
    /// Top-left of the frozen monitor in drawing coordinates (relative to `OverlayState::bounds`).
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    /// 0x??RRGGBB pixels.
    pixels: Vec<u32>,
}

static MOSAIC_SOURCES: Mutex<Vec<MosaicSource>> = Mutex::new(Vec::new());

fn mosaic_clear() {
    if let Ok(mut sources) = MOSAIC_SOURCES.lock() {
        sources.clear();
    }
}

/// Block edge in pixels: follows the tool width (1..15), never too fine to be a mosaic.
fn mosaic_block_size(width: i32) -> i32 {
    (width.max(1) * 3).clamp(6, 45)
}

/// Mean colour of a block as a GDI colour that never collides with the transparent key.
fn mosaic_block_color(source: &MosaicSource, x0: i32, y0: i32, x1: i32, y1: i32) -> COLORREF {
    let (mut red, mut green, mut blue, mut count) = (0u64, 0u64, 0u64, 0u64);
    for y in y0..y1 {
        let row = (y * source.width) as usize;
        for x in x0..x1 {
            if let Some(pixel) = source.pixels.get(row + x as usize) {
                red += ((*pixel >> 16) & 0xff) as u64;
                green += ((*pixel >> 8) & 0xff) as u64;
                blue += (*pixel & 0xff) as u64;
                count += 1;
            }
        }
    }
    if count == 0 {
        return COLORREF(0x0001_0101);
    }
    let (r, g, b) = ((red / count) as u32, (green / count) as u32, (blue / count) as u32);
    let mut dib = (r << 16) | (g << 8) | b;
    if dib == 0 || dib == TRANSPARENT_PIXEL {
        dib = 0x0001_0101;
    }
    COLORREF(((dib & 0xff) << 16) | (dib & 0xff00) | ((dib >> 16) & 0xff))
}

/// Freezes the monitor under `point` (drawing coordinates) unless it is already frozen.
unsafe fn mosaic_prepare(state: &OverlayState, point: Point) {
    let has_mosaic = state
        .drawings
        .iter()
        .any(|item| matches!(item, DrawingItem::Shape { tool: NativeTool::Mosaic, .. }));
    if !has_mosaic {
        mosaic_clear();
    }
    let screen = Point { x: point.x + state.bounds.left, y: point.y + state.bounds.top };
    let Some(surface) = state.surfaces.iter().find(|s| {
        screen.x >= s.bounds.left && screen.x < s.bounds.right
            && screen.y >= s.bounds.top && screen.y < s.bounds.bottom
    }) else {
        return;
    };
    let left = surface.bounds.left - state.bounds.left;
    let top = surface.bounds.top - state.bounds.top;
    if let Ok(sources) = MOSAIC_SOURCES.lock() {
        if sources.iter().any(|source| source.left == left && source.top == top) {
            return;
        }
    }
    match capture_screen(state, surface.bounds, true) {
        Ok(pixels) => {
            if let Ok(mut sources) = MOSAIC_SOURCES.lock() {
                sources.push(MosaicSource {
                    left,
                    top,
                    width: (surface.bounds.right - surface.bounds.left).max(1),
                    height: (surface.bounds.bottom - surface.bounds.top).max(1),
                    pixels,
                });
            }
        }
        Err(error) => record_error(format!("Mosaic capture failed: {error}")),
    }
}

unsafe fn draw_mosaic(dc: HDC, start: Point, end: Point, width: i32) {
    use windows::Win32::Graphics::Gdi::{FillRect, IntersectClipRect, RestoreDC, SaveDC};

    let (left, right) = (start.x.min(end.x), start.x.max(end.x));
    let (top, bottom) = (start.y.min(end.y), start.y.max(end.y));
    if right - left < 2 || bottom - top < 2 {
        return;
    }
    let block = mosaic_block_size(width);
    let Ok(sources) = MOSAIC_SOURCES.lock() else {
        return;
    };
    let saved = SaveDC(dc);
    IntersectClipRect(dc, left, top, right, bottom);
    for source in sources.iter() {
        let x_from = left.max(source.left) - source.left;
        let x_to = right.min(source.left + source.width) - source.left;
        let y_from = top.max(source.top) - source.top;
        let y_to = bottom.min(source.top + source.height) - source.top;
        if x_to <= x_from || y_to <= y_from {
            continue;
        }
        let mut by = y_from / block * block;
        while by < y_to {
            let mut bx = x_from / block * block;
            while bx < x_to {
                let color = mosaic_block_color(
                    source,
                    bx,
                    by,
                    (bx + block).min(source.width),
                    (by + block).min(source.height),
                );
                let brush = CreateSolidBrush(color);
                let rect = RECT {
                    left: source.left + bx,
                    top: source.top + by,
                    right: source.left + bx + block,
                    bottom: source.top + by + block,
                };
                FillRect(dc, &rect, brush);
                DeleteObject(brush);
                bx += block;
            }
            by += block;
        }
    }
    RestoreDC(dc, saved);
}
