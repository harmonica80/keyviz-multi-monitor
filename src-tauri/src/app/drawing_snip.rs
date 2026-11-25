// Snipping tool support (scissors + pin button). Included into `platform` (see native_drawing.rs).
//
// Freezes the monitor under the toolbar - the real screen plus the drawings, and the screen
// alone for JSON files - and hands the pixels to `crate::app::snip`, which owns the selection
// window, cropping, saving, copying and the pinned notes.

/// Date stamp used for the file names of snipped screenshots (same scheme as the screen saves).
pub fn snip_date() -> String {
    today_stamp()
}

unsafe fn snip_capture(state: &mut OverlayState) -> Result<(i32, i32, u32, u32), String> {
    if !state.visible {
        return Err("Open the drawing mode before taking a screenshot".into());
    }
    if state.zoom.is_some() {
        return Err("Leave the magnifier before taking a screenshot".into());
    }
    commit_text_editor(state);
    finish_gesture_history(state);
    let rect = toolbar_monitor(state).ok_or("No drawing surface is available")?;
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);

    // Under an opened JSON the frozen image is the "screen"; otherwise grab the real one.
    let raw: Vec<u32> = match &state.background {
        Some(background) if same_rect(&background.rect, &rect) => background.pixels.clone(),
        _ => capture_screen(state, rect, true)?,
    };
    let layer = render_drawings_layer(state, rect)?;
    let mut composite = raw.clone();
    for (target, source) in composite.iter_mut().zip(layer.pixels().iter()) {
        *target = blend_layer_pixel(*target, *source);
    }
    layer.free();

    // Drawn objects in monitor-local coordinates, serialised once for the JSON files.
    let (ox, oy) = (state.bounds.left - rect.left, state.bounds.top - rect.top);
    let items = state.drawings.iter().filter_map(|item| {
        let bounds = drawing_bounds(item);
        let json = drawings_to_json(std::slice::from_ref(item), ox, oy).into_iter().next()?;
        Some(crate::app::snip::SnipItem {
            left: bounds.left + ox,
            top: bounds.top + oy,
            right: bounds.right + ox,
            bottom: bounds.bottom + oy,
            json,
        })
    }).collect();

    crate::app::snip::set_session(crate::app::snip::SnipSession {
        left: rect.left,
        top: rect.top,
        width: width as u32,
        height: height as u32,
        composite,
        raw,
        items,
        dir: None,
    });
    record_event(format!("Screenshot started on monitor ({},{},{},{})", rect.left, rect.top, rect.right, rect.bottom));
    Ok((rect.left, rect.top, width as u32, height as u32))
}
