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
                DIB_RGB_COLORS, DT_LEFT, DT_SINGLELINE, DT_TOP, HDC, HOLLOW_BRUSH, NULL_PEN,
                PS_DOT, PS_SOLID, TRANSPARENT,
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
    const SELECTION_HANDLE_SIZE: i32 = 7;
    const ROTATION_HANDLE_OFFSET: i32 = 24;

    #[derive(Clone)]
    pub struct NativeDrawingOverlay {
        sender: Option<Sender<DrawingCommand>>,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum NativeTool {
        Pointer,
        Select,
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
        ToggleGroup,
        DeleteSelectionOrClear,
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
            rotation: f64,
            group: Option<u64>,
        },
        Shape {
            tool: NativeTool,
            start: Point,
            end: Point,
            color: COLORREF,
            width: i32,
            rotation: f64,
            group: Option<u64>,
        },
        Text {
            start: Point,
            text: String,
            color: COLORREF,
            width: i32,
            rotation: f64,
            group: Option<u64>,
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

    enum SelectionAction {
        Marquee,
        Move {
            last: Point,
        },
        Resize {
            anchor: Point,
            angle: f64,
            originals: Vec<(usize, DrawingItem)>,
        },
        Rotate {
            center: Point,
            start_angle: f64,
            originals: Vec<(usize, DrawingItem)>,
        },
    }

    struct SelectionSession {
        start: Point,
        current: Point,
        action: SelectionAction,
    }

    #[derive(Clone, Copy)]
    struct SelectionFrame {
        corners: [Point; 4],
        center: Point,
        angle: f64,
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
        selected: Vec<usize>,
        selection: Option<SelectionSession>,
        next_group_id: u64,
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

    #[derive(Clone, Serialize)]
    struct DrawingSelectionPayload {
        count: usize,
        grouped: bool,
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

        pub fn toggle_group(&self) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::ToggleGroup);
        }

        pub fn delete_selection_or_clear(&self) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::DeleteSelectionOrClear);
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
            "select" => Some(NativeTool::Select),
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
                    selected: Vec::new(),
                    selection: None,
                    next_group_id: 1,
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
                state.selected.clear();
                state.selection = None;
                state.visible = false;
                ShowWindow(state.hwnd, SW_HIDE);
                emit_history(&state.app, false);
                emit_selection_state(state);
            }
            DrawingCommand::SetTool(tool) => {
                commit_text_editor(state);
                let click_through = matches!(tool, NativeTool::Pointer);
                state.tool = tool;
                state.selected.clear();
                state.selection = None;
                state.click_through = click_through;
                apply_click_through(state.hwnd, click_through);
                replace_tool_cursor(state, false);
                if state.visible {
                    raise_toolbar(&state.app);
                }
                refresh_overlay(state);
                emit_selection_state(state);
            }
            DrawingCommand::SetColor(color) => state.color = parse_color(&color),
            DrawingCommand::SetWidth(width) => {
                let width = width.clamp(1, 15);
                state.width = width;
                if matches!(state.tool, NativeTool::Eraser) {
                    replace_tool_cursor(state, false);
                }
            }
            DrawingCommand::Clear => {
                cancel_text_editor(state);
                state.drawings.clear();
                state.active = None;
                state.selected.clear();
                state.selection = None;
                emit_history(&state.app, false);
                emit_selection_state(state);
                refresh_overlay(state);
            }
            DrawingCommand::Undo => {
                if state.edit.is_none() {
                    state.selected.clear();
                    state.selection = None;
                    state.drawings.pop();
                    emit_history(&state.app, !state.drawings.is_empty());
                    emit_selection_state(state);
                    refresh_overlay(state);
                }
            }
            DrawingCommand::ToggleGroup => {
                toggle_selected_group(state);
                emit_selection_state(state);
                refresh_overlay(state);
            }
            DrawingCommand::DeleteSelectionOrClear => {
                if matches!(state.tool, NativeTool::Select) {
                    delete_selected(state);
                } else {
                    state.drawings.clear();
                    state.active = None;
                    state.selected.clear();
                    state.selection = None;
                }
                emit_history(&state.app, !state.drawings.is_empty());
                emit_selection_state(state);
                refresh_overlay(state);
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
        if matches!(state.tool, NativeTool::Select) {
            begin_selection_at(state, point);
            if let Some(hwnd) = capture_hwnd {
                SetCapture(hwnd);
            }
            refresh_overlay(state);
            return;
        }

        state.selected.clear();
        state.selection = None;
        match state.tool.clone() {
            NativeTool::Pointer | NativeTool::Select => {}
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
        if state.hwnd != hwnd || (state.active.is_none() && state.selection.is_none()) {
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
        if matches!(state.tool, NativeTool::Select) && state.selection.is_some() {
            update_selection_at(state, point);
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
        if matches!(state.tool, NativeTool::Select) && state.selection.is_some() {
            finish_selection(state);
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
                    rotation: 0.0,
                    group: None,
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
                rotation: 0.0,
                group: None,
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
        if matches!(state.tool, NativeTool::Text) && state.edit.is_some() {
            let width = (state.width + step).clamp(1, 15);
            state.width = width;
            if let Some(edit) = state.edit.as_mut() {
                edit.width = width;
            }
            emit_width(&state.app, width);
            refresh_overlay(state);
            return;
        }
        if matches!(state.tool, NativeTool::Eraser) {
            let width = (state.width + step).clamp(1, 15);
            state.width = width;
            replace_tool_cursor(state, true);
            emit_width(&state.app, width);
            refresh_overlay(state);
            return;
        }
        // Object editing is intentionally limited to the selection tool.
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
        if !state.selected.is_empty() {
            draw_selection(drawing_dc, state);
        }
        if let Some(SelectionSession {
            start,
            current,
            action: SelectionAction::Marquee,
        }) = state.selection.as_ref()
        {
            draw_marquee(drawing_dc, rect_from_points(*start, *current));
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
                0.0,
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

    fn drawing_group(drawing: &DrawingItem) -> Option<u64> {
        match drawing {
            DrawingItem::Stroke { group, .. }
            | DrawingItem::Shape { group, .. }
            | DrawingItem::Text { group, .. } => *group,
        }
    }

    fn set_drawing_group(drawing: &mut DrawingItem, value: Option<u64>) {
        match drawing {
            DrawingItem::Stroke { group, .. }
            | DrawingItem::Shape { group, .. }
            | DrawingItem::Text { group, .. } => *group = value,
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
        let value = value.clamp(1, 100);
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
            .find(|(_, drawing)| drawing_hit_test(drawing, point))
            .map(|(index, _)| index)
    }

    fn grouped_indices(state: &OverlayState, index: usize) -> Vec<usize> {
        let Some(group) = state.drawings.get(index).and_then(drawing_group) else {
            return vec![index];
        };
        state
            .drawings
            .iter()
            .enumerate()
            .filter_map(|(item_index, drawing)| {
                (drawing_group(drawing) == Some(group)).then_some(item_index)
            })
            .collect()
    }

    fn expand_selected_groups(state: &OverlayState, selected: &mut Vec<usize>) {
        let groups: Vec<u64> = selected
            .iter()
            .filter_map(|index| state.drawings.get(*index).and_then(drawing_group))
            .collect();
        for (index, drawing) in state.drawings.iter().enumerate() {
            if drawing_group(drawing).is_some_and(|group| groups.contains(&group))
                && !selected.contains(&index)
            {
                selected.push(index);
            }
        }
        selected.sort_unstable();
        selected.dedup();
    }

    fn toggle_selected_group(state: &mut OverlayState) {
        if state.selected.is_empty() {
            return;
        }
        let common_group = selected_common_group(state);
        if let Some(group) = common_group {
            for drawing in state.drawings.iter_mut() {
                if drawing_group(drawing) == Some(group) {
                    set_drawing_group(drawing, None);
                }
            }
        } else if state.selected.len() >= 2 {
            let group = state.next_group_id;
            state.next_group_id = state.next_group_id.wrapping_add(1).max(1);
            for index in state.selected.iter().copied() {
                if let Some(drawing) = state.drawings.get_mut(index) {
                    set_drawing_group(drawing, Some(group));
                }
            }
        }
    }

    fn selected_common_group(state: &OverlayState) -> Option<u64> {
        state
            .selected
            .first()
            .and_then(|index| state.drawings.get(*index))
            .and_then(drawing_group)
            .filter(|group| {
                state
                    .selected
                    .iter()
                    .all(|index| state.drawings.get(*index).and_then(drawing_group) == Some(*group))
            })
    }

    fn delete_selected(state: &mut OverlayState) {
        if state.selected.is_empty() {
            return;
        }
        let mut selected = state.selected.clone();
        selected.sort_unstable_by(|left, right| right.cmp(left));
        selected.dedup();
        for index in selected {
            if index < state.drawings.len() {
                state.drawings.remove(index);
            }
        }
        state.selected.clear();
        state.selection = None;
    }

    fn selection_bounds(state: &OverlayState) -> Option<RECT> {
        let mut bounds = state
            .selected
            .iter()
            .filter_map(|index| state.drawings.get(*index).map(drawing_bounds));
        let first = bounds.next()?;
        Some(bounds.fold(first, |combined, item| RECT {
            left: combined.left.min(item.left),
            top: combined.top.min(item.top),
            right: combined.right.max(item.right),
            bottom: combined.bottom.max(item.bottom),
        }))
    }

    fn selection_frame(state: &OverlayState) -> Option<SelectionFrame> {
        let first = state
            .selected
            .iter()
            .find_map(|index| state.drawings.get(*index))?;
        let angle = drawing_orientation(first);
        let cos = angle.cos();
        let sin = angle.sin();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut padding = 0.0_f64;

        for drawing in state
            .selected
            .iter()
            .filter_map(|index| state.drawings.get(*index))
        {
            padding = padding.max(drawing_selection_padding(drawing));
            for point in drawing_frame_points(drawing) {
                let local_x = point.x as f64 * cos + point.y as f64 * sin;
                let local_y = -point.x as f64 * sin + point.y as f64 * cos;
                min_x = min_x.min(local_x);
                min_y = min_y.min(local_y);
                max_x = max_x.max(local_x);
                max_y = max_y.max(local_y);
            }
        }
        if !min_x.is_finite() {
            return None;
        }
        min_x -= padding;
        min_y -= padding;
        max_x += padding;
        max_y += padding;

        let corners = [
            selection_local_to_world(min_x, min_y, angle),
            selection_local_to_world(max_x, min_y, angle),
            selection_local_to_world(max_x, max_y, angle),
            selection_local_to_world(min_x, max_y, angle),
        ];
        Some(SelectionFrame {
            corners,
            center: selection_local_to_world((min_x + max_x) / 2.0, (min_y + max_y) / 2.0, angle),
            angle,
        })
    }

    fn drawing_orientation(drawing: &DrawingItem) -> f64 {
        match drawing {
            DrawingItem::Stroke { rotation, .. } | DrawingItem::Text { rotation, .. } => *rotation,
            DrawingItem::Shape {
                tool,
                start,
                end,
                rotation,
                ..
            } => {
                if matches!(tool, NativeTool::Rectangle | NativeTool::Ellipse) {
                    *rotation
                } else {
                    angle_between(*start, *end)
                }
            }
        }
    }

    fn drawing_frame_points(drawing: &DrawingItem) -> Vec<Point> {
        match drawing {
            DrawingItem::Stroke { points, .. } => points.clone(),
            DrawingItem::Shape {
                tool,
                start,
                end,
                rotation,
                ..
            } if matches!(tool, NativeTool::Rectangle | NativeTool::Ellipse) => {
                rotated_shape_corners(*start, *end, *rotation).to_vec()
            }
            DrawingItem::Shape { start, end, .. } => vec![*start, *end],
            DrawingItem::Text {
                start,
                text,
                width,
                rotation,
                ..
            } => text_corners(*start, text, *width, *rotation).to_vec(),
        }
    }

    fn drawing_selection_padding(drawing: &DrawingItem) -> f64 {
        match drawing {
            DrawingItem::Stroke { width, .. } => (*width).max(1) as f64 + 5.0,
            DrawingItem::Shape { tool, width, .. } => {
                let padding = (*width).max(1) as f64 + 5.0;
                if matches!(tool, NativeTool::Arrow) {
                    padding.max(22.0)
                } else {
                    padding
                }
            }
            DrawingItem::Text { .. } => 5.0,
        }
    }

    fn selection_local_to_world(x: f64, y: f64, angle: f64) -> Point {
        let cos = angle.cos();
        let sin = angle.sin();
        Point {
            x: (x * cos - y * sin).round() as i32,
            y: (x * sin + y * cos).round() as i32,
        }
    }

    fn point_near(point: Point, target: Point, radius: i32) -> bool {
        (point.x - target.x).abs() <= radius && (point.y - target.y).abs() <= radius
    }

    fn selection_resize_anchor(frame: SelectionFrame, point: Point) -> Option<Point> {
        frame
            .corners
            .iter()
            .enumerate()
            .find_map(|(index, handle)| {
                point_near(point, *handle, SELECTION_HANDLE_SIZE + 3)
                    .then_some(frame.corners[(index + 2) % 4])
            })
    }

    fn rotation_handle(frame: SelectionFrame) -> Point {
        let top_center = Point {
            x: (frame.corners[0].x + frame.corners[1].x) / 2,
            y: (frame.corners[0].y + frame.corners[1].y) / 2,
        };
        let dx = (top_center.x - frame.center.x) as f64;
        let dy = (top_center.y - frame.center.y) as f64;
        let length = (dx * dx + dy * dy).sqrt().max(1.0);
        Point {
            x: (top_center.x as f64 + dx / length * ROTATION_HANDLE_OFFSET as f64).round() as i32,
            y: (top_center.y as f64 + dy / length * ROTATION_HANDLE_OFFSET as f64).round() as i32,
        }
    }

    fn selected_originals(state: &OverlayState) -> Vec<(usize, DrawingItem)> {
        state
            .selected
            .iter()
            .filter_map(|index| {
                state
                    .drawings
                    .get(*index)
                    .cloned()
                    .map(|item| (*index, item))
            })
            .collect()
    }

    fn begin_selection_at(state: &mut OverlayState, point: Point) {
        if let Some(frame) = selection_frame(state) {
            let rotate_handle = rotation_handle(frame);
            if point_near(point, rotate_handle, SELECTION_HANDLE_SIZE + 4) {
                state.selection = Some(SelectionSession {
                    start: point,
                    current: point,
                    action: SelectionAction::Rotate {
                        center: frame.center,
                        start_angle: angle_between(frame.center, point),
                        originals: selected_originals(state),
                    },
                });
                return;
            }
            if let Some(anchor) = selection_resize_anchor(frame, point) {
                state.selection = Some(SelectionSession {
                    start: point,
                    current: point,
                    action: SelectionAction::Resize {
                        anchor,
                        angle: frame.angle,
                        originals: selected_originals(state),
                    },
                });
                return;
            }
        }

        if let Some(index) = hit_test_drawing(state, point) {
            if !state.selected.contains(&index) {
                state.selected = grouped_indices(state, index);
            }
            state.selection = Some(SelectionSession {
                start: point,
                current: point,
                action: SelectionAction::Move { last: point },
            });
        } else {
            state.selected.clear();
            state.selection = Some(SelectionSession {
                start: point,
                current: point,
                action: SelectionAction::Marquee,
            });
        }
    }

    fn update_selection_at(state: &mut OverlayState, point: Point) {
        let Some(mut session) = state.selection.take() else {
            return;
        };
        session.current = point;
        match &mut session.action {
            SelectionAction::Marquee => {
                let marquee = rect_from_points(session.start, point);
                let mut selected: Vec<usize> = state
                    .drawings
                    .iter()
                    .enumerate()
                    .filter_map(|(index, drawing)| {
                        rects_intersect(marquee, drawing_bounds(drawing)).then_some(index)
                    })
                    .collect();
                expand_selected_groups(state, &mut selected);
                state.selected = selected;
            }
            SelectionAction::Move { last } => {
                let dx = point.x - last.x;
                let dy = point.y - last.y;
                if dx != 0 || dy != 0 {
                    for index in state.selected.iter().copied() {
                        if let Some(drawing) = state.drawings.get_mut(index) {
                            translate_drawing(drawing, dx, dy);
                        }
                    }
                    *last = point;
                }
            }
            SelectionAction::Resize {
                anchor,
                angle,
                originals,
            } => {
                let base = point_to_selection_local(session.start, *anchor, *angle);
                let current = point_to_selection_local(point, *anchor, *angle);
                let scale_x = safe_scale(current.0, base.0);
                let scale_y = safe_scale(current.1, base.1);
                let width_scale = ((scale_x.abs() + scale_y.abs()) / 2.0).max(0.05);
                for (index, original) in originals.iter() {
                    if let Some(drawing) = state.drawings.get_mut(*index) {
                        *drawing =
                            scale_drawing(original, *anchor, scale_x, scale_y, width_scale, *angle);
                    }
                }
            }
            SelectionAction::Rotate {
                center,
                start_angle,
                originals,
            } => {
                let delta = angle_between(*center, point) - *start_angle;
                for (index, original) in originals.iter() {
                    if let Some(drawing) = state.drawings.get_mut(*index) {
                        *drawing = rotate_drawing(original, *center, delta);
                    }
                }
            }
        }
        state.selection = Some(session);
    }

    fn finish_selection(state: &mut OverlayState) {
        state.selection = None;
        emit_selection_state(state);
    }

    fn safe_scale(value: f64, base: f64) -> f64 {
        if base.abs() < 1.0 {
            1.0
        } else {
            (value / base).max(0.05)
        }
    }

    fn rect_from_points(start: Point, end: Point) -> RECT {
        RECT {
            left: start.x.min(end.x),
            top: start.y.min(end.y),
            right: start.x.max(end.x),
            bottom: start.y.max(end.y),
        }
    }

    fn rects_intersect(left: RECT, right: RECT) -> bool {
        left.left <= right.right
            && left.right >= right.left
            && left.top <= right.bottom
            && left.bottom >= right.top
    }

    fn angle_between(center: Point, point: Point) -> f64 {
        ((point.y - center.y) as f64).atan2((point.x - center.x) as f64)
    }

    fn transform_point(point: Point, anchor: Point, scale_x: f64, scale_y: f64) -> Point {
        Point {
            x: (anchor.x as f64 + (point.x - anchor.x) as f64 * scale_x).round() as i32,
            y: (anchor.y as f64 + (point.y - anchor.y) as f64 * scale_y).round() as i32,
        }
    }

    fn point_to_selection_local(point: Point, anchor: Point, angle: f64) -> (f64, f64) {
        let dx = (point.x - anchor.x) as f64;
        let dy = (point.y - anchor.y) as f64;
        let cos = angle.cos();
        let sin = angle.sin();
        (dx * cos + dy * sin, -dx * sin + dy * cos)
    }

    fn transform_point_oriented(
        point: Point,
        anchor: Point,
        scale_x: f64,
        scale_y: f64,
        angle: f64,
    ) -> Point {
        let (local_x, local_y) = point_to_selection_local(point, anchor, angle);
        let cos = angle.cos();
        let sin = angle.sin();
        Point {
            x: (anchor.x as f64 + local_x * scale_x * cos - local_y * scale_y * sin).round() as i32,
            y: (anchor.y as f64 + local_x * scale_x * sin + local_y * scale_y * cos).round() as i32,
        }
    }

    fn transformed_angle(rotation: f64, scale_x: f64, scale_y: f64, angle: f64) -> f64 {
        let vector_x = rotation.cos();
        let vector_y = rotation.sin();
        let cos = angle.cos();
        let sin = angle.sin();
        let local_x = vector_x * cos + vector_y * sin;
        let local_y = -vector_x * sin + vector_y * cos;
        let world_x = local_x * scale_x * cos - local_y * scale_y * sin;
        let world_y = local_x * scale_x * sin + local_y * scale_y * cos;
        world_y.atan2(world_x)
    }

    fn rotate_point(point: Point, center: Point, angle: f64) -> Point {
        let x = (point.x - center.x) as f64;
        let y = (point.y - center.y) as f64;
        let cos = angle.cos();
        let sin = angle.sin();
        Point {
            x: (center.x as f64 + x * cos - y * sin).round() as i32,
            y: (center.y as f64 + x * sin + y * cos).round() as i32,
        }
    }

    fn scale_drawing(
        original: &DrawingItem,
        anchor: Point,
        scale_x: f64,
        scale_y: f64,
        width_scale: f64,
        angle: f64,
    ) -> DrawingItem {
        let mut drawing = original.clone();
        match &mut drawing {
            DrawingItem::Stroke {
                points,
                width,
                rotation,
                ..
            } => {
                for point in points {
                    *point = transform_point_oriented(*point, anchor, scale_x, scale_y, angle);
                }
                *width = ((*width as f64 * width_scale).round() as i32).clamp(1, 100);
                *rotation = transformed_angle(*rotation, scale_x, scale_y, angle);
            }
            DrawingItem::Shape {
                tool,
                start,
                end,
                width,
                rotation,
                ..
            } => {
                if matches!(tool, NativeTool::Rectangle | NativeTool::Ellipse) {
                    let center = Point {
                        x: (start.x + end.x) / 2,
                        y: (start.y + end.y) / 2,
                    };
                    let half_width = (end.x - start.x).abs() / 2;
                    let half_height = (end.y - start.y).abs() / 2;
                    let x_handle = rotate_point(
                        Point {
                            x: center.x + half_width,
                            y: center.y,
                        },
                        center,
                        *rotation,
                    );
                    let y_handle = rotate_point(
                        Point {
                            x: center.x,
                            y: center.y + half_height,
                        },
                        center,
                        *rotation,
                    );
                    let new_center =
                        transform_point_oriented(center, anchor, scale_x, scale_y, angle);
                    let new_x = transform_point_oriented(x_handle, anchor, scale_x, scale_y, angle);
                    let new_y = transform_point_oriented(y_handle, anchor, scale_x, scale_y, angle);
                    let new_half_width = distance_between(new_center, new_x).round() as i32;
                    let new_half_height = distance_between(new_center, new_y).round() as i32;
                    *rotation = angle_between(new_center, new_x);
                    *start = Point {
                        x: new_center.x - new_half_width,
                        y: new_center.y - new_half_height,
                    };
                    *end = Point {
                        x: new_center.x + new_half_width,
                        y: new_center.y + new_half_height,
                    };
                } else {
                    *start = transform_point_oriented(*start, anchor, scale_x, scale_y, angle);
                    *end = transform_point_oriented(*end, anchor, scale_x, scale_y, angle);
                }
                *width = ((*width as f64 * width_scale).round() as i32).clamp(1, 100);
            }
            DrawingItem::Text {
                start,
                width,
                rotation,
                ..
            } => {
                *start = transform_point_oriented(*start, anchor, scale_x, scale_y, angle);
                *width = ((*width as f64 * width_scale).round() as i32).clamp(1, 100);
                *rotation = transformed_angle(*rotation, scale_x, scale_y, angle);
            }
        }
        drawing
    }

    fn distance_between(left: Point, right: Point) -> f64 {
        let dx = (right.x - left.x) as f64;
        let dy = (right.y - left.y) as f64;
        (dx * dx + dy * dy).sqrt()
    }

    fn rotate_drawing(original: &DrawingItem, center: Point, angle: f64) -> DrawingItem {
        let mut drawing = original.clone();
        match &mut drawing {
            DrawingItem::Stroke {
                points, rotation, ..
            } => {
                for point in points {
                    *point = rotate_point(*point, center, angle);
                }
                *rotation += angle;
            }
            DrawingItem::Shape {
                tool,
                start,
                end,
                rotation,
                ..
            } => {
                if matches!(tool, NativeTool::Rectangle | NativeTool::Ellipse) {
                    let old_center = Point {
                        x: (start.x + end.x) / 2,
                        y: (start.y + end.y) / 2,
                    };
                    let new_center = rotate_point(old_center, center, angle);
                    translate_point_pair(
                        start,
                        end,
                        new_center.x - old_center.x,
                        new_center.y - old_center.y,
                    );
                    *rotation += angle;
                } else {
                    *start = rotate_point(*start, center, angle);
                    *end = rotate_point(*end, center, angle);
                }
            }
            DrawingItem::Text {
                start, rotation, ..
            } => {
                *start = rotate_point(*start, center, angle);
                *rotation += angle;
            }
        }
        drawing
    }

    fn translate_point_pair(start: &mut Point, end: &mut Point, dx: i32, dy: i32) {
        start.x += dx;
        start.y += dy;
        end.x += dx;
        end.y += dy;
    }

    fn drawing_hit_test(drawing: &DrawingItem, point: Point) -> bool {
        let bounds = drawing_bounds(drawing);
        point.x >= bounds.left - 4
            && point.x <= bounds.right + 4
            && point.y >= bounds.top - 4
            && point.y <= bounds.bottom + 4
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
                tool,
                start,
                end,
                width,
                rotation,
                ..
            } => {
                let padding = (*width).max(1) + 5;
                let points = if matches!(tool, NativeTool::Rectangle | NativeTool::Ellipse) {
                    rotated_shape_corners(*start, *end, *rotation).to_vec()
                } else {
                    vec![*start, *end]
                };
                RECT {
                    left: points.iter().map(|point| point.x).min().unwrap_or(0) - padding,
                    top: points.iter().map(|point| point.y).min().unwrap_or(0) - padding,
                    right: points.iter().map(|point| point.x).max().unwrap_or(0) + padding,
                    bottom: points.iter().map(|point| point.y).max().unwrap_or(0) + padding,
                }
            }
            DrawingItem::Text {
                start,
                text,
                width,
                rotation,
                ..
            } => {
                let corners = text_corners(*start, text, *width, *rotation);
                RECT {
                    left: corners.iter().map(|point| point.x).min().unwrap_or(start.x) - 5,
                    top: corners.iter().map(|point| point.y).min().unwrap_or(start.y) - 5,
                    right: corners.iter().map(|point| point.x).max().unwrap_or(start.x) + 5,
                    bottom: corners.iter().map(|point| point.y).max().unwrap_or(start.y) + 5,
                }
            }
        }
    }

    fn text_corners(start: Point, text: &str, width: i32, rotation: f64) -> [Point; 4] {
        let font_size = text_font_size(width);
        let units: f64 = text
            .chars()
            .map(|character| if character.is_ascii() { 0.62 } else { 1.0 })
            .sum();
        let text_width = (units * font_size as f64).ceil() as i32;
        [
            start,
            Point {
                x: start.x + text_width,
                y: start.y,
            },
            Point {
                x: start.x + text_width,
                y: start.y + font_size,
            },
            Point {
                x: start.x,
                y: start.y + font_size,
            },
        ]
        .map(|point| rotate_point(point, start, rotation))
    }

    fn rotated_shape_corners(start: Point, end: Point, rotation: f64) -> [Point; 4] {
        let center = Point {
            x: (start.x + end.x) / 2,
            y: (start.y + end.y) / 2,
        };
        [
            Point {
                x: start.x,
                y: start.y,
            },
            Point {
                x: end.x,
                y: start.y,
            },
            Point { x: end.x, y: end.y },
            Point {
                x: start.x,
                y: end.y,
            },
        ]
        .map(|point| rotate_point(point, center, rotation))
    }

    unsafe fn draw_selection(dc: HDC, state: &OverlayState) {
        let Some(frame) = selection_frame(state) else {
            return;
        };
        let pen = CreatePen(PS_DOT, 1, COLORREF(0x0080_8080));
        let old_pen = SelectObject(dc, pen);
        let brush = GetStockObject(HOLLOW_BRUSH);
        let old_brush = SelectObject(dc, brush);
        MoveToEx(dc, frame.corners[0].x, frame.corners[0].y, None);
        for corner in frame.corners.iter().skip(1) {
            LineTo(dc, corner.x, corner.y);
        }
        LineTo(dc, frame.corners[0].x, frame.corners[0].y);

        let top_center = Point {
            x: (frame.corners[0].x + frame.corners[1].x) / 2,
            y: (frame.corners[0].y + frame.corners[1].y) / 2,
        };
        let rotation = rotation_handle(frame);
        MoveToEx(dc, top_center.x, top_center.y, None);
        LineTo(dc, rotation.x, rotation.y);
        Ellipse(
            dc,
            rotation.x - SELECTION_HANDLE_SIZE,
            rotation.y - SELECTION_HANDLE_SIZE,
            rotation.x + SELECTION_HANDLE_SIZE,
            rotation.y + SELECTION_HANDLE_SIZE,
        );

        let handle_brush = CreateSolidBrush(COLORREF(0x00ff_ffff));
        SelectObject(dc, handle_brush);
        for handle in frame.corners {
            Rectangle(
                dc,
                handle.x - SELECTION_HANDLE_SIZE,
                handle.y - SELECTION_HANDLE_SIZE,
                handle.x + SELECTION_HANDLE_SIZE,
                handle.y + SELECTION_HANDLE_SIZE,
            );
        }
        SelectObject(dc, brush);
        DeleteObject(handle_brush);
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(pen);
    }

    unsafe fn draw_marquee(dc: HDC, bounds: RECT) {
        let pen = CreatePen(PS_DOT, 1, COLORREF(0x0040_4040));
        let old_pen = SelectObject(dc, pen);
        let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
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
                ..
            } => draw_polyline(dc, points, *color, *width),
            DrawingItem::Shape {
                tool,
                start,
                end,
                color,
                width,
                rotation,
                ..
            } => draw_shape(dc, tool.clone(), *start, *end, *color, *width, *rotation),
            DrawingItem::Text {
                start,
                text,
                color,
                width,
                rotation,
                ..
            } => draw_text(dc, *start, text, *color, *width, *rotation),
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
            } => draw_shape(dc, tool.clone(), *start, *end, *color, *width, 0.0),
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
        rotation: f64,
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
                if rotation.abs() < f64::EPSILON {
                    Rectangle(dc, start.x, start.y, end.x, end.y);
                } else {
                    let corners =
                        rotated_shape_corners(start, end, rotation).map(|point| WinPoint {
                            x: point.x,
                            y: point.y,
                        });
                    let _ = Polygon(dc, &corners);
                }
            }
            NativeTool::Ellipse => {
                if rotation.abs() < f64::EPSILON {
                    Ellipse(dc, start.x, start.y, end.x, end.y);
                } else {
                    let center_x = (start.x + end.x) as f64 / 2.0;
                    let center_y = (start.y + end.y) as f64 / 2.0;
                    let radius_x = (end.x - start.x).abs() as f64 / 2.0;
                    let radius_y = (end.y - start.y).abs() as f64 / 2.0;
                    let cos = rotation.cos();
                    let sin = rotation.sin();
                    let points: Vec<WinPoint> = (0..48)
                        .map(|index| {
                            let angle = std::f64::consts::TAU * index as f64 / 48.0;
                            let x = radius_x * angle.cos();
                            let y = radius_y * angle.sin();
                            WinPoint {
                                x: (center_x + x * cos - y * sin).round() as i32,
                                y: (center_y + x * sin + y * cos).round() as i32,
                            }
                        })
                        .collect();
                    let _ = Polygon(dc, &points);
                }
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
        let head_length = (24.0 + width as f64 * 0.7)
            .clamp(26.0, 36.0)
            .min(distance * 0.38);
        let head_half = (11.0 + width as f64 * 0.9)
            .clamp(14.0, 25.0)
            .min(distance * 0.2);
        let start_half = 0.7;
        let shaft_half = (3.0 + width as f64 * 0.45).clamp(4.0, 10.0);
        let point = |along: f64, normal: f64| WinPoint {
            x: (start.x as f64 + ux * along + nx * normal).round() as i32,
            y: (start.y as f64 + uy * along + ny * normal).round() as i32,
        };
        let head_base = distance - head_length;
        let points = [
            point(0.0, start_half),
            point(head_base, shaft_half),
            point(head_base, head_half),
            WinPoint { x: end.x, y: end.y },
            point(head_base, -head_half),
            point(head_base, -shaft_half),
            point(0.0, -start_half),
        ];
        let brush = CreateSolidBrush(color);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
        let _ = Polygon(dc, &points);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(brush);
    }

    unsafe fn draw_text(
        dc: windows::Win32::Graphics::Gdi::HDC,
        start: Point,
        text: &str,
        color: COLORREF,
        width: i32,
        rotation: f64,
    ) {
        let height = -text_font_size(width);
        let escapement = (rotation.to_degrees() * 10.0).round() as i32;
        let font = CreateFontW(
            height,
            0,
            escapement,
            escapement,
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
                rotation: 0.0,
                group: None,
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
            NativeTool::Pointer | NativeTool::Select => IDC_ARROW,
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
        create_argb_cursor(40, 4, 35, |dc| {
            // Match the toolbar's Lucide pencil: a slim outlined body, a dark
            // graphite tip, and the short diagonal cap separator.
            let outline = CreatePen(PS_SOLID, 2, COLORREF(0x0028_2828));
            let fill = CreateSolidBrush(COLORREF(0x00f8_f8f8));
            let old_pen = SelectObject(dc, outline);
            let old_brush = SelectObject(dc, fill);
            let body = [
                WinPoint { x: 4, y: 35 },
                WinPoint { x: 8, y: 24 },
                WinPoint { x: 27, y: 5 },
                WinPoint { x: 36, y: 14 },
                WinPoint { x: 17, y: 33 },
            ];
            let _ = Polygon(dc, &body);

            MoveToEx(dc, 25, 8, None);
            LineTo(dc, 33, 16);

            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
            DeleteObject(fill);
            DeleteObject(outline);

            let tip_brush = CreateSolidBrush(COLORREF(0x0028_2828));
            let old_tip_brush = SelectObject(dc, tip_brush);
            let old_tip_pen = SelectObject(dc, GetStockObject(NULL_PEN));
            let tip = [
                WinPoint { x: 4, y: 35 },
                WinPoint { x: 8, y: 24 },
                WinPoint { x: 14, y: 30 },
            ];
            let _ = Polygon(dc, &tip);
            SelectObject(dc, old_tip_pen);
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

        let mask_stride = ((size as usize + 15) / 16) * 2;
        // The AND mask is used when a compositor cannot honor the ARGB cursor.
        // Keep transparent pixels set to 1 and clear only the visible icon bits;
        // an all-zero mask makes the cursor's full bitmap appear as a small box.
        let mut mask_bits = vec![0xffu8; mask_stride * size as usize];
        for y in 0..size as usize {
            for x in 0..size as usize {
                if (pixels[y * size as usize + x] >> 24) != 0 {
                    let byte = y * mask_stride + x / 8;
                    mask_bits[byte] &= !(0x80 >> (x % 8));
                }
            }
        }
        SelectObject(memory_dc, old_bitmap);

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

    fn emit_selection_state(state: &OverlayState) {
        let _ = state.app.emit_to(
            "drawing-toolbar",
            "drawing-selection-changed",
            DrawingSelectionPayload {
                count: state.selected.len(),
                grouped: selected_common_group(state).is_some(),
            },
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
        Select,
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
        pub fn toggle_group(&self) {}
        pub fn delete_selection_or_clear(&self) {}
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
