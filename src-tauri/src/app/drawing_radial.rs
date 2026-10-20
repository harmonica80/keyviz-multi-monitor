// Included in native_drawing::platform: the menu shares the existing display/input surfaces.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

const RADIAL_TOOLS: [NativeTool; 12] = [
    NativeTool::Pen,
    NativeTool::Arrow,
    NativeTool::Number,
    NativeTool::Eraser,
    NativeTool::Rectangle,
    NativeTool::Ellipse,
    NativeTool::CheckMark,
    NativeTool::CrossMark,
    NativeTool::Text,
    NativeTool::Line,
    NativeTool::Select,
    NativeTool::Pointer,
];
const RADIAL_RADIUS: f64 = 78.0;
static RADIAL_OPEN: AtomicBool = AtomicBool::new(false);
static RADIAL_ESCAPE_UNTIL: AtomicU64 = AtomicU64::new(0);

fn radial_clock() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

pub fn radial_consumes_close() -> bool {
    RADIAL_OPEN.load(Ordering::Relaxed)
        || radial_clock() < RADIAL_ESCAPE_UNTIL.load(Ordering::Relaxed)
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct RadialShortcut {
    key: u32,
    modifiers: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RadialKeyAction {
    Toggle,
    Cancel,
}

fn radial_key_action(
    shortcut: RadialShortcut,
    open: bool,
    key: u32,
    modifiers: u8,
    repeated: bool,
    left_down: bool,
) -> Option<RadialKeyAction> {
    if repeated {
        None
    } else if open && key == 0x1b {
        Some(RadialKeyAction::Cancel)
    } else if !left_down
        && shortcut.key != 0
        && key == shortcut.key
        && modifiers == shortcut.modifiers
    {
        Some(RadialKeyAction::Toggle)
    } else {
        None
    }
}

fn radial_shortcut(keys: &[String]) -> Result<RadialShortcut, String> {
    let mut shortcut = RadialShortcut::default();
    for key in keys {
        let modifier = match key.as_str() {
            "Control" | "ControlLeft" | "ControlRight" => 1,
            "Shift" | "ShiftLeft" | "ShiftRight" => 2,
            "Alt" | "AltLeft" | "AltRight" => 4,
            _ => 0,
        };
        if modifier != 0 {
            shortcut.modifiers |= modifier;
            continue;
        }
        let vk = if key.starts_with("Key")
            && key.len() == 4
            && key.as_bytes()[3].is_ascii_alphabetic()
        {
            key.as_bytes()[3].to_ascii_uppercase() as u32
        } else if key.starts_with("Num") && key.len() == 4 && key.as_bytes()[3].is_ascii_digit() {
            key.as_bytes()[3] as u32
        } else if let Some(number) = key
            .strip_prefix('F')
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|n| (1..=24).contains(n))
        {
            0x6f + number
        } else {
            match key.as_str() {
                "Space" | " " => 0x20,
                "Tab" => 0x09,
                "Minus" => 0xbd,
                "Equal" => 0xbb,
                "LeftBracket" => 0xdb,
                "RightBracket" => 0xdd,
                "BackSlash" => 0xdc,
                "SemiColon" => 0xba,
                "Quote" => 0xde,
                "Comma" => 0xbc,
                "Dot" => 0xbe,
                "Slash" => 0xbf,
                "BackQuote" => 0xc0,
                _ => return Err("Unsupported radial menu shortcut".into()),
            }
        };
        if shortcut.key != 0 {
            return Err("Use one key with optional Ctrl, Shift or Alt".into());
        }
        shortcut.key = vk;
    }
    if !keys.is_empty() && shortcut.key == 0 {
        return Err("Choose a non-modifier key".into());
    }
    if shortcut.modifiers & 4 != 0 && [0x09, 0x73].contains(&shortcut.key) {
        return Err("This shortcut is reserved by Windows".into());
    }
    Ok(shortcut)
}

#[derive(Clone)]
struct RadialGate {
    enabled: bool,
    open: bool,
    shortcut: RadialShortcut,
    settings: isize,
}

impl Default for RadialGate {
    fn default() -> Self {
        Self {
            enabled: false,
            open: false,
            shortcut: RadialShortcut {
                key: 0x51,
                modifiers: 0,
            },
            settings: 0,
        }
    }
}

// The hook only reads a small snapshot and queues work. It never paints or waits
// for the drawing/AppState locks, even while a large monitor is being redrawn.
mod radial_keyboard {
    use super::*;
    use std::cell::RefCell;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetForegroundWindow, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
        GA_ROOTOWNER, HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };
    struct HookState {
        gate: Arc<Mutex<RadialGate>>,
        sender: Sender<DrawingCommand>,
        down: [bool; 256],
        swallowed: [bool; 256],
    }
    thread_local! { static HOOK: RefCell<Option<HookState>> = const { RefCell::new(None) }; }

    pub(super) fn start(gate: Arc<Mutex<RadialGate>>, sender: Sender<DrawingCommand>) {
        thread::spawn(move || unsafe {
            let mut down = [false; 256];
            for key in [
                0x10, 0x11, 0x12, 0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0x5b, 0x5c,
            ] {
                down[key] = GetAsyncKeyState(key as i32) < 0;
            }
            HOOK.with(|slot| {
                *slot.borrow_mut() = Some(HookState {
                    gate,
                    sender,
                    down,
                    swallowed: [false; 256],
                })
            });
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(callback),
                GetModuleHandleW(None).unwrap_or_default(),
                0,
            );
            let Ok(hook) = hook else {
                record_error("Failed to install radial menu keyboard hook");
                return;
            };
            let mut message = MSG::default();
            while GetMessageW(&mut message, HWND(0), 0, 0).0 > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            let _ = UnhookWindowsHookEx(hook);
            HOOK.with(|slot| *slot.borrow_mut() = None);
        });
    }

    unsafe extern "system" fn callback(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code < 0 {
            return CallNextHookEx(HHOOK(0), code, wparam, lparam);
        }
        let event = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = event.vkCode as usize;
        let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        let up = matches!(wparam.0 as u32, WM_KEYUP | WM_SYSKEYUP);
        if vk >= 256 || (!down && !up) {
            return CallNextHookEx(HHOOK(0), code, wparam, lparam);
        }
        let consumed = HOOK.with(|slot| {
            let mut slot = slot.borrow_mut();
            let Some(hook) = slot.as_mut() else {
                return false;
            };
            let repeated = hook.down[vk];
            hook.down[vk] = down;
            if hook.swallowed[vk] {
                if vk == 0x1b {
                    RADIAL_ESCAPE_UNTIL.store(radial_clock() + 150, Ordering::Relaxed);
                }
                if up {
                    hook.swallowed[vk] = false;
                }
                return true;
            }
            if !down || repeated {
                return false;
            }
            let Ok(gate) = hook.gate.try_lock() else {
                return false;
            };
            if !gate.enabled {
                return false;
            }
            let foreground = GetAncestor(GetForegroundWindow(), GA_ROOTOWNER);
            if gate.settings != 0 && foreground.0 == gate.settings {
                return false;
            }
            let modifiers = (u8::from(hook.down[0x11] || hook.down[0xa2] || hook.down[0xa3]))
                | (u8::from(hook.down[0x10] || hook.down[0xa0] || hook.down[0xa1]) << 1)
                | (u8::from(hook.down[0x12] || hook.down[0xa4] || hook.down[0xa5]) << 2)
                | (u8::from(hook.down[0x5b] || hook.down[0x5c]) << 3);
            let Some(action) = radial_key_action(
                gate.shortcut,
                gate.open,
                vk as u32,
                modifiers,
                repeated,
                GetAsyncKeyState(1) < 0,
            ) else {
                return false;
            };
            let command = match action {
                RadialKeyAction::Cancel => {
                    RADIAL_ESCAPE_UNTIL.store(radial_clock() + 150, Ordering::Relaxed);
                    DrawingCommand::CancelRadial
                }
                RadialKeyAction::Toggle => DrawingCommand::ToggleRadial,
            };
            drop(gate);
            if hook.sender.send(command).is_err() {
                return false;
            }
            hook.swallowed[vk] = true;
            true
        });
        if consumed {
            LRESULT(1)
        } else {
            CallNextHookEx(HHOOK(0), code, wparam, lparam)
        }
    }
}

