#[cfg(target_os = "windows")]
mod platform {
    use std::{
        ffi::c_void,
        iter::once,
        mem::size_of,
        sync::{
            mpsc::{self, Receiver, Sender},
            Mutex, OnceLock,
        },
        thread,
        time::Duration,
    };

    use serde::Serialize;
    use tauri::{AppHandle, Emitter, Manager};
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{
                COLORREF, HANDLE, HWND, LPARAM, LRESULT, POINT as WinPoint, RECT, SIZE, WPARAM,
            },
            Graphics::Gdi::{
                CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen,
                CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, Ellipse, GetDC,
                GetStockObject, LineTo, MoveToEx, Polygon, Rectangle, ReleaseDC, SelectObject,
                SetBkMode, AC_SRC_ALPHA, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
                DIB_RGB_COLORS, DT_LEFT, DT_SINGLELINE, DT_TOP, HDC, HOLLOW_BRUSH, PS_DOT,
                PS_SOLID, TRANSPARENT,
            },
            System::LibraryLoader::GetModuleHandleW,
            UI::{
                Input::KeyboardAndMouse::{
                    ReleaseCapture, SetCapture, SetFocus, VK_BACK, VK_ESCAPE, VK_RETURN,
                },
                WindowsAndMessaging::{
                    CreateIconIndirect, CreateWindowExW, DefWindowProcW, DestroyCursor,
                    DestroyWindow, DispatchMessageW, GetAncestor, GetWindowLongPtrW, LoadCursorW,
                    PeekMessageW, RegisterClassW, SetCursor, SetWindowLongPtrW, SetWindowPos,
                    ShowWindow, TranslateMessage, UpdateLayeredWindow, CREATESTRUCTW, CS_HREDRAW,
                    CS_VREDRAW, GA_ROOT, GWLP_USERDATA, GWL_EXSTYLE, HCURSOR, HTCLIENT,
                    HTTRANSPARENT, HWND_TOPMOST, ICONINFO, IDC_ARROW, IDC_CROSS, IDC_IBEAM, MSG,
                    PM_REMOVE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                    SWP_SHOWWINDOW, SW_HIDE, ULW_ALPHA, WM_APP, WM_CHAR, WM_COMMAND, WM_CREATE,
                    WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
                    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR, WNDCLASSW,
                    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
                    WS_EX_TRANSPARENT, WS_POPUP,
                },
            },
        },
    };

    const TRANSPARENT_KEY: COLORREF = COLORREF(1 | (2 << 8) | (3 << 16));
    const TRANSPARENT_PIXEL: u32 = 0x0001_0203;
    // Use the same RGB as the transparent key with minimal alpha so the layered
    // window can receive mouse input without showing dark artifacts on video.
    const INPUT_CAPTURE_PIXEL: u32 = 0x0101_0203;
    const WM_APP_COMMIT_TEXT: u32 = WM_APP + 1;
    const WM_APP_CANCEL_TEXT: u32 = WM_APP + 2;
    const NON_ANTIALIASED_FONT_QUALITY: u32 = 3;
    const TEXT_PADDING: i32 = 8;
    const MK_LBUTTON_MASK: usize = 0x0001;
    const ERASER_WIDTH_MULTIPLIER: i32 = 12;

    #[derive(Clone)]
    pub struct NativeDrawingOverlay {
        sender: Option<Sender<DrawingCommand>>,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum NativeTool {
        Pointer,
        Pen,
        Eraser,
        Line,
        Arrow,
        Rectangle,
        Ellipse,
        Text,
    }

    enum DrawingCommand {
        Show {
            left: i32,
            top: i32,
            width: i32,
            height: i32,
            toolbar_passthrough: Option<RECT>,
        },
        Hide,
        SetTool(NativeTool),
        SetColor(String),
        SetWidth(i32),
        Clear,
        Undo,
        SetClickThrough(bool),
        SetToolbarPassthrough(Option<RECT>),
        Focus,
        Raise,
        PointerDown {
            x: i32,
            y: i32,
        },
        PointerMove {
            x: i32,
            y: i32,
        },
        PointerUp {
            x: i32,
            y: i32,
        },
        Resize {
            left: i32,
            top: i32,
            width: i32,
            height: i32,
        },
    }

    #[derive(Clone, Copy)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[derive(Clone)]
    enum DrawingItem {
        Stroke {
            points: Vec<Point>,
            color: COLORREF,
            width: i32,
            erase: bool,
        },
        Shape {
            tool: NativeTool,
            start: Point,
            end: Point,
            color: COLORREF,
            width: i32,
        },
        Text {
            start: Point,
            text: String,
            color: COLORREF,
            width: i32,
        },
    }

    #[derive(Clone)]
    enum ActiveDrawing {
        Stroke {
            points: Vec<Point>,
            color: COLORREF,
            width: i32,
            erase: bool,
        },
        Shape {
            tool: NativeTool,
            start: Point,
            end: Point,
            color: COLORREF,
            width: i32,
        },
    }

    struct EditSession {
        start: Point,
        text: String,
        color: COLORREF,
        width: i32,
    }

    struct DragSession {
        index: usize,
        last: Point,
    }

    struct OverlayState {
        app: AppHandle,
        hwnd: HWND,
        tool: NativeTool,
        color: COLORREF,
        width: i32,
        drawings: Vec<DrawingItem>,
        active: Option<ActiveDrawing>,
        click_through: bool,
        visible: bool,
        bounds: RECT,
        toolbar_passthrough: Option<RECT>,
        edit: Option<EditSession>,
        selected: Option<usize>,
        drag: Option<DragSession>,
        cursor: HCURSOR,
        cursor_owned: bool,
    }

    #[derive(Clone, Serialize)]
    struct DrawingHistoryPayload {
        can_undo: bool,
    }

    #[derive(Clone, Serialize)]
    struct DrawingWidthPayload {
        width: i32,
    }

    static OVERLAY_STATE: OnceLock<Mutex<Option<OverlayState>>> = OnceLock::new();

    fn overlay_state() -> &'static Mutex<Option<OverlayState>> {
        OVERLAY_STATE.get_or_init(|| Mutex::new(None))
    }

    impl Default for NativeDrawingOverlay {
        fn default() -> Self {
            Self { sender: None }
        }
    }

    impl NativeDrawingOverlay {
        pub fn new(app: &AppHandle) -> Self {
            let (sender, receiver) = mpsc::channel();
            let app_handle = app.clone();
            thread::spawn(move || {
                if let Err(error) = run_window(receiver, app_handle) {
                    eprintln!("Native drawing overlay failed: {error}");
                }
            });
            Self {
                sender: Some(sender),
            }
        }

        pub fn show(
            &self,
            left: i32,
            top: i32,
            width: i32,
            height: i32,
            toolbar_passthrough: Option<(i32, i32, i32, i32)>,
        ) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::Show {
                left,
                top,
                width,
                height,
                toolbar_passthrough: toolbar_passthrough.map(|(left, top, right, bottom)| RECT {
                    left,
                    top,
                    right,
                    bottom,
                }),
            });
        }

        pub fn hide(&self) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::Hide);
        }

        pub fn set_tool(&self, tool: NativeTool) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::SetTool(tool));
        }

        pub fn set_color(&self, color: &str) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::SetColor(color.to_string()));
        }

        pub fn set_width(&self, width: i32) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::SetWidth(width));
        }

        pub fn clear(&self) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::Clear);
        }

        pub fn undo(&self) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::Undo);
        }

        pub fn set_click_through(&self, enabled: bool) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::SetClickThrough(enabled));
        }

        pub fn set_toolbar_passthrough(&self, bounds: Option<(i32, i32, i32, i32)>) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::SetToolbarPassthrough(bounds.map(
                |(left, top, right, bottom)| RECT {
                    left,
                    top,
                    right,
                    bottom,
                },
            )));
        }

        pub fn focus(&self) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::Focus);
        }

        pub fn raise(&self) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::Raise);
        }

        pub fn pointer_down(&self, x: i32, y: i32) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::PointerDown { x, y });
        }

        pub fn pointer_move(&self, x: i32, y: i32) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::PointerMove { x, y });
        }

        pub fn pointer_up(&self, x: i32, y: i32) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::PointerUp { x, y });
        }

        pub fn resize(&self, left: i32, top: i32, width: i32, height: i32) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::Resize {
                left,
                top,
                width,
                height,
            });
        }
    }

    pub fn parse_tool(value: &str) -> Option<NativeTool> {
        match value {
            "pointer" => Some(NativeTool::Pointer),
            "pen" => Some(NativeTool::Pen),
            "eraser" => Some(NativeTool::Eraser),
            "line" => Some(NativeTool::Line),
            "arrow" => Some(NativeTool::Arrow),
            "rectangle" => Some(NativeTool::Rectangle),
            "ellipse" => Some(NativeTool::Ellipse),
            "text" => Some(NativeTool::Text),
            _ => None,
        }
    }

    fn run_window(receiver: Receiver<DrawingCommand>, app: AppHandle) -> Result<(), String> {
        let class_name = wide("KeyvizNativeDrawingOverlay");
        let window_name = wide("Keyviz Drawing");

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
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
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
            let (cursor, cursor_owned) = create_tool_cursor(NativeTool::Pen, 5);

            if let Ok(mut state) = overlay_state().lock() {
                *state = Some(OverlayState {
                    app,
                    hwnd,
                    tool: NativeTool::Pen,
                    color: parse_color("#ef2b2d"),
                    width: 5,
                    drawings: Vec::new(),
                    active: None,
                    click_through: false,
                    visible: false,
                    bounds: RECT::default(),
                    toolbar_passthrough: None,
                    edit: None,
                    selected: None,
                    drag: None,
                    cursor,
                    cursor_owned,
                });
            }

            ShowWindow(hwnd, SW_HIDE);
            message_loop(receiver);

            if let Ok(mut state) = overlay_state().lock() {
                *state = None;
            }
            DestroyWindow(hwnd);
        }

        Ok(())
    }

    unsafe fn message_loop(receiver: Receiver<DrawingCommand>) {
        let mut message = MSG::default();

        loop {
            match receiver.recv_timeout(Duration::from_millis(16)) {
                Ok(command) => apply_command(command),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            while PeekMessageW(&mut message, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    unsafe fn apply_command(command: DrawingCommand) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };

        match command {
            DrawingCommand::Show {
                left,
                top,
                width,
                height,
                toolbar_passthrough,
            } => {
                state.toolbar_passthrough = toolbar_passthrough;
                state.visible = true;
                state.bounds = RECT {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                };
                let _ = SetWindowPos(
                    state.hwnd,
                    HWND_TOPMOST,
                    left,
                    top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                apply_click_through(state.hwnd, state.click_through);
                emit_history(&state.app, !state.drawings.is_empty());
                refresh_overlay(state);
            }
            DrawingCommand::Resize {
                left,
                top,
                width,
                height,
            } => {
                state.bounds = RECT {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                };
                let _ = SetWindowPos(
                    state.hwnd,
                    HWND_TOPMOST,
                    left,
                    top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                apply_click_through(state.hwnd, state.click_through);
                emit_history(&state.app, !state.drawings.is_empty());
                refresh_overlay(state);
            }
            DrawingCommand::Hide => {
                cancel_text_editor(state);
                state.drawings.clear();
                state.active = None;
                state.selected = None;
                state.drag = None;
                state.visible = false;
                ShowWindow(state.hwnd, SW_HIDE);
                emit_history(&state.app, false);
            }
            DrawingCommand::SetTool(tool) => {
                commit_text_editor(state);
                let click_through = matches!(tool, NativeTool::Pointer);
                state.tool = tool;
                state.selected = None;
                state.drag = None;
                state.click_through = click_through;
                apply_click_through(state.hwnd, click_through);
                replace_tool_cursor(state, false);
                if state.visible {
                    raise_toolbar(&state.app);
                }
                refresh_overlay(state);
            }
            DrawingCommand::SetColor(color) => state.color = parse_color(&color),
            DrawingCommand::SetWidth(width) => {
                let width = width.clamp(1, 15);
                state.width = width;
                if matches!(state.tool, NativeTool::Eraser) {
                    replace_tool_cursor(state, false);
                }
                if let Some(index) = state.selected {
                    let tool = state.tool;
                    if let Some(item) = state.drawings.get_mut(index) {
                        if drawing_matches_tool(item, tool) {
                            set_drawing_width(item, width);
                            refresh_overlay(state);
                        }
                    }
                }
            }
            DrawingCommand::Clear => {
                cancel_text_editor(state);
                state.drawings.clear();
                state.active = None;
                state.selected = None;
                state.drag = None;
                emit_history(&state.app, false);
                refresh_overlay(state);
            }
            DrawingCommand::Undo => {
                if state.edit.is_none() {
                    state.selected = None;
                    state.drag = None;
                    state.drawings.pop();
                    emit_history(&state.app, !state.drawings.is_empty());
                    refresh_overlay(state);
                }
            }
            DrawingCommand::SetClickThrough(enabled) => {
                state.click_through = enabled;
                apply_click_through(state.hwnd, enabled);
                if state.visible {
                    raise_toolbar(&state.app);
                }
                refresh_overlay(state);
            }
            DrawingCommand::SetToolbarPassthrough(bounds) => {
                state.toolbar_passthrough = bounds;
                refresh_overlay(state);
            }
            DrawingCommand::Focus => {
                if state.edit.is_some() || !state.click_through {
                    let _ = SetFocus(state.hwnd);
                }
            }
            DrawingCommand::Raise => {
                if !state.visible {
                    return;
                }
                let _ = SetWindowPos(
                    state.hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                );
                raise_toolbar(&state.app);
            }
            DrawingCommand::PointerDown { x, y } => {
                if let Some(point) = global_point_for_drawing(state, x, y) {
                    begin_drawing_at(state, point, Some(state.hwnd));
                }
            }
            DrawingCommand::PointerMove { x, y } => {
                if let Some(point) = global_point_for_drawing(state, x, y) {
                    update_drawing_at(state, point);
                }
            }
            DrawingCommand::PointerUp { x, y } => {
                if let Some(point) = global_point_for_drawing(state, x, y) {
                    finish_drawing_at(state, point);
                }
            }
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_CREATE => {
                let createstruct = lparam.0 as *const CREATESTRUCTW;
                if !createstruct.is_null() {
                    let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, hwnd.0);
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_NCHITTEST => {
                if is_click_through_or_passthrough_point(hwnd, lparam) {
                    LRESULT(HTTRANSPARENT as isize)
                } else {
                    LRESULT(HTCLIENT as isize)
                }
            }
            WM_SETCURSOR => {
                if set_overlay_cursor(hwnd) {
                    LRESULT(1)
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_PAINT => DefWindowProcW(hwnd, message, wparam, lparam),
            WM_LBUTTONDOWN => {
                on_left_button_down(hwnd, lparam);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                on_mouse_move(hwnd, wparam, lparam);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                on_left_button_up(hwnd, lparam);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                on_mouse_wheel(hwnd, wparam);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                on_key_down(hwnd, wparam);
                LRESULT(0)
            }
            WM_CHAR => {
                on_char(hwnd, wparam);
                LRESULT(0)
            }
            WM_COMMAND => LRESULT(0),
            WM_APP_COMMIT_TEXT => {
                on_commit_text(hwnd);
                LRESULT(0)
            }
            WM_APP_CANCEL_TEXT => {
                on_cancel_text(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe fn on_left_button_down(hwnd: HWND, lparam: LPARAM) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };
        if state.hwnd != hwnd {
            return;
        }

        let point = lparam_point(lparam);
        begin_drawing_at(state, point, Some(hwnd));
    }

    unsafe fn begin_drawing_at(state: &mut OverlayState, point: Point, capture_hwnd: Option<HWND>) {
        if !state.visible
            || matches!(state.tool, NativeTool::Pointer)
            || is_toolbar_passthrough_point(state, point, false)
        {
            return;
        }

        commit_text_editor(state);
        if !matches!(state.tool, NativeTool::Eraser) {
            if let Some(index) = hit_test_drawing(state, point) {
                state.selected = Some(index);
                state.drag = Some(DragSession { index, last: point });
                state.width = drawing_width(&state.drawings[index]);
                emit_width(&state.app, state.width);
                if let Some(hwnd) = capture_hwnd {
                    SetCapture(hwnd);
                }
                refresh_overlay(state);
                return;
            }
        }

        state.selected = None;
        state.drag = None;
        match state.tool.clone() {
            NativeTool::Pointer => {}
            NativeTool::Text => {
                create_text_editor(state, point);
            }
            NativeTool::Pen => {
                state.active = Some(ActiveDrawing::Stroke {
                    points: vec![point],
                    color: state.color,
                    width: state.width.max(1),
                    erase: false,
                });
                if let Some(hwnd) = capture_hwnd {
                    SetCapture(hwnd);
                }
            }
            NativeTool::Eraser => {
                state.active = Some(ActiveDrawing::Stroke {
                    points: vec![point],
                    color: TRANSPARENT_KEY,
                    width: (state.width.max(1) * ERASER_WIDTH_MULTIPLIER)
                        .max(ERASER_WIDTH_MULTIPLIER),
                    erase: true,
                });
                if let Some(hwnd) = capture_hwnd {
                    SetCapture(hwnd);
                }
            }
            tool => {
                state.active = Some(ActiveDrawing::Shape {
                    tool,
                    start: point,
                    end: point,
                    color: state.color,
                    width: state.width.max(1),
                });
                if let Some(hwnd) = capture_hwnd {
                    SetCapture(hwnd);
                }
            }
        }
        refresh_overlay(state);
    }

    unsafe fn on_mouse_move(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };
        if state.hwnd != hwnd || (state.active.is_none() && state.drag.is_none()) {
            return;
        }
        if (wparam.0 & MK_LBUTTON_MASK) == 0 {
            return;
        }

        let point = lparam_point(lparam);
        update_drawing_at(state, point);
    }

    unsafe fn update_drawing_at(state: &mut OverlayState, point: Point) {
        if !state.visible || matches!(state.tool, NativeTool::Pointer) {
            return;
        }
        if let Some(drag) = state.drag.as_mut() {
            let dx = point.x - drag.last.x;
            let dy = point.y - drag.last.y;
            if dx != 0 || dy != 0 {
                if let Some(item) = state.drawings.get_mut(drag.index) {
                    translate_drawing(item, dx, dy);
                }
                drag.last = point;
            }
            refresh_overlay(state);
            return;
        }

        match state.active.as_mut() {
            Some(ActiveDrawing::Stroke { points, .. }) => points.push(point),
            Some(ActiveDrawing::Shape { end, .. }) => *end = point,
            None => {}
        }
        refresh_overlay(state);
    }

    unsafe fn on_left_button_up(hwnd: HWND, _lparam: LPARAM) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };
        if state.hwnd != hwnd {
            return;
        }
        finish_drawing_at(state, lparam_point(_lparam));
    }

    unsafe fn finish_drawing_at(state: &mut OverlayState, _point: Point) {
        if state.drag.take().is_some() {
            ReleaseCapture();
            refresh_overlay(state);
            return;
        }

        let Some(active) = state.active.take() else {
            return;
        };
        let drawing = match active {
            ActiveDrawing::Stroke {
                points,
                color,
                width,
                erase,
            } => {
                if points.len() < 2 {
                    ReleaseCapture();
                    return;
                }
                DrawingItem::Stroke {
                    points,
                    color,
                    width,
                    erase,
                }
            }
            ActiveDrawing::Shape {
                tool,
                start,
                end,
                color,
                width,
            } => DrawingItem::Shape {
                tool,
                start,
                end,
                color,
                width,
            },
        };

        state.drawings.push(drawing);
        ReleaseCapture();
        emit_history(&state.app, true);
        refresh_overlay(state);
    }

    unsafe fn on_mouse_wheel(hwnd: HWND, wparam: WPARAM) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };
        if state.hwnd != hwnd || !state.visible {
            return;
        }

        let delta = ((wparam.0 >> 16) & 0xffff) as u16 as i16;
        if delta == 0 {
            return;
        }
        let step = if delta > 0 { 1 } else { -1 };
        if matches!(state.tool, NativeTool::Eraser) {
            let width = (state.width + step).clamp(1, 15);
            state.width = width;
            replace_tool_cursor(state, true);
            emit_width(&state.app, width);
            refresh_overlay(state);
            return;
        }
        let Some(index) = state.selected else {
            return;
        };
        let tool = state.tool;
        let Some(item) = state.drawings.get_mut(index) else {
            state.selected = None;
            return;
        };
        if !drawing_matches_tool(item, tool) {
            state.selected = None;
            return;
        }

        let width = (drawing_width(item) + step).clamp(1, 15);
        set_drawing_width(item, width);
        state.width = width;
        emit_width(&state.app, width);
        refresh_overlay(state);
    }

    unsafe fn on_key_down(hwnd: HWND, wparam: WPARAM) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };
        if state.hwnd != hwnd {
            return;
        }

        match wparam.0 as u32 {
            code if code == VK_RETURN.0 as u32 && state.edit.is_some() => {
                commit_text_editor(state);
            }
            code if code == VK_BACK.0 as u32 && state.edit.is_some() => {
                if let Some(edit) = state.edit.as_mut() {
                    edit.text.pop();
                }
                refresh_overlay(state);
            }
            code if code == VK_ESCAPE.0 as u32 && state.edit.is_some() => {
                if state.edit.is_some() {
                    cancel_text_editor(state);
                }
            }
            _ => {}
        }
    }

    unsafe fn on_char(hwnd: HWND, wparam: WPARAM) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };
        if state.hwnd != hwnd {
            return;
        }
        let Some(edit) = state.edit.as_mut() else {
            return;
        };
        if matches!(wparam.0 as u32, 8 | 13 | 27) {
            return;
        }
        if let Some(ch) = char::from_u32(wparam.0 as u32) {
            if !ch.is_control() {
                edit.text.push(ch);
                refresh_overlay(state);
            }
        }
    }

    unsafe fn on_commit_text(hwnd: HWND) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };
        if state.hwnd != hwnd {
            return;
        }
        commit_text_editor(state);
    }

    unsafe fn on_cancel_text(hwnd: HWND) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };
        if state.hwnd != hwnd {
            return;
        }
        cancel_text_editor(state);
    }

    unsafe fn refresh_overlay(state: &OverlayState) {
        let width = (state.bounds.right - state.bounds.left).max(1);
        let height = (state.bounds.bottom - state.bounds.top).max(1);
        let pixel_count = (width as usize).saturating_mul(height as usize);
        if pixel_count == 0 {
            return;
        }

        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            return;
        }
        let memory_dc = CreateCompatibleDC(screen_dc);
        if memory_dc.0 == 0 {
            ReleaseDC(HWND(0), screen_dc);
            return;
        }

        let mut bitmap_info = BITMAPINFO {
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
        let Ok(bitmap) = CreateDIBSection(
            memory_dc,
            &mut bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            HANDLE(0),
            0,
        ) else {
            DeleteDC(memory_dc);
            ReleaseDC(HWND(0), screen_dc);
            return;
        };
        if bits.is_null() {
            DeleteObject(bitmap);
            DeleteDC(memory_dc);
            ReleaseDC(HWND(0), screen_dc);
            return;
        }

        let old_bitmap = SelectObject(memory_dc, bitmap);
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, pixel_count);
        pixels.fill(TRANSPARENT_PIXEL);

        let drawing_dc = HDC(memory_dc.0);
        SetBkMode(drawing_dc, TRANSPARENT);
        for drawing in state.drawings.iter() {
            draw_item(drawing_dc, drawing);
        }
        if let Some(index) = state.selected {
            if let Some(drawing) = state.drawings.get(index) {
                draw_selection(drawing_dc, drawing);
            }
        }
        if let Some(active) = state.active.as_ref() {
            draw_active(drawing_dc, active);
        }
        if let Some(edit) = state.edit.as_ref() {
            let preview = if edit.text.is_empty() {
                "|".to_string()
            } else {
                format!("{}|", edit.text)
            };
            draw_text(
                drawing_dc,
                Point {
                    x: edit.start.x + TEXT_PADDING,
                    y: edit.start.y + TEXT_PADDING,
                },
                &preview,
                edit.color,
                edit.width,
            );
        }

        for (index, pixel) in pixels.iter_mut().enumerate() {
            let x = (index % width as usize) as i32 + state.bounds.left;
            let y = (index / width as usize) as i32 + state.bounds.top;
            if is_toolbar_passthrough_point(state, Point { x, y }, true) {
                *pixel = 0;
                continue;
            }
            if (*pixel & 0x00ff_ffff) == TRANSPARENT_PIXEL {
                *pixel = if state.click_through {
                    0
                } else {
                    INPUT_CAPTURE_PIXEL
                };
            } else {
                *pixel |= 0xff00_0000;
            }
        }

        let destination = WinPoint {
            x: state.bounds.left,
            y: state.bounds.top,
        };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let source = WinPoint { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: 0,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let _ = UpdateLayeredWindow(
            state.hwnd,
            screen_dc,
            Some(&destination),
            Some(&size),
            drawing_dc,
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );

        SelectObject(memory_dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(memory_dc);
        ReleaseDC(HWND(0), screen_dc);
    }

    fn drawing_matches_tool(drawing: &DrawingItem, tool: NativeTool) -> bool {
        match drawing {
            DrawingItem::Stroke { erase, .. } => matches!(tool, NativeTool::Pen) && !erase,
            DrawingItem::Shape {
                tool: item_tool, ..
            } => *item_tool == tool,
            DrawingItem::Text { .. } => matches!(tool, NativeTool::Text),
        }
    }

    fn drawing_width(drawing: &DrawingItem) -> i32 {
        match drawing {
            DrawingItem::Stroke { width, .. }
            | DrawingItem::Shape { width, .. }
            | DrawingItem::Text { width, .. } => *width,
        }
    }

    fn set_drawing_width(drawing: &mut DrawingItem, value: i32) {
        let value = value.clamp(1, 15);
        match drawing {
            DrawingItem::Stroke { width, .. }
            | DrawingItem::Shape { width, .. }
            | DrawingItem::Text { width, .. } => *width = value,
        }
    }

    fn translate_drawing(drawing: &mut DrawingItem, dx: i32, dy: i32) {
        match drawing {
            DrawingItem::Stroke { points, .. } => {
                for point in points {
                    point.x += dx;
                    point.y += dy;
                }
            }
            DrawingItem::Shape { start, end, .. } => {
                start.x += dx;
                start.y += dy;
                end.x += dx;
                end.y += dy;
            }
            DrawingItem::Text { start, .. } => {
                start.x += dx;
                start.y += dy;
            }
        }
    }

    fn hit_test_drawing(state: &OverlayState, point: Point) -> Option<usize> {
        state
            .drawings
            .iter()
            .enumerate()
            .rev()
            .find(|(_, drawing)| {
                drawing_matches_tool(drawing, state.tool) && drawing_hit_test(drawing, point)
            })
            .map(|(index, _)| index)
    }

    fn drawing_hit_test(drawing: &DrawingItem, point: Point) -> bool {
        let tolerance = (drawing_width(drawing) as f64 / 2.0 + 8.0).max(10.0);
        match drawing {
            DrawingItem::Stroke { points, .. } => points
                .windows(2)
                .any(|segment| distance_to_segment(point, segment[0], segment[1]) <= tolerance),
            DrawingItem::Shape {
                tool, start, end, ..
            } => match tool {
                NativeTool::Line | NativeTool::Arrow => {
                    distance_to_segment(point, *start, *end) <= tolerance
                }
                NativeTool::Rectangle => {
                    let left = start.x.min(end.x);
                    let right = start.x.max(end.x);
                    let top = start.y.min(end.y);
                    let bottom = start.y.max(end.y);
                    let corners = [
                        Point { x: left, y: top },
                        Point { x: right, y: top },
                        Point {
                            x: right,
                            y: bottom,
                        },
                        Point { x: left, y: bottom },
                    ];
                    (0..4).any(|index| {
                        distance_to_segment(point, corners[index], corners[(index + 1) % 4])
                            <= tolerance
                    })
                }
                NativeTool::Ellipse => ellipse_hit_test(point, *start, *end, tolerance),
                _ => false,
            },
            DrawingItem::Text {
                start, text, width, ..
            } => {
                let font_size = text_font_size(*width) as f64;
                let text_units: f64 = text
                    .chars()
                    .map(|character| if character.is_ascii() { 0.62 } else { 1.0 })
                    .sum();
                let text_width = (text_units * font_size).max(font_size * 0.6);
                point.x as f64 >= start.x as f64 - tolerance
                    && point.x as f64 <= start.x as f64 + text_width + tolerance
                    && point.y as f64 >= start.y as f64 - tolerance
                    && point.y as f64 <= start.y as f64 + font_size + tolerance
            }
        }
    }

    fn distance_to_segment(point: Point, start: Point, end: Point) -> f64 {
        let dx = (end.x - start.x) as f64;
        let dy = (end.y - start.y) as f64;
        if dx == 0.0 && dy == 0.0 {
            return (((point.x - start.x).pow(2) + (point.y - start.y).pow(2)) as f64).sqrt();
        }
        let projection = (((point.x - start.x) as f64 * dx + (point.y - start.y) as f64 * dy)
            / (dx * dx + dy * dy))
            .clamp(0.0, 1.0);
        let nearest_x = start.x as f64 + projection * dx;
        let nearest_y = start.y as f64 + projection * dy;
        ((point.x as f64 - nearest_x).powi(2) + (point.y as f64 - nearest_y).powi(2)).sqrt()
    }

    fn ellipse_hit_test(point: Point, start: Point, end: Point, tolerance: f64) -> bool {
        let radius_x = ((end.x - start.x).abs() as f64 / 2.0).max(1.0);
        let radius_y = ((end.y - start.y).abs() as f64 / 2.0).max(1.0);
        let center_x = (start.x + end.x) as f64 / 2.0;
        let center_y = (start.y + end.y) as f64 / 2.0;
        let normalized = (((point.x as f64 - center_x) / radius_x).powi(2)
            + ((point.y as f64 - center_y) / radius_y).powi(2))
        .sqrt();
        (normalized - 1.0).abs() * radius_x.min(radius_y) <= tolerance
    }

    fn drawing_bounds(drawing: &DrawingItem) -> RECT {
        match drawing {
            DrawingItem::Stroke { points, width, .. } => {
                let padding = (*width).max(1) + 5;
                let left = points.iter().map(|point| point.x).min().unwrap_or(0) - padding;
                let top = points.iter().map(|point| point.y).min().unwrap_or(0) - padding;
                let right = points.iter().map(|point| point.x).max().unwrap_or(0) + padding;
                let bottom = points.iter().map(|point| point.y).max().unwrap_or(0) + padding;
                RECT {
                    left,
                    top,
                    right,
                    bottom,
                }
            }
            DrawingItem::Shape {
                start, end, width, ..
            } => {
                let padding = (*width).max(1) + 5;
                RECT {
                    left: start.x.min(end.x) - padding,
                    top: start.y.min(end.y) - padding,
                    right: start.x.max(end.x) + padding,
                    bottom: start.y.max(end.y) + padding,
                }
            }
            DrawingItem::Text {
                start, text, width, ..
            } => {
                let font_size = text_font_size(*width);
                let units: f64 = text
                    .chars()
                    .map(|character| if character.is_ascii() { 0.62 } else { 1.0 })
                    .sum();
                RECT {
                    left: start.x - 5,
                    top: start.y - 5,
                    right: start.x + (units * font_size as f64).ceil() as i32 + 5,
                    bottom: start.y + font_size + 5,
                }
            }
        }
    }

    unsafe fn draw_selection(dc: HDC, drawing: &DrawingItem) {
        let bounds = drawing_bounds(drawing);
        let pen = CreatePen(PS_DOT, 1, COLORREF(0x0080_8080));
        let old_pen = SelectObject(dc, pen);
        let brush = GetStockObject(HOLLOW_BRUSH);
        let old_brush = SelectObject(dc, brush);
        Rectangle(dc, bounds.left, bounds.top, bounds.right, bounds.bottom);
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }

    unsafe fn draw_item(dc: windows::Win32::Graphics::Gdi::HDC, drawing: &DrawingItem) {
        match drawing {
            DrawingItem::Stroke {
                points,
                color,
                width,
                erase: _,
            } => draw_polyline(dc, points, *color, *width),
            DrawingItem::Shape {
                tool,
                start,
                end,
                color,
                width,
            } => draw_shape(dc, tool.clone(), *start, *end, *color, *width),
            DrawingItem::Text {
                start,
                text,
                color,
                width,
            } => draw_text(dc, *start, text, *color, *width),
        }
    }

    unsafe fn draw_active(dc: windows::Win32::Graphics::Gdi::HDC, drawing: &ActiveDrawing) {
        match drawing {
            ActiveDrawing::Stroke {
                points,
                color,
                width,
                erase: _,
            } => draw_polyline(dc, points, *color, *width),
            ActiveDrawing::Shape {
                tool,
                start,
                end,
                color,
                width,
            } => draw_shape(dc, tool.clone(), *start, *end, *color, *width),
        }
    }

    unsafe fn draw_polyline(
        dc: windows::Win32::Graphics::Gdi::HDC,
        points: &[Point],
        color: COLORREF,
        width: i32,
    ) {
        if points.len() < 2 {
            return;
        }
        let pen = CreatePen(PS_SOLID, width.max(1), color);
        let old_pen = SelectObject(dc, pen);
        MoveToEx(dc, points[0].x, points[0].y, None);
        for point in points.iter().skip(1) {
            LineTo(dc, point.x, point.y);
        }
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }

    unsafe fn draw_shape(
        dc: windows::Win32::Graphics::Gdi::HDC,
        tool: NativeTool,
        start: Point,
        end: Point,
        color: COLORREF,
        width: i32,
    ) {
        let pen = CreatePen(PS_SOLID, width.max(1), color);
        let old_pen = SelectObject(dc, pen);
        let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
        match tool {
            NativeTool::Line => {
                MoveToEx(dc, start.x, start.y, None);
                LineTo(dc, end.x, end.y);
            }
            NativeTool::Arrow => {
                draw_tapered_arrow(dc, start, end, color, width.max(1));
            }
            NativeTool::Rectangle => {
                Rectangle(dc, start.x, start.y, end.x, end.y);
            }
            NativeTool::Ellipse => {
                Ellipse(dc, start.x, start.y, end.x, end.y);
            }
            _ => {}
        }
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }

    unsafe fn draw_tapered_arrow(
        dc: windows::Win32::Graphics::Gdi::HDC,
        start: Point,
        end: Point,
        color: COLORREF,
        width: i32,
    ) {
        let dx = (end.x - start.x) as f64;
        let dy = (end.y - start.y) as f64;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance < 2.0 {
            return;
        }
        let ux = dx / distance;
        let uy = dy / distance;
        let nx = -uy;
        let ny = ux;
        let head_length = ((width * 6) as f64).max(30.0).min(distance * 0.48);
        let head_half = ((width * 3) as f64).max(12.0).min(distance * 0.24);
        let start_half = ((width as f64) * 0.2).max(1.0);
        let neck_half = ((width as f64) * 0.9).max(3.0);
        let point = |along: f64, normal: f64| WinPoint {
            x: (start.x as f64 + ux * along + nx * normal).round() as i32,
            y: (start.y as f64 + uy * along + ny * normal).round() as i32,
        };
        let neck = distance - head_length;
        let points = [
            point(0.0, start_half),
            point(neck, neck_half),
            point(distance - head_length * 0.68, head_half),
            WinPoint { x: end.x, y: end.y },
            point(distance - head_length * 1.02, -head_half * 0.62),
            point(neck, -neck_half),
            point(0.0, -start_half),
        ];
        let brush = CreateSolidBrush(color);
        let old_brush = SelectObject(dc, brush);
        let _ = Polygon(dc, &points);
        SelectObject(dc, old_brush);
        DeleteObject(brush);
    }

    unsafe fn draw_text(
        dc: windows::Win32::Graphics::Gdi::HDC,
        start: Point,
        text: &str,
        color: COLORREF,
        width: i32,
    ) {
        let height = -text_font_size(width);
        let font = CreateFontW(
            height,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            0,
            0,
            0,
            NON_ANTIALIASED_FONT_QUALITY,
            0,
            PCWSTR(wide("Microsoft JhengHei").as_ptr()),
        );
        let old_font = SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT);
        let old_color = windows::Win32::Graphics::Gdi::SetTextColor(dc, color);
        let mut rect = RECT {
            left: start.x,
            top: start.y,
            right: start.x + 1200,
            bottom: start.y + 120,
        };
        let mut wide_text = wide(text);
        DrawTextW(
            dc,
            &mut wide_text,
            &mut rect,
            DT_LEFT | DT_TOP | DT_SINGLELINE,
        );
        windows::Win32::Graphics::Gdi::SetTextColor(dc, old_color);
        SelectObject(dc, old_font);
        DeleteObject(font);
    }

    fn text_font_size(width: i32) -> i32 {
        18.max(width * 4)
    }

    unsafe fn create_text_editor(state: &mut OverlayState, point: Point) {
        cancel_text_editor(state);
        state.edit = Some(EditSession {
            start: point,
            text: String::new(),
            color: state.color,
            width: state.width.max(1),
        });
        let _ = SetFocus(state.hwnd);
        refresh_overlay(state);
    }

    unsafe fn commit_text_editor(state: &mut OverlayState) {
        let Some(edit) = state.edit.take() else {
            return;
        };
        let text = edit.text.trim().to_string();
        if !text.is_empty() {
            state.drawings.push(DrawingItem::Text {
                start: Point {
                    x: edit.start.x + TEXT_PADDING,
                    y: edit.start.y + TEXT_PADDING,
                },
                text,
                color: edit.color,
                width: edit.width,
            });
            emit_history(&state.app, true);
        }
        refresh_overlay(state);
    }

    unsafe fn cancel_text_editor(state: &mut OverlayState) {
        state.edit = None;
        refresh_overlay(state);
    }

    fn raise_toolbar(app: &AppHandle) {
        let Some(toolbar) = app.get_webview_window("drawing-toolbar") else {
            return;
        };
        let Ok(hwnd) = toolbar.hwnd() else {
            return;
        };
        unsafe {
            let toolbar_hwnd = HWND(hwnd.0 as isize);
            let root = GetAncestor(toolbar_hwnd, GA_ROOT);
            let target = if root.0 == 0 { toolbar_hwnd } else { root };
            let _ = SetWindowPos(
                target,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
        }
    }

    fn is_click_through_or_passthrough_point(hwnd: HWND, lparam: LPARAM) -> bool {
        let point = lparam_point(lparam);
        overlay_state()
            .lock()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .filter(|state| state.hwnd == hwnd)
                    .map(|state| {
                        state.click_through || is_toolbar_passthrough_point(state, point, true)
                    })
            })
            .unwrap_or(true)
    }

    fn global_point_for_drawing(state: &OverlayState, x: i32, y: i32) -> Option<Point> {
        if !state.visible || matches!(state.tool, NativeTool::Pointer) {
            return None;
        }
        let global = Point { x, y };
        if is_toolbar_passthrough_point(state, global, true) {
            return None;
        }
        if x < state.bounds.left
            || x >= state.bounds.right
            || y < state.bounds.top
            || y >= state.bounds.bottom
        {
            return None;
        }
        Some(Point {
            x: x - state.bounds.left,
            y: y - state.bounds.top,
        })
    }

    fn is_toolbar_passthrough_point(
        state: &OverlayState,
        point: Point,
        point_is_global: bool,
    ) -> bool {
        let global = if point_is_global {
            point
        } else {
            Point {
                x: point.x + state.bounds.left,
                y: point.y + state.bounds.top,
            }
        };
        state
            .toolbar_passthrough
            .as_ref()
            .map(|rect| {
                global.x >= rect.left
                    && global.x < rect.right
                    && global.y >= rect.top
                    && global.y < rect.bottom
            })
            .unwrap_or(false)
    }

    unsafe fn create_tool_cursor(tool: NativeTool, width: i32) -> (HCURSOR, bool) {
        let custom = match tool {
            NativeTool::Pen => create_pen_cursor(),
            NativeTool::Eraser => create_eraser_cursor(width),
            _ => None,
        };
        if let Some(cursor) = custom {
            return (cursor, true);
        }

        let resource = match tool {
            NativeTool::Pointer => IDC_ARROW,
            NativeTool::Text => IDC_IBEAM,
            _ => IDC_CROSS,
        };
        (LoadCursorW(None, resource).unwrap_or(HCURSOR(0)), false)
    }

    unsafe fn replace_tool_cursor(state: &mut OverlayState, apply_now: bool) {
        let (cursor, owned) = create_tool_cursor(state.tool, state.width);
        if cursor.0 == 0 {
            return;
        }
        let previous = state.cursor;
        let previous_owned = state.cursor_owned;
        state.cursor = cursor;
        state.cursor_owned = owned;
        if apply_now {
            SetCursor(cursor);
        }
        if previous_owned && previous.0 != 0 && previous != cursor {
            let _ = DestroyCursor(previous);
        }
    }

    unsafe fn set_overlay_cursor(hwnd: HWND) -> bool {
        let Ok(state_guard) = overlay_state().lock() else {
            return false;
        };
        let Some(state) = state_guard.as_ref() else {
            return false;
        };
        if state.hwnd != hwnd || !state.visible || state.cursor.0 == 0 {
            return false;
        }
        SetCursor(state.cursor);
        true
    }

    unsafe fn create_pen_cursor() -> Option<HCURSOR> {
        create_argb_cursor(40, 4, 34, |dc| {
            let outline = CreatePen(PS_SOLID, 2, COLORREF(0x0018_1818));
            let fill = CreateSolidBrush(COLORREF(0x00f4_f4f4));
            let old_pen = SelectObject(dc, outline);
            let old_brush = SelectObject(dc, fill);
            let body = [
                WinPoint { x: 5, y: 32 },
                WinPoint { x: 9, y: 22 },
                WinPoint { x: 26, y: 5 },
                WinPoint { x: 34, y: 13 },
                WinPoint { x: 17, y: 30 },
            ];
            let _ = Polygon(dc, &body);
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
            DeleteObject(fill);
            DeleteObject(outline);

            let tip_brush = CreateSolidBrush(COLORREF(0x0018_1818));
            let old_tip_brush = SelectObject(dc, tip_brush);
            let tip = [
                WinPoint { x: 5, y: 32 },
                WinPoint { x: 9, y: 22 },
                WinPoint { x: 13, y: 28 },
            ];
            let _ = Polygon(dc, &tip);
            SelectObject(dc, old_tip_brush);
            DeleteObject(tip_brush);
        })
    }

    unsafe fn create_eraser_cursor(width: i32) -> Option<HCURSOR> {
        let diameter = (width.clamp(1, 15) * ERASER_WIDTH_MULTIPLIER).clamp(12, 180);
        let size = diameter + 20;
        let center = size / 2;
        create_argb_cursor(size, center as u32, center as u32, |dc| {
            let circle_pen = CreatePen(PS_DOT, 1, COLORREF(0x0070_7070));
            let old_pen = SelectObject(dc, circle_pen);
            let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
            let radius = diameter / 2;
            Ellipse(
                dc,
                center - radius,
                center - radius,
                center + radius,
                center + radius,
            );
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
            DeleteObject(circle_pen);

            let eraser_pen = CreatePen(PS_SOLID, 2, COLORREF(0x0018_1818));
            let eraser_brush = CreateSolidBrush(COLORREF(0x00f4_f4f4));
            let old_eraser_pen = SelectObject(dc, eraser_pen);
            let old_eraser_brush = SelectObject(dc, eraser_brush);
            let eraser = [
                WinPoint {
                    x: center - 10,
                    y: center + 2,
                },
                WinPoint {
                    x: center - 3,
                    y: center - 8,
                },
                WinPoint {
                    x: center + 11,
                    y: center + 1,
                },
                WinPoint {
                    x: center + 4,
                    y: center + 11,
                },
            ];
            let _ = Polygon(dc, &eraser);
            MoveToEx(dc, center - 3, center - 8, None);
            LineTo(dc, center + 4, center + 11);
            SelectObject(dc, old_eraser_brush);
            SelectObject(dc, old_eraser_pen);
            DeleteObject(eraser_brush);
            DeleteObject(eraser_pen);
        })
    }

    unsafe fn create_argb_cursor<F>(
        size: i32,
        hotspot_x: u32,
        hotspot_y: u32,
        draw: F,
    ) -> Option<HCURSOR>
    where
        F: FnOnce(HDC),
    {
        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            return None;
        }
        let memory_dc = CreateCompatibleDC(screen_dc);
        if memory_dc.0 == 0 {
            ReleaseDC(HWND(0), screen_dc);
            return None;
        }

        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(color_bitmap) = CreateDIBSection(
            memory_dc,
            &mut bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            HANDLE(0),
            0,
        ) else {
            DeleteDC(memory_dc);
            ReleaseDC(HWND(0), screen_dc);
            return None;
        };
        if bits.is_null() {
            DeleteObject(color_bitmap);
            DeleteDC(memory_dc);
            ReleaseDC(HWND(0), screen_dc);
            return None;
        }

        let old_bitmap = SelectObject(memory_dc, color_bitmap);
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize);
        pixels.fill(0);
        SetBkMode(memory_dc, TRANSPARENT);
        draw(HDC(memory_dc.0));
        for pixel in pixels.iter_mut() {
            if (*pixel & 0x00ff_ffff) != 0 {
                *pixel |= 0xff00_0000;
            }
        }
        SelectObject(memory_dc, old_bitmap);

        let mask_stride = ((size as usize + 15) / 16) * 2;
        let mask_bits = vec![0u8; mask_stride * size as usize];
        let mask_bitmap = CreateBitmap(size, size, 1, 1, Some(mask_bits.as_ptr() as *const c_void));
        let icon_info = ICONINFO {
            fIcon: false.into(),
            xHotspot: hotspot_x,
            yHotspot: hotspot_y,
            hbmMask: mask_bitmap,
            hbmColor: color_bitmap,
        };
        let cursor = CreateIconIndirect(&icon_info)
            .ok()
            .map(|icon| HCURSOR(icon.0));

        DeleteObject(mask_bitmap);
        DeleteObject(color_bitmap);
        DeleteDC(memory_dc);
        ReleaseDC(HWND(0), screen_dc);
        cursor
    }

    unsafe fn apply_click_through(hwnd: HWND, enabled: bool) {
        let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if enabled {
            style |= WS_EX_TRANSPARENT.0;
            style |= WS_EX_NOACTIVATE.0;
        } else {
            style &= !WS_EX_TRANSPARENT.0;
            style &= !WS_EX_NOACTIVATE.0;
        }
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style as isize);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }

    fn parse_color(value: &str) -> COLORREF {
        let value = value.trim_start_matches('#');
        let parsed = u32::from_str_radix(value, 16).unwrap_or(0xef2b2d);
        let red = (parsed >> 16) & 0xff;
        let green = (parsed >> 8) & 0xff;
        let blue = parsed & 0xff;
        COLORREF(red | (green << 8) | (blue << 16))
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(once(0)).collect()
    }

    fn emit_history(app: &AppHandle, can_undo: bool) {
        let _ = app.emit_to(
            "drawing-toolbar",
            "drawing-history",
            DrawingHistoryPayload { can_undo },
        );
    }

    fn emit_width(app: &AppHandle, width: i32) {
        let _ = app.emit_to(
            "drawing-toolbar",
            "drawing-width-changed",
            DrawingWidthPayload { width },
        );
    }

    fn lparam_point(lparam: LPARAM) -> Point {
        let x = (lparam.0 as u32 & 0xffff) as i16 as i32;
        let y = ((lparam.0 as u32 >> 16) & 0xffff) as i16 as i32;
        Point { x, y }
    }
}

