//! Platform-independent math for the screen magnifier (ZoomIt-style zoom).
//!
//! The magnified view keeps the screen point under the cursor fixed: with zoom
//! level `z` and the cursor at `c` (monitor-local), the visible source window
//! starts at `c * (1 - 1/z)`. Moving the cursor to an edge therefore reveals
//! that edge, and changing the level zooms toward / away from the cursor.

pub const ZOOM_START: f64 = 2.0;
pub const ZOOM_MIN: f64 = 1.0;
pub const ZOOM_MAX: f64 = 8.0;
pub const ZOOM_STEP: f64 = 1.25;

/// Next zoom level after one wheel notch (`steps > 0` zooms in).
pub fn next_zoom_level(level: f64, steps: i32) -> f64 {
    let next = level * ZOOM_STEP.powi(steps);
    // Snap to 1x so zooming all the way out shows the plain screen exactly.
    if next < ZOOM_MIN + 0.01 {
        ZOOM_MIN
    } else {
        next.min(ZOOM_MAX)
    }
}

/// Linear blend of two 0x??RRGGBB pixels; `weight` is the share of `q` in 0..=256.
#[inline]
fn lerp_pixel(p: u32, q: u32, weight: u32) -> u32 {
    let keep = 256 - weight;
    let red_blue = (((p & 0x00ff_00ff) * keep + (q & 0x00ff_00ff) * weight) >> 8) & 0x00ff_00ff;
    let green = (((p & 0x0000_ff00) * keep + (q & 0x0000_ff00) * weight) >> 8) & 0x0000_ff00;
    red_blue | green
}

/// For each destination index along one axis: (source index, next source index, weight of next).
fn axis_samples(size: usize, cursor: f64, level: f64) -> Vec<(usize, usize, u32)> {
    let last = size.saturating_sub(1);
    // Pixel centres: destination pixel `cursor` samples exactly source pixel `cursor`.
    let start = (cursor.clamp(0.0, last as f64) + 0.5) * (1.0 - 1.0 / level);
    (0..size)
        .map(|index| {
            let source = (start + (index as f64 + 0.5) / level - 0.5).clamp(0.0, last as f64);
            let first = source.floor() as usize;
            let weight = ((source - first as f64) * 256.0) as u32;
            (first, (first + 1).min(last), weight.min(256))
        })
        .collect()
}

/// Renders the magnified view of `source` (a `width` x `height` opaque snapshot)
/// into `dest` (same size). Output pixels are fully opaque (alpha 0xff).
pub fn render_zoom(
    source: &[u32],
    width: usize,
    height: usize,
    cursor_x: f64,
    cursor_y: f64,
    level: f64,
    dest: &mut [u32],
) {
    let count = width.saturating_mul(height);
    if count == 0 || source.len() < count || dest.len() < count {
        return;
    }
    let level = level.clamp(ZOOM_MIN, ZOOM_MAX);
    let columns = axis_samples(width, cursor_x, level);
    let rows = axis_samples(height, cursor_y, level);
    for (y, &(row0, row1, row_weight)) in rows.iter().enumerate() {
        let upper = &source[row0 * width..row0 * width + width];
        let lower = &source[row1 * width..row1 * width + width];
        let out = &mut dest[y * width..y * width + width];
        for (x, &(col0, col1, col_weight)) in columns.iter().enumerate() {
            let top = lerp_pixel(upper[col0], upper[col1], col_weight);
            let bottom = lerp_pixel(lower[col0], lower[col1], col_weight);
            out[x] = 0xff00_0000 | lerp_pixel(top, bottom, row_weight);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_steps_are_clamped_and_snap_to_one() {
        assert_eq!(next_zoom_level(ZOOM_START, 1), 2.5);
        assert_eq!(next_zoom_level(ZOOM_MAX, 1), ZOOM_MAX);
        assert_eq!(next_zoom_level(1.2, -1), ZOOM_MIN);
        assert_eq!(next_zoom_level(ZOOM_MIN, -1), ZOOM_MIN);
    }

    #[test]
    fn level_one_is_identity() {
        let source: Vec<u32> = (0..12).map(|i| 0x0010_2030 * i).collect();
        let mut dest = vec![0; 12];
        render_zoom(&source, 4, 3, 2.0, 1.0, 1.0, &mut dest);
        for (out, expected) in dest.iter().zip(&source) {
            assert_eq!(*out, 0xff00_0000 | (expected & 0x00ff_ffff));
        }
    }

    #[test]
    fn point_under_cursor_stays_in_place() {
        // 100x1 gradient; zoomed 4x around x=70, the pixel at x=70 shows source x=70.
        let source: Vec<u32> = (0..100u32).map(|x| x * 2).collect();
        let mut dest = vec![0; 100];
        render_zoom(&source, 100, 1, 70.0, 0.0, 4.0, &mut dest);
        let value = dest[70] & 0xff;
        assert_eq!(value, 140);
        // Neighbours advance 4x slower than the source.
        let left = dest[66] & 0xff;
        assert!((137..=139).contains(&left), "got {left}");
    }

    #[test]
    fn edges_are_reachable() {
        let source: Vec<u32> = (0..100u32).map(|x| x * 2).collect();
        let mut dest = vec![0; 100];
        render_zoom(&source, 100, 1, 99.0, 0.0, 2.0, &mut dest);
        assert!(dest[99] & 0xff >= 196);
        render_zoom(&source, 100, 1, 0.0, 0.0, 2.0, &mut dest);
        assert_eq!(dest[0] & 0xff, 0);
    }

    #[test]
    fn blend_is_exact_at_endpoints() {
        assert_eq!(lerp_pixel(0x00ff_8040, 0x0000_0000, 0), 0x00ff_8040);
        assert_eq!(lerp_pixel(0x0000_0000, 0x00ff_8040, 256), 0x00ff_8040);
    }
}