struct RadialMenu {
    center: Point,
    radius: f64,
    hovered: Option<usize>,
}

impl RadialMenu {
    fn hit(&self, point: Point) -> Option<usize> {
        let x = (point.x - self.center.x) as f64;
        let y = (point.y - self.center.y) as f64;
        let r = x.hypot(y) / self.radius;
        if !(0.22..=1.0).contains(&r) {
            return None;
        }
        let sector = ((y.atan2(x) + std::f64::consts::PI * 2.0 / 3.0)
            .rem_euclid(std::f64::consts::TAU)
            / (std::f64::consts::PI / 3.0))
            .floor() as usize;
        Some(sector.min(5) + if r < 0.61 { 0 } else { 6 })
    }
}

fn radial_available(visible: bool, click_through: bool, editing: bool, probing: bool) -> bool {
    visible && !click_through && !editing && !probing
}

fn radial_right_action(
    open: bool,
    enabled: bool,
    available: bool,
    left_down: bool,
) -> Option<RadialKeyAction> {
    if open {
        Some(RadialKeyAction::Cancel)
    } else if enabled && available && !left_down {
        Some(RadialKeyAction::Toggle)
    } else {
        None
    }
}

unsafe fn radial_right_click(state: &mut OverlayState) {
    let action = radial_right_action(
        state.radial.is_some(),
        state.radial_right_click,
        radial_available(
            state.visible,
            state.click_through,
            state.edit.is_some(),
            state.probe.is_some(),
        ),
        GetAsyncKeyState(1) < 0,
    );
    match action {
        Some(RadialKeyAction::Toggle) => toggle_radial(state),
        Some(RadialKeyAction::Cancel) => cancel_radial(state),
        None => {}
    }
}