#[cfg(target_os = "windows")]
pub use platform::*;

#[cfg(not(target_os = "windows"))]
mod platform_stub {
    use tauri::AppHandle;

    #[derive(Clone, Default)]
    pub struct NativeDrawingOverlay;

    #[derive(Clone)]
    pub enum NativeTool {
        Pointer,
        Pen,
        Eraser,
        Line,
        Arrow,
        Rectangle,
        Ellipse,
        Text,
    }

    impl NativeDrawingOverlay {
        pub fn new(_app: &AppHandle) -> Self {
            Self
        }
        pub fn show(
            &self,
            _left: i32,
            _top: i32,
            _width: i32,
            _height: i32,
            _toolbar_passthrough: Option<(i32, i32, i32, i32)>,
        ) {
        }
        pub fn hide(&self) {}
        pub fn set_tool(&self, _tool: NativeTool) {}
        pub fn set_color(&self, _color: &str) {}
        pub fn set_width(&self, _width: i32) {}
        pub fn clear(&self) {}
        pub fn undo(&self) {}
        pub fn set_click_through(&self, _enabled: bool) {}
        pub fn set_toolbar_passthrough(&self, _bounds: Option<(i32, i32, i32, i32)>) {}
        pub fn focus(&self) {}
        pub fn raise(&self) {}
        pub fn resize(&self, _left: i32, _top: i32, _width: i32, _height: i32) {}
    }

    pub fn parse_tool(_value: &str) -> Option<NativeTool> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub use platform_stub::*;
