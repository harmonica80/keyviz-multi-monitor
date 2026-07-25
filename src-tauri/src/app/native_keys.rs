use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeKeyVisual {
    pub update_sequence: u64,
    pub visible: bool,
    pub groups: Vec<NativeKeyGroup>,
    pub flex_direction: String,
    pub alignment: String,
    pub margin_x: f64,
    pub margin_y: f64,
    pub style: String,
    pub text_size: f64,
    pub background_enabled: bool,
    pub background_color: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeKeyGroup {
    pub keys: Vec<NativeKeyItem>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeKeyItem {
    pub label: String,
    pub modifier: bool,
    pub mouse_kind: Option<String>,
    pub pressed: bool,
}

#[cfg(target_os = "windows")]
mod platform {
    use std::{
        sync::mpsc::{self, Receiver, Sender},
        thread,
        time::Duration,
    };

    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
            Graphics::Gdi::{
                BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, EndPaint,
                FillRect, GetClientRect, GetStockObject, GetTextExtentPoint32W, LineTo, MoveToEx,
                RoundRect, SelectObject, SetBkMode, SetTextColor, TextOutW, HOLLOW_BRUSH, NULL_PEN,
                PAINTSTRUCT, PS_SOLID, TRANSPARENT,
            },
            System::LibraryLoader::GetModuleHandleW,
            UI::WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, InvalidateRect,
                PeekMessageW, RegisterClassW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow,
                TranslateMessage, UpdateWindow, CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, LWA_COLORKEY,
                MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WM_DESTROY, WM_ERASEBKGND,
                WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
                WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
            },
        },
    };

    use super::NativeKeyVisual;

    const TRANSPARENT_KEY: COLORREF = COLORREF(1 | (2 << 8) | (3 << 16));

    #[derive(Clone, Default)]
    pub struct NativeKeyOverlay {
        sender: Option<Sender<KeyCommand>>,
    }

    enum KeyCommand {
        Update(KeyUpdate),
        Hide,
    }

    struct KeyUpdate {
        visual: NativeKeyVisual,
        monitor_left: i32,
        monitor_top: i32,
        monitor_width: i32,
        monitor_height: i32,
        scale: f64,
    }

    #[derive(Clone)]
    struct KeyLayout {
        rect: RECT,
        label: String,
        mouse_kind: Option<String>,
        pressed: bool,
    }

    #[derive(Clone)]
    struct GroupLayout {
        rect: RECT,
        keys: Vec<KeyLayout>,
        plus_positions: Vec<(i32, i32)>,
    }

    struct PaintModel {
        groups: Vec<GroupLayout>,
        theme: Theme,
        font_size: i32,
        corner_radius: i32,
        background_enabled: bool,
        background_color: COLORREF,
    }

    #[derive(Clone, Copy)]
    struct Theme {
        key: COLORREF,
        text: COLORREF,
        border: COLORREF,
        border_width: i32,
        shadow: COLORREF,
        shadow_y: i32,
        pressed_y: i32,
    }

    impl NativeKeyOverlay {
        pub fn new() -> Self {
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                if let Err(error) = run_window(receiver) {
                    eprintln!("Native key overlay failed: {error}");
                }
            });
            Self {
                sender: Some(sender),
            }
        }

        pub fn update(
            &self,
            visual: NativeKeyVisual,
            monitor_position: (i32, i32),
            monitor_size: (u32, u32),
            scale: f64,
        ) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(KeyCommand::Update(KeyUpdate {
                visual,
                monitor_left: monitor_position.0,
                monitor_top: monitor_position.1,
                monitor_width: monitor_size.0 as i32,
                monitor_height: monitor_size.1 as i32,
                scale,
            }));
        }

        pub fn hide(&self) {
            if let Some(sender) = &self.sender {
                let _ = sender.send(KeyCommand::Hide);
            }
        }
    }

    fn run_window(receiver: Receiver<KeyCommand>) -> Result<(), String> {
        let class_name = wide("KeyvizNativeKeyOverlay");
        let window_name = wide("Keyviz Keys");
        unsafe {
            let module = GetModuleHandleW(None).map_err(|error| error.to_string())?;
            let window_class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: module,
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            if RegisterClassW(&window_class) == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }

            let hwnd = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(window_name.as_ptr()),
                WS_POPUP,
                0,
                0,
                1,
                1,
                HWND(0),
                None,
                module,
                None,
            );
            if hwnd.0 == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if !SetLayeredWindowAttributes(hwnd, TRANSPARENT_KEY, 255, LWA_COLORKEY).as_bool() {
                DestroyWindow(hwnd);
                return Err(std::io::Error::last_os_error().to_string());
            }

            message_loop(hwnd, receiver);
            clear_paint_model(hwnd);
            DestroyWindow(hwnd);
        }
        Ok(())
    }

    unsafe fn message_loop(hwnd: HWND, receiver: Receiver<KeyCommand>) {
        let mut message = MSG::default();
        loop {
            match receiver.recv_timeout(Duration::from_millis(16)) {
                Ok(KeyCommand::Update(update)) => apply_update(hwnd, update),
                Ok(KeyCommand::Hide) => {
                    clear_paint_model(hwnd);
                    ShowWindow(hwnd, SW_HIDE);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            while PeekMessageW(&mut message, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    unsafe fn apply_update(hwnd: HWND, update: KeyUpdate) {
        if !update.visual.visible || update.visual.groups.is_empty() {
            clear_paint_model(hwnd);
            ShowWindow(hwnd, SW_HIDE);
            return;
        }

        let (model, width, height) = build_layout(&update.visual, update.scale.max(0.5));
        let margin_x = (update.visual.margin_x * update.scale).round() as i32;
        let margin_y = (update.visual.margin_y * update.scale).round() as i32;
        let horizontal = match update.visual.alignment.as_str() {
            "top-left" | "center-left" | "bottom-left" => margin_x,
            "top-right" | "center-right" | "bottom-right" => {
                update.monitor_width - width - margin_x
            }
            _ => (update.monitor_width - width) / 2,
        };
        let vertical = match update.visual.alignment.as_str() {
            "top-left" | "top-center" | "top-right" => margin_y,
            "center-left" | "center" | "center-right" => (update.monitor_height - height) / 2,
            _ => update.monitor_height - height - margin_y,
        };

        set_paint_model(hwnd, model);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            update.monitor_left + horizontal.max(0),
            update.monitor_top + vertical.max(0),
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        InvalidateRect(hwnd, None, true);
        UpdateWindow(hwnd);
    }

    fn build_layout(visual: &NativeKeyVisual, scale: f64) -> (PaintModel, i32, i32) {
        let text_size = (visual.text_size.max(12.0) * scale).round() as i32;
        let font_size = (text_size as f64 * 0.72).round() as i32;
        let key_height = (text_size as f64 * 1.9).round() as i32;
        let key_gap = (text_size as f64 * 0.35).round() as i32;
        let group_gap = (text_size as f64 * 0.5).round() as i32;
        let padding = 10.max((text_size as f64 * 0.35).round() as i32);
        let group_padding = if visual.background_enabled {
            (text_size as f64 * 0.4).round() as i32
        } else {
            0
        };

        let mut raw_groups = Vec::new();
        for group in &visual.groups {
            let mut widths = Vec::new();
            for key in &group.keys {
                let minimum = if key.modifier {
                    text_size as f64 * 2.8
                } else if key.mouse_kind.is_some() {
                    text_size as f64 * 1.9
                } else {
                    text_size as f64 * 2.0
                };
                let estimated_text = key.label.chars().count() as f64 * font_size as f64 * 0.62;
                widths.push(minimum.max(estimated_text + text_size as f64 * 1.3).round() as i32);
            }
            let plus_width = (font_size as f64 * 0.8).round() as i32;
            let content_width = widths.iter().sum::<i32>()
                + (group.keys.len().saturating_sub(1) as i32) * (key_gap * 2 + plus_width);
            raw_groups.push((group, widths, content_width + group_padding * 2));
        }

        let row_layout = visual.flex_direction == "row";
        let content_width = if row_layout {
            raw_groups.iter().map(|(_, _, width)| *width).sum::<i32>()
                + group_gap * raw_groups.len().saturating_sub(1) as i32
        } else {
            raw_groups
                .iter()
                .map(|(_, _, width)| *width)
                .max()
                .unwrap_or(1)
        };
        let group_height = key_height + group_padding * 2;
        let content_height = if row_layout {
            group_height
        } else {
            group_height * raw_groups.len() as i32
                + group_gap * raw_groups.len().saturating_sub(1) as i32
        };
        let width = (content_width + padding * 2).max(1);
        let height = (content_height + padding * 2 + 16).max(1);
        let mut layouts = Vec::new();
        let mut group_x = padding;
        let mut group_y = padding;

        for (group, widths, group_width) in raw_groups {
            let rect = RECT {
                left: group_x,
                top: group_y,
                right: group_x + group_width,
                bottom: group_y + group_height,
            };
            let mut keys = Vec::new();
            let mut plus_positions = Vec::new();
            let mut x = group_x + group_padding;
            for (index, key) in group.keys.iter().enumerate() {
                let y_offset = if key.pressed { 2 } else { 0 };
                keys.push(KeyLayout {
                    rect: RECT {
                        left: x,
                        top: group_y + group_padding + y_offset,
                        right: x + widths[index],
                        bottom: group_y + group_padding + y_offset + key_height,
                    },
                    label: key.label.clone(),
                    mouse_kind: key.mouse_kind.clone(),
                    pressed: key.pressed,
                });
                x += widths[index];
                if index + 1 < group.keys.len() {
                    plus_positions.push((x + key_gap, group_y + group_height / 2));
                    x += key_gap * 2 + (font_size as f64 * 0.8).round() as i32;
                }
            }
            layouts.push(GroupLayout {
                rect,
                keys,
                plus_positions,
            });
            if row_layout {
                group_x += group_width + group_gap;
            } else {
                group_y += group_height + group_gap;
            }
        }

        (
            PaintModel {
                groups: layouts,
                theme: theme(&visual.style, scale),
                font_size,
                corner_radius: 8.max((text_size as f64 * 0.28).round() as i32),
                background_enabled: visual.background_enabled,
                background_color: parse_color(&visual.background_color),
            },
            width,
            height,
        )
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                paint(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe fn paint(hwnd: HWND) {
        let Some(model) = take_paint_model(hwnd) else {
            return;
        };
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(hwnd, &mut paint);
        let mut client = RECT::default();
        GetClientRect(hwnd, &mut client);
        let background = CreateSolidBrush(TRANSPARENT_KEY);
        FillRect(dc, &client, background);
        DeleteObject(background);

        let font = CreateFontW(
            -model.font_size,
            0,
            0,
            0,
            500,
            0,
            0,
            0,
            0,
            0,
            0,
            5,
            0,
            PCWSTR(wide("Microsoft JhengHei").as_ptr()),
        );
        let old_font = SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT);

        for group in &model.groups {
            if model.background_enabled {
                fill_round_rect(
                    dc,
                    group.rect,
                    model.corner_radius * 2,
                    model.background_color,
                );
            }
            for key in &group.keys {
                draw_key(dc, key, &model);
            }
            SetTextColor(dc, model.theme.text);
            for (x, center_y) in &group.plus_positions {
                let plus = ['+' as u16];
                let mut size = Default::default();
                let _ = GetTextExtentPoint32W(dc, &plus, &mut size);
                let _ = TextOutW(dc, *x, *center_y - size.cy / 2, &plus);
            }
        }

        SelectObject(dc, old_font);
        DeleteObject(font);
        EndPaint(hwnd, &paint);
        restore_paint_model(hwnd, model);
    }

    unsafe fn draw_key(
        dc: windows::Win32::Graphics::Gdi::HDC,
        key: &KeyLayout,
        model: &PaintModel,
    ) {
        let shadow_y = if key.pressed {
            model.theme.shadow_y.min(2)
        } else {
            model.theme.shadow_y
        };
        if shadow_y > 0 {
            let shadow = RECT {
                top: key.rect.top + shadow_y,
                bottom: key.rect.bottom + shadow_y,
                ..key.rect
            };
            fill_round_rect(dc, shadow, model.corner_radius * 2, model.theme.shadow);
        }

        let brush = CreateSolidBrush(model.theme.key);
        let pen = CreatePen(
            PS_SOLID,
            model.theme.border_width.max(1),
            model.theme.border,
        );
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(
            dc,
            key.rect.left,
            key.rect.top,
            key.rect.right,
            key.rect.bottom,
            model.corner_radius * 2,
            model.corner_radius * 2,
        );
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(pen);
        DeleteObject(brush);

        SetTextColor(dc, model.theme.text);
        if let Some(kind) = &key.mouse_kind {
            draw_mouse_icon(dc, key.rect, kind, model.theme.text);
        } else {
            let text: Vec<u16> = key.label.encode_utf16().collect();
            let mut size = Default::default();
            let _ = GetTextExtentPoint32W(dc, &text, &mut size);
            let x = key.rect.left + (key.rect.right - key.rect.left - size.cx) / 2;
            let y = key.rect.top + (key.rect.bottom - key.rect.top - size.cy) / 2;
            let _ = TextOutW(dc, x, y, &text);
        }
    }

    unsafe fn draw_mouse_icon(
        dc: windows::Win32::Graphics::Gdi::HDC,
        rect: RECT,
        kind: &str,
        color: COLORREF,
    ) {
        let height = (rect.bottom - rect.top).min(38);
        let width = (height * 2 / 3).max(18);
        let left = rect.left + (rect.right - rect.left - width) / 2;
        let top = rect.top + (rect.bottom - rect.top - height) / 2;
        let pen = CreatePen(PS_SOLID, 2, color);
        let old_pen = SelectObject(dc, pen);
        let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
        RoundRect(dc, left, top, left + width, top + height, width, width);
        MoveToEx(dc, left, top + height / 3, None);
        LineTo(dc, left + width, top + height / 3);
        MoveToEx(dc, left + width / 2, top, None);
        LineTo(dc, left + width / 2, top + height / 3);
        let marker_x = match kind {
            "Left" => left + width / 4,
            "Right" => left + width * 3 / 4,
            _ => left + width / 2,
        };
        let marker_brush = CreateSolidBrush(color);
        let previous_brush = SelectObject(dc, marker_brush);
        RoundRect(dc, marker_x - 2, top + 4, marker_x + 2, top + 10, 3, 3);
        SelectObject(dc, previous_brush);
        DeleteObject(marker_brush);
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }

    unsafe fn fill_round_rect(
        dc: windows::Win32::Graphics::Gdi::HDC,
        rect: RECT,
        radius: i32,
        color: COLORREF,
    ) {
        let brush = CreateSolidBrush(color);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
        RoundRect(
            dc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius,
            radius,
        );
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(brush);
    }

    fn theme(style: &str, scale: f64) -> Theme {
        let scaled = |value: i32| (value as f64 * scale).round().max(1.0) as i32;
        match style {
            "outline" => Theme {
                key: rgb(255, 255, 255),
                text: rgb(36, 39, 43),
                border: rgb(48, 52, 58),
                border_width: scaled(2),
                shadow: TRANSPARENT_KEY,
                shadow_y: 0,
                pressed_y: scaled(2),
            },
            "raised" => raised_theme(
                rgb(247, 249, 251),
                rgb(36, 39, 43),
                rgb(207, 213, 220),
                scale,
            ),
            "dark" => raised_theme(rgb(32, 40, 56), rgb(255, 255, 255), rgb(14, 20, 32), scale),
            "retro" => raised_theme(
                rgb(246, 232, 184),
                rgb(59, 52, 35),
                rgb(189, 152, 75),
                scale,
            ),
            "mint" => raised_theme(
                rgb(237, 255, 249),
                rgb(16, 63, 55),
                rgb(75, 200, 168),
                scale,
            ),
            "rose" => raised_theme(
                rgb(255, 242, 245),
                rgb(83, 34, 47),
                rgb(239, 134, 161),
                scale,
            ),
            _ => Theme {
                key: rgb(255, 255, 255),
                text: rgb(32, 33, 36),
                border: rgb(226, 231, 238),
                border_width: scaled(1),
                shadow: rgb(224, 226, 230),
                shadow_y: scaled(3),
                pressed_y: scaled(2),
            },
        }
    }

    fn raised_theme(key: COLORREF, text: COLORREF, shadow: COLORREF, scale: f64) -> Theme {
        Theme {
            key,
            text,
            border: shadow,
            border_width: (1.0 * scale).round().max(1.0) as i32,
            shadow,
            shadow_y: (6.0 * scale).round().max(1.0) as i32,
            pressed_y: (4.0 * scale).round().max(1.0) as i32,
        }
    }

    fn parse_color(value: &str) -> COLORREF {
        let value = value.trim_start_matches('#');
        let parsed = u32::from_str_radix(value.get(..6).unwrap_or(value), 16).unwrap_or(0xffffff);
        rgb(
            ((parsed >> 16) & 0xff) as u8,
            ((parsed >> 8) & 0xff) as u8,
            (parsed & 0xff) as u8,
        )
    }

    const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
        COLORREF(red as u32 | ((green as u32) << 8) | ((blue as u32) << 16))
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn models() -> &'static std::sync::Mutex<std::collections::HashMap<isize, PaintModel>> {
        use std::sync::OnceLock;
        static MODELS: OnceLock<std::sync::Mutex<std::collections::HashMap<isize, PaintModel>>> =
            OnceLock::new();
        MODELS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
    }

    fn set_paint_model(hwnd: HWND, model: PaintModel) {
        if let Ok(mut models) = models().lock() {
            models.insert(hwnd.0, model);
        }
    }

    fn take_paint_model(hwnd: HWND) -> Option<PaintModel> {
        models().lock().ok()?.remove(&hwnd.0)
    }

    fn restore_paint_model(hwnd: HWND, model: PaintModel) {
        set_paint_model(hwnd, model);
    }

    fn clear_paint_model(hwnd: HWND) {
        if let Ok(mut models) = models().lock() {
            models.remove(&hwnd.0);
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::NativeKeyVisual;

    #[derive(Clone, Default)]
    pub struct NativeKeyOverlay;

    impl NativeKeyOverlay {
        pub fn new() -> Self {
            Self
        }

        pub fn update(
            &self,
            _visual: NativeKeyVisual,
            _monitor_position: (i32, i32),
            _monitor_size: (u32, u32),
            _scale: f64,
        ) {
        }

        pub fn hide(&self) {}
    }
}

pub use platform::NativeKeyOverlay;