fn sync_radial_gate(state: &OverlayState) {
    if let Ok(mut gate) = state.radial_gate.lock() {
        // Mouse-button state blocks live gestures. A missed mouse-up must not
        // permanently disable the shortcut via a stale active/selection value.
        gate.enabled = radial_available(
            state.visible,
            state.click_through,
            state.edit.is_some(),
            state.probe.is_some(),
        );
        gate.open = state.radial.is_some();
    }
    RADIAL_OPEN.store(state.radial.is_some(), Ordering::Relaxed);
}

// Keys consumed by the global hook never arrive here. Handling the window's
// own key messages also keeps Q working after drawing gives this window focus,
// including when Windows has stopped delivering low-level hook callbacks.
unsafe fn radial_window_key(state: &mut OverlayState, key: u32, lparam: LPARAM) -> bool {
    if !radial_available(
        state.visible,
        state.click_through,
        state.edit.is_some(),
        state.probe.is_some(),
    ) {
        return false;
    }
    let shortcut = match state.radial_gate.lock() {
        Ok(gate) => gate.shortcut,
        Err(_) => return false,
    };
    let key_down = |key| windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(key) < 0;
    let modifiers = u8::from(key_down(0x11))
        | (u8::from(key_down(0x10)) << 1)
        | (u8::from(key_down(0x12)) << 2)
        | (u8::from(key_down(0x5b) || key_down(0x5c)) << 3);
    let Some(action) = radial_key_action(
        shortcut,
        state.radial.is_some(),
        key,
        modifiers,
        lparam.0 & (1 << 30) != 0,
        GetAsyncKeyState(1) < 0,
    ) else {
        return false;
    };
    match action {
        RadialKeyAction::Toggle => toggle_radial(state),
        RadialKeyAction::Cancel => {
            RADIAL_ESCAPE_UNTIL.store(radial_clock() + 150, Ordering::Relaxed);
            cancel_radial(state);
        }
    }
    true
}

unsafe fn cancel_radial(state: &mut OverlayState) {
    if state.radial.take().is_some() {
        sync_radial_gate(state);
        replace_tool_cursor(state, true);
        refresh_overlay(state);
    }
}

fn radial_center(cursor: Point, bounds: RECT, radius: f64) -> Point {
    let inset = radius.ceil() as i32 + 4;
    Point {
        x: cursor.x.clamp(
            bounds.left + inset,
            (bounds.right - inset).max(bounds.left + inset),
        ),
        y: cursor.y.clamp(
            bounds.top + inset,
            (bounds.bottom - inset).max(bounds.top + inset),
        ),
    }
}

unsafe fn toggle_radial(state: &mut OverlayState) {
    if state.radial.is_some() {
        cancel_radial(state);
        return;
    }
    if !radial_available(
        state.visible,
        state.click_through,
        state.edit.is_some(),
        state.probe.is_some(),
    ) || GetAsyncKeyState(1) < 0
    {
        return;
    }
    if state.active.is_some() || state.selection.is_some() {
        // Finish, rather than discard, a gesture whose button-up was lost.
        finish_drawing_at(state, Point { x: 0, y: 0 });
    }
    let mut cursor = WinPoint::default();
    if !windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor).as_bool() {
        return;
    }
    let Some(surface) = state.surfaces.iter().find(|s| {
        s.display_ready
            && cursor.x >= s.bounds.left
            && cursor.x < s.bounds.right
            && cursor.y >= s.bounds.top
            && cursor.y < s.bounds.bottom
    }) else {
        return;
    };
    let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(surface.input_hwnd).max(96);
    let radius = (RADIAL_RADIUS * dpi as f64 / 96.0).min(
        ((surface.bounds.right - surface.bounds.left)
            .min(surface.bounds.bottom - surface.bounds.top)
            - 12)
            .max(20) as f64
            / 2.0,
    );
    let mut center = radial_center(
        Point {
            x: cursor.x,
            y: cursor.y,
        },
        surface.bounds,
        radius,
    );
    if let Some(toolbar) = state.toolbar_passthrough {
        let r = radius.ceil() as i32 + 4;
        if center.x + r > toolbar.left
            && center.x - r < toolbar.right
            && center.y + r > toolbar.top
            && center.y - r < toolbar.bottom
        {
            if toolbar.left - r >= surface.bounds.left + r {
                center.x = toolbar.left - r;
            } else if toolbar.right + r < surface.bounds.right - r {
                center.x = toolbar.right + r;
            }
        }
    }
    state.radial = Some(RadialMenu {
        center: Point {
            x: center.x - state.bounds.left,
            y: center.y - state.bounds.top,
        },
        radius,
        hovered: None,
    });
    state.selected.clear();
    state.last_wheel = None;
    ReleaseCapture();
    SetCursor(LoadCursorW(None, IDC_ARROW).unwrap_or_default());
    refresh_overlay(state);
}

unsafe fn radial_click(state: &mut OverlayState, point: Point) -> bool {
    let Some(menu) = state.radial.as_ref() else {
        return false;
    };
    let tool = menu
        .hit(point)
        .map(|index| RADIAL_TOOLS[index])
        .filter(|tool| state.radial_tools.contains(tool));
    cancel_radial(state);
    state.radial_ignore_up = true;
    if let Some(tool) = tool {
        let app = state.app.clone();
        thread::spawn(move || {
            if let Err(error) =
                crate::app::commands::drawing_set_tool(app.clone(), tool.as_str().into())
            {
                record_error(format!("Radial tool selection failed: {error}"));
            } else {
                let _ = app.emit(
                    "drawing-tool-changed",
                    serde_json::json!({ "tool": tool.as_str() }),
                );
            }
        });
    }
    true
}

unsafe fn draw_radial(dc: HDC, menu: &RadialMenu, tools: &[NativeTool]) {
    with_gdiplus_graphics(dc, false, |graphics| {
        let fill = |points: &[WinPoint], color| {
            let mut brush = std::ptr::null_mut();
            if GdipCreateSolidFill(color, &mut brush) == 0 {
                GdipFillPolygonI(graphics, brush, points.as_ptr(), points.len() as i32, 0);
                GdipDeleteBrush(brush);
            }
        };
        let point = |angle: f64, radius: f64| WinPoint {
            x: menu.center.x + (angle.cos() * radius).round() as i32,
            y: menu.center.y + (angle.sin() * radius).round() as i32,
        };
        if let Some(pen) = create_gdiplus_pen(0xffc4cad2, 1) {
            for index in 0..12 {
                let inner = menu.radius * if index < 6 { 0.22 } else { 0.61 };
                let outer = menu.radius * if index < 6 { 0.61 } else { 1.0 };
                let angle = (index % 6) as f64 * std::f64::consts::PI / 3.0
                    - std::f64::consts::PI * 2.0 / 3.0;
                let points: Vec<WinPoint> = (0..=20)
                    .map(|step| point(angle + step as f64 * std::f64::consts::PI / 60.0, outer))
                    .chain((0..=20).rev().map(|step| {
                        point(angle + step as f64 * std::f64::consts::PI / 60.0, inner)
                    }))
                    .collect();
                let enabled = tools.contains(&RADIAL_TOOLS[index]);
                fill(
                    &points,
                    if menu.hovered == Some(index) && enabled {
                        0xffdcecff
                    } else {
                        0xfffafbfd
                    },
                );
                GdipDrawPolygonI(graphics, pen, points.as_ptr(), points.len() as i32);
            }
            GdipDeletePen(pen);
        }
        let hub: Vec<WinPoint> = (0..96)
            .map(|i| point(i as f64 * std::f64::consts::TAU / 96.0, menu.radius * 0.22))
            .collect();
        fill(&hub, 0xffedf0f4);
        true
    });
    for (index, tool) in RADIAL_TOOLS.iter().enumerate() {
        if !tools.contains(tool) {
            continue;
        }
        let angle = (index % 6) as f64 * std::f64::consts::PI / 3.0 - std::f64::consts::FRAC_PI_2;
        let r = menu.radius * if index < 6 { 0.415 } else { 0.805 };
        let center = Point {
            x: menu.center.x + (r * angle.cos()).round() as i32,
            y: menu.center.y + (r * angle.sin()).round() as i32,
        };
        draw_radial_icon(
            dc,
            *tool,
            center,
            menu.radius / 156.0,
            menu.hovered == Some(index),
        );
    }
    // The hub is cancellation, not another tool.
    draw_radial_icon(
        dc,
        NativeTool::CrossMark,
        menu.center,
        menu.radius / 240.0,
        false,
    );
}

unsafe fn draw_radial_icon(dc: HDC, tool: NativeTool, center: Point, scale: f64, hovered: bool) {
    let color = if hovered {
        COLORREF(0xd87b16)
    } else {
        COLORREF(0x38332e)
    };
    let p = |x: f64, y: f64| Point {
        x: center.x + (x * scale).round() as i32,
        y: center.y + (y * scale).round() as i32,
    };
    let width = (2.0 * scale).round().max(1.0) as i32;
    let line = |a, b| {
        draw_antialiased_shape(dc, &NativeTool::Line, a, b, color, width, 0.0);
    };
    match tool {
        NativeTool::Rectangle
        | NativeTool::Ellipse
        | NativeTool::CheckMark
        | NativeTool::CrossMark => {
            draw_antialiased_shape(dc, &tool, p(-12.0, -11.0), p(12.0, 11.0), color, width, 0.0);
        }
        NativeTool::Line => line(p(-12.0, 10.0), p(12.0, -10.0)),
        NativeTool::Arrow => {
            line(p(-11.0, 11.0), p(11.0, -11.0));
            line(p(0.0, -11.0), p(11.0, -11.0));
            line(p(11.0, -11.0), p(11.0, 0.0));
        }
        NativeTool::Text => {
            line(p(-11.0, -12.0), p(11.0, -12.0));
            line(p(0.0, -12.0), p(0.0, 12.0));
            line(p(-6.0, 12.0), p(6.0, 12.0));
        }
        NativeTool::Number => {
            draw_antialiased_shape(
                dc,
                &NativeTool::Ellipse,
                p(-14.0, -14.0),
                p(14.0, 14.0),
                color,
                width,
                0.0,
            );
            line(p(-3.0, -5.0), p(1.0, -8.0));
            line(p(1.0, -8.0), p(1.0, 8.0));
            line(p(-3.0, 8.0), p(5.0, 8.0));
        }
        NativeTool::Select => {
            for i in [-12.0, -4.0, 4.0] {
                line(p(i, -12.0), p(i + 4.0, -12.0));
                line(p(i, 12.0), p(i + 4.0, 12.0));
                line(p(-12.0, i), p(-12.0, i + 4.0));
                line(p(12.0, i), p(12.0, i + 4.0));
            }
        }
        _ => {
            let points: &[(f64, f64)] = match tool {
                NativeTool::Pen => &[
                    (-12.0, 12.0),
                    (-9.0, 3.0),
                    (6.0, -12.0),
                    (12.0, -6.0),
                    (-3.0, 9.0),
                    (-12.0, 12.0),
                ],
                NativeTool::Eraser => &[
                    (-13.0, 3.0),
                    (2.0, -12.0),
                    (13.0, -1.0),
                    (0.0, 12.0),
                    (-4.0, 12.0),
                    (-13.0, 3.0),
                ],
                _ => &[
                    (-9.0, -13.0),
                    (-9.0, 11.0),
                    (-3.0, 5.0),
                    (3.0, 14.0),
                    (7.0, 11.0),
                    (1.0, 2.0),
                    (10.0, 2.0),
                    (-9.0, -13.0),
                ],
            };
            for pair in points.windows(2) {
                line(p(pair[0].0, pair[0].1), p(pair[1].0, pair[1].1));
            }
            if matches!(tool, NativeTool::Pen) {
                line(p(3.0, -9.0), p(9.0, -3.0));
            }
            if matches!(tool, NativeTool::Eraser) {
                line(p(-8.0, -2.0), p(3.0, 9.0));
            }
        }
    }
}

#[cfg(test)]
mod radial_hover_tests {
    use super::*;
    #[test]
    fn radial_menu_renders_without_filling_the_rest_of_the_canvas() {
        unsafe {
            let mut surface = OverlaySurface {
                display_hwnd: HWND(0),
                input_hwnd: HWND(0),
                bounds: RECT {
                    left: 0,
                    top: 0,
                    right: 360,
                    bottom: 360,
                },
                canvas: None,
                display_ready: false,
                last_update_error: None,
                frame_stats: None,
            };
            assert!(ensure_overlay_canvas(&mut surface));
            let canvas = surface.canvas.as_ref().unwrap();
            let pixels = std::slice::from_raw_parts_mut(canvas.bits as *mut u32, 360 * 360);
            pixels.fill(0);
            draw_radial(
                HDC(canvas.memory_dc.0),
                &RadialMenu {
                    center: Point { x: 180, y: 180 },
                    radius: RADIAL_RADIUS,
                    hovered: Some(1),
                },
                &RADIAL_TOOLS,
            );
            assert!(GdiFlush().as_bool());
            assert_eq!(pixels[0], 0);
            assert_ne!(pixels[180 * 360 + 185], 0);
            let painted = pixels.iter().filter(|pixel| **pixel != 0).count();
            assert!((17_000..21_000).contains(&painted));
            assert_eq!(pixels[180 * 360 + 270], 0);
            if let Ok(path) = std::env::var("KEYVIZ_RADIAL_PREVIEW") {
                let bytes = std::slice::from_raw_parts(canvas.bits as *const u8, 360 * 360 * 4);
                let mut bmp = Vec::new();
                bmp.extend_from_slice(b"BM");
                bmp.extend_from_slice(&(54u32 + bytes.len() as u32).to_le_bytes());
                bmp.extend_from_slice(&[0; 4]);
                bmp.extend_from_slice(&54u32.to_le_bytes());
                bmp.extend_from_slice(&40u32.to_le_bytes());
                bmp.extend_from_slice(&360i32.to_le_bytes());
                bmp.extend_from_slice(&(-360i32).to_le_bytes());
                bmp.extend_from_slice(&1u16.to_le_bytes());
                bmp.extend_from_slice(&32u16.to_le_bytes());
                bmp.extend_from_slice(&[0; 24]);
                bmp.extend_from_slice(bytes);
                if let Some(parent) = std::path::Path::new(&path).parent() {
                    std::fs::create_dir_all(parent).unwrap();
                }
                std::fs::write(path, bmp).unwrap();
            }
            release_overlay_canvas(&mut surface.canvas);
        }
    }
    #[test]
    fn two_rings_have_twelve_distinct_targets_and_neutral_center() {
        let menu = RadialMenu {
            center: Point { x: -600, y: 300 },
            radius: RADIAL_RADIUS,
            hovered: None,
        };
        assert!(menu.hit(menu.center).is_none());
        assert!(menu.hit(Point { x: 0, y: 0 }).is_none());
        for index in 0..12 {
            let angle =
                (index % 6) as f64 * std::f64::consts::PI / 3.0 - std::f64::consts::FRAC_PI_2;
            let r = menu.radius * if index < 6 { 0.415 } else { 0.805 };
            let point = Point {
                x: menu.center.x + (r * angle.cos()).round() as i32,
                y: menu.center.y + (r * angle.sin()).round() as i32,
            };
            assert_eq!(menu.hit(point), Some(index));
        }
    }
    #[test]
    fn placement_stays_inside_negative_coordinate_monitor() {
        let bounds = RECT {
            left: -1920,
            top: -200,
            right: 0,
            bottom: 880,
        };
        for point in [Point { x: -1920, y: -200 }, Point { x: -1, y: 879 }] {
            let center = radial_center(point, bounds, 234.0);
            assert!(center.x - 234 >= bounds.left && center.x + 234 < bounds.right);
            assert!(center.y - 234 >= bounds.top && center.y + 234 < bounds.bottom);
        }
    }
    #[test]
    fn shortcut_defaults_custom_combinations_and_invalid_keys() {
        let keys = |values: &[&str]| values.iter().map(|v| v.to_string()).collect::<Vec<_>>();
        assert_eq!(
            radial_shortcut(&keys(&["KeyQ"])).unwrap(),
            RadialShortcut {
                key: 0x51,
                modifiers: 0
            }
        );
        assert_eq!(
            radial_shortcut(&keys(&["ControlLeft", "ShiftLeft", "KeyR"]))
                .unwrap()
                .modifiers,
            3
        );
        assert_eq!(radial_shortcut(&[]).unwrap().key, 0);
        for keys in [
            keys(&["Escape"]),
            keys(&["MetaLeft", "KeyQ"]),
            keys(&["Alt", "Tab"]),
            keys(&["KeyQ", "KeyR"]),
        ] {
            assert!(radial_shortcut(&keys).is_err());
        }
    }

    #[test]
    fn shortcut_actions_rearm_after_left_release_and_ignore_auto_repeat() {
        let shortcut = RadialGate::default().shortcut;
        for _ in 0..10 {
            assert_eq!(
                radial_key_action(shortcut, false, 0x51, 0, false, true),
                None
            );
            assert_eq!(
                radial_key_action(shortcut, false, 0x51, 0, false, false),
                Some(RadialKeyAction::Toggle)
            );
            assert_eq!(
                radial_key_action(shortcut, true, 0x51, 0, true, false),
                None
            );
            assert_eq!(
                radial_key_action(shortcut, true, 0x1b, 0, false, false),
                Some(RadialKeyAction::Cancel)
            );
            assert_eq!(
                radial_key_action(shortcut, false, 0x1b, 0, false, false),
                None
            );
        }
        assert_eq!(
            radial_key_action(shortcut, false, 0x51, 1, false, false),
            None
        );
    }

    #[test]
    fn shortcut_gate_excludes_pointer_text_probe_and_hidden_modes() {
        assert!(radial_available(true, false, false, false));
        assert!(!radial_available(false, false, false, false));
        assert!(!radial_available(true, true, false, false));
        assert!(!radial_available(true, false, true, false));
        assert!(!radial_available(true, false, false, true));
        assert_eq!(RADIAL_RADIUS * 2.0, 156.0);
    }

    #[test]
    fn right_click_opens_cancels_and_respects_preference_and_mode() {
        assert_eq!(
            radial_right_action(false, true, true, false),
            Some(RadialKeyAction::Toggle)
        );
        assert_eq!(
            radial_right_action(true, true, true, false),
            Some(RadialKeyAction::Cancel)
        );
        // Cancelling an existing menu stays possible even if opening is disabled.
        assert_eq!(
            radial_right_action(true, false, true, false),
            Some(RadialKeyAction::Cancel)
        );
        assert_eq!(radial_right_action(false, false, true, false), None);
        assert_eq!(radial_right_action(false, true, false, false), None);
        assert_eq!(radial_right_action(false, true, true, true), None);
    }

    #[test]
    fn toolbar_shortcut_queues_without_requiring_a_canvas_click() {
        let (sender, receiver) = mpsc::channel();
        let overlay = NativeDrawingOverlay {
            sender: Some(sender),
        };
        overlay.radial_menu_action(false).unwrap();
        assert!(matches!(
            receiver.try_recv().unwrap(),
            DrawingCommand::ToggleRadial
        ));
        overlay.radial_menu_action(true).unwrap();
        assert!(matches!(
            receiver.try_recv().unwrap(),
            DrawingCommand::CancelRadial
        ));
        overlay
            .configure_radial(vec!["KeyQ".into()], vec!["pen".into()], false)
            .unwrap();
        assert!(matches!(
            receiver.try_recv().unwrap(),
            DrawingCommand::ConfigureRadial {
                right_click: false,
                ..
            }
        ));
        assert!(NativeDrawingOverlay { sender: None }
            .radial_menu_action(false)
            .is_err());
    }
}
