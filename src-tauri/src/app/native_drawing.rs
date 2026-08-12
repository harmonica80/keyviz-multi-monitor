#[cfg(target_os = "windows")]
mod platform {
    use std::{
        collections::VecDeque,
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
                BeginPaint, CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW,
                CreatePen, CreateSolidBrush, CreatedHDC, DeleteDC, DeleteObject, Ellipse, EndPaint,
                GetDC, GetStockObject, GetTextExtentPoint32W, LineTo, MoveToEx, Polygon, Rectangle,
                ReleaseDC, SelectObject, SetBkMode, SetViewportOrgEx, TextOutW, AC_SRC_ALPHA,
                BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC,
                HGDIOBJ, HOLLOW_BRUSH, NULL_PEN, PAINTSTRUCT, PS_DOT, PS_SOLID, TRANSPARENT,
            },
            System::LibraryLoader::GetModuleHandleW,
            UI::{
                Input::KeyboardAndMouse::{
                    ReleaseCapture, SetCapture, SetFocus, VK_BACK, VK_ESCAPE, VK_RETURN,
                },
                WindowsAndMessaging::{
                    CreateIconIndirect, CreateWindowExW, DefWindowProcW, DestroyCursor,
                    DestroyWindow, DispatchMessageW, GetAncestor, LoadCursorW, PeekMessageW,
                    RegisterClassW, SetCursor, SetWindowLongPtrW, SetWindowPos, ShowWindow,
                    TranslateMessage, UpdateLayeredWindow, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW,
                    GA_ROOT, GWLP_USERDATA, HCURSOR, HTCLIENT, HTTRANSPARENT, HWND_TOPMOST,
                    ICONINFO, IDC_ARROW, IDC_CROSS, IDC_IBEAM, MSG, PM_NOREMOVE, PM_REMOVE,
                    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, ULW_ALPHA,
                    WM_APP, WM_CHAR, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN,
                    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCHITTEST,
                    WM_PAINT, WM_SETCURSOR, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
                    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
                    WS_POPUP,
                },
            },
        },
    };

    const TRANSPARENT_KEY: COLORREF = COLORREF(1 | (2 << 8) | (3 << 16));
    const TRANSPARENT_PIXEL: u32 = 0x0001_0203;
    const WM_APP_COMMIT_TEXT: u32 = WM_APP + 1;
    const WM_APP_CANCEL_TEXT: u32 = WM_APP + 2;
    const NON_ANTIALIASED_FONT_QUALITY: u32 = 3;
    const TEXT_PADDING: i32 = 8;
    const MK_LBUTTON_MASK: usize = 0x0001;
    const ERASER_WIDTH_MULTIPLIER: i32 = 12;
    const SELECTION_HANDLE_SIZE: i32 = 7;
    const ROTATION_HANDLE_OFFSET: i32 = 24;

    #[repr(C)]
    struct GdiplusStartupInput {
        version: u32,
        debug_event_callback: *mut c_void,
        suppress_background_thread: i32,
        suppress_external_codecs: i32,
    }

    #[link(name = "gdiplus")]
    extern "system" {
        fn GdiplusStartup(
            token: *mut usize,
            input: *const GdiplusStartupInput,
            output: *mut c_void,
        ) -> i32;
        fn GdipCreateFromHDC(hdc: HDC, graphics: *mut *mut c_void) -> i32;
        fn GdipDeleteGraphics(graphics: *mut c_void) -> i32;
        fn GdipSetSmoothingMode(graphics: *mut c_void, mode: i32) -> i32;
        fn GdipSetPixelOffsetMode(graphics: *mut c_void, mode: i32) -> i32;
        fn GdipSetCompositingMode(graphics: *mut c_void, mode: i32) -> i32;
        fn GdipCreatePen1(color: u32, width: f32, unit: i32, pen: *mut *mut c_void) -> i32;
        fn GdipDeletePen(pen: *mut c_void) -> i32;
        fn GdipSetPenStartCap(pen: *mut c_void, cap: i32) -> i32;
        fn GdipSetPenEndCap(pen: *mut c_void, cap: i32) -> i32;
        fn GdipSetPenLineJoin(pen: *mut c_void, join: i32) -> i32;
        fn GdipDrawLineI(
            graphics: *mut c_void,
            pen: *mut c_void,
            x1: i32,
            y1: i32,
            x2: i32,
            y2: i32,
        ) -> i32;
        fn GdipDrawRectangleI(
            graphics: *mut c_void,
            pen: *mut c_void,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        ) -> i32;
        fn GdipDrawEllipseI(
            graphics: *mut c_void,
            pen: *mut c_void,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        ) -> i32;
        fn GdipDrawPolygonI(
            graphics: *mut c_void,
            pen: *mut c_void,
            points: *const WinPoint,
            count: i32,
        ) -> i32;
        fn GdipCreatePath(fill_mode: i32, path: *mut *mut c_void) -> i32;
        fn GdipAddPathLineI(path: *mut c_void, x1: i32, y1: i32, x2: i32, y2: i32) -> i32;
        fn GdipAddPathBezier(
            path: *mut c_void,
            x1: f32,
            y1: f32,
            x2: f32,
            y2: f32,
            x3: f32,
            y3: f32,
            x4: f32,
            y4: f32,
        ) -> i32;
        fn GdipDrawPath(graphics: *mut c_void, pen: *mut c_void, path: *mut c_void) -> i32;
        fn GdipDeletePath(path: *mut c_void) -> i32;
        fn GdipCreateSolidFill(color: u32, brush: *mut *mut c_void) -> i32;
        fn GdipDeleteBrush(brush: *mut c_void) -> i32;
        fn GdipFillEllipseI(
            graphics: *mut c_void,
            brush: *mut c_void,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        ) -> i32;
        fn GdipFillPolygonI(
            graphics: *mut c_void,
            brush: *mut c_void,
            points: *const WinPoint,
            count: i32,
            fill_mode: i32,
        ) -> i32;
    }

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
        Number,
        CheckMark,
        CrossMark,
    }

    enum DrawingCommand {
        Show {
            monitors: Vec<RECT>,
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
            monitors: Vec<RECT>,
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
        Number {
            center: Point,
            value: u32,
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

    struct OverlayCanvas {
        memory_dc: CreatedHDC,
        bitmap: HBITMAP,
        old_bitmap: HGDIOBJ,
        bits: usize,
        width: i32,
        height: i32,
    }

    struct OverlaySurface {
        display_hwnd: HWND,
        input_hwnd: HWND,
        bounds: RECT,
        canvas: Option<OverlayCanvas>,
    }

    struct OverlayState {
        app: AppHandle,
        surfaces: Vec<OverlaySurface>,
        input_hwnd: Option<HWND>,
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
        next_number: u32,
        cursor: HCURSOR,
        cursor_owned: bool,
        retired_cursors: Vec<HCURSOR>,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
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
            monitors: Vec<(i32, i32, i32, i32)>,
            toolbar_passthrough: Option<(i32, i32, i32, i32)>,
        ) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::Show {
                monitors: monitors
                    .into_iter()
                    .map(|(left, top, width, height)| RECT {
                        left,
                        top,
                        right: left + width,
                        bottom: top + height,
                    })
                    .collect(),
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

        pub fn resize(&self, monitors: Vec<(i32, i32, i32, i32)>) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(DrawingCommand::Resize {
                monitors: monitors
                    .into_iter()
                    .map(|(left, top, width, height)| RECT {
                        left,
                        top,
                        right: left + width,
                        bottom: top + height,
                    })
                    .collect(),
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
            "number" => Some(NativeTool::Number),
            "check-mark" => Some(NativeTool::CheckMark),
            "cross-mark" => Some(NativeTool::CrossMark),
            _ => None,
        }
    }

    fn run_window(receiver: Receiver<DrawingCommand>, app: AppHandle) -> Result<(), String> {
        let class_name = wide("KeyvizNativeDrawingOverlay");

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

            let default_color = parse_color("#ef2b2d");
            let (cursor, cursor_owned) = create_tool_cursor(NativeTool::Pen, 5, 1, default_color);

            if let Ok(mut state) = overlay_state().lock() {
                *state = Some(OverlayState {
                    app,
                    surfaces: Vec::new(),
                    input_hwnd: None,
                    tool: NativeTool::Pen,
                    color: default_color,
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
                    next_number: 1,
                    cursor,
                    cursor_owned,
                    retired_cursors: Vec::new(),
                });
            }

            message_loop(receiver);

            if let Ok(mut state_guard) = overlay_state().lock() {
                if let Some(state) = state_guard.as_mut() {
                    if restore_system_cursor() {
                        destroy_retired_cursors(state);
                        if state.cursor_owned && state.cursor.0 != 0 {
                            let _ = DestroyCursor(state.cursor);
                        }
                    }
                    destroy_overlay_surfaces(&mut state.surfaces);
                }
                *state_guard = None;
            }
        }

        Ok(())
    }

    unsafe fn create_overlay_surface(bounds: RECT) -> Result<OverlaySurface, String> {
        let module = GetModuleHandleW(None).map_err(|error| error.to_string())?;
        let class_name = wide("KeyvizNativeDrawingOverlay");
        let display_name = wide("Keyviz Drawing Display");
        let display_hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(display_name.as_ptr()),
            WS_POPUP,
            bounds.left,
            bounds.top,
            (bounds.right - bounds.left).max(1),
            (bounds.bottom - bounds.top).max(1),
            HWND(0),
            None,
            module,
            None,
        );
        if display_hwnd.0 == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }

        // Input is handled by a separate window that intentionally has no DWM
        // redirection bitmap. This keeps the visible layered window fully
        // transparent instead of filling its background with low-alpha black
        // pixels just to make it hit-testable. Capture and pinning tools can
        // otherwise promote those pixels to opaque black.
        let input_name = wide("Keyviz Drawing Input");
        let input_hwnd = CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(input_name.as_ptr()),
            WS_POPUP,
            bounds.left,
            bounds.top,
            (bounds.right - bounds.left).max(1),
            (bounds.bottom - bounds.top).max(1),
            HWND(0),
            None,
            module,
            None,
        );
        if input_hwnd.0 == 0 {
            DestroyWindow(display_hwnd);
            return Err(std::io::Error::last_os_error().to_string());
        }

        ShowWindow(display_hwnd, SW_HIDE);
        ShowWindow(input_hwnd, SW_HIDE);
        Ok(OverlaySurface {
            display_hwnd,
            input_hwnd,
            bounds,
            canvas: None,
        })
    }

    unsafe fn destroy_overlay_surfaces(surfaces: &mut Vec<OverlaySurface>) {
        for mut surface in surfaces.drain(..) {
            release_overlay_canvas(&mut surface.canvas);
            DestroyWindow(surface.input_hwnd);
            DestroyWindow(surface.display_hwnd);
        }
    }

    unsafe fn rebuild_overlay_surfaces(state: &mut OverlayState, monitors: &[RECT]) -> bool {
        let monitors: Vec<RECT> = monitors
            .iter()
            .copied()
            .filter(|bounds| bounds.right > bounds.left && bounds.bottom > bounds.top)
            .collect();
        if monitors.is_empty() {
            return false;
        }

        let mut new_surfaces = Vec::with_capacity(monitors.len());
        for bounds in &monitors {
            match create_overlay_surface(*bounds) {
                Ok(surface) => new_surfaces.push(surface),
                Err(error) => {
                    eprintln!("Failed to create drawing surface: {error}");
                    destroy_overlay_surfaces(&mut new_surfaces);
                    return false;
                }
            }
        }

        let bounds = RECT {
            left: monitors.iter().map(|bounds| bounds.left).min().unwrap_or(0),
            top: monitors.iter().map(|bounds| bounds.top).min().unwrap_or(0),
            right: monitors
                .iter()
                .map(|bounds| bounds.right)
                .max()
                .unwrap_or(1),
            bottom: monitors
                .iter()
                .map(|bounds| bounds.bottom)
                .max()
                .unwrap_or(1),
        };
        destroy_overlay_surfaces(&mut state.surfaces);
        state.surfaces = new_surfaces;
        state.bounds = bounds;
        state.input_hwnd = first_surface_hwnd(state);
        true
    }

    fn first_surface_hwnd(state: &OverlayState) -> Option<HWND> {
        state.surfaces.first().map(|surface| surface.input_hwnd)
    }

    fn surface_layout_matches(state: &OverlayState, monitors: &[RECT]) -> bool {
        state.surfaces.len() == monitors.len()
            && state
                .surfaces
                .iter()
                .zip(monitors)
                .all(|(surface, monitor)| {
                    surface.bounds.left == monitor.left
                        && surface.bounds.top == monitor.top
                        && surface.bounds.right == monitor.right
                        && surface.bounds.bottom == monitor.bottom
                })
    }

    fn surface_hwnd_for_point(state: &OverlayState, point: Point) -> Option<HWND> {
        let global_x = point.x + state.bounds.left;
        let global_y = point.y + state.bounds.top;
        state
            .surfaces
            .iter()
            .find(|surface| {
                global_x >= surface.bounds.left
                    && global_x < surface.bounds.right
                    && global_y >= surface.bounds.top
                    && global_y < surface.bounds.bottom
            })
            .map(|surface| surface.input_hwnd)
            .or_else(|| first_surface_hwnd(state))
    }

    unsafe fn message_loop(receiver: Receiver<DrawingCommand>) {
        let mut message = MSG::default();
        let mut commands = VecDeque::new();

        loop {
            if commands.is_empty() {
                match receiver.recv_timeout(Duration::from_millis(16)) {
                    Ok(command) => queue_drawing_command(&mut commands, command),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            while let Ok(command) = receiver.try_recv() {
                queue_drawing_command(&mut commands, command);
            }
            while let Some(command) = commands.pop_front() {
                apply_command(command);
            }

            while PeekMessageW(&mut message, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                if message.message == WM_MOUSEMOVE {
                    loop {
                        let mut next = MSG::default();
                        if !PeekMessageW(&mut next, HWND(0), 0, 0, PM_NOREMOVE).as_bool()
                            || next.message != WM_MOUSEMOVE
                        {
                            break;
                        }
                        if PeekMessageW(&mut next, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                            message = next;
                        }
                    }
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    fn queue_drawing_command(queue: &mut VecDeque<DrawingCommand>, command: DrawingCommand) {
        if matches!(&command, DrawingCommand::PointerMove { .. })
            && matches!(queue.back(), Some(DrawingCommand::PointerMove { .. }))
        {
            queue.pop_back();
        }
        queue.push_back(command);
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
                monitors,
                toolbar_passthrough,
            } => {
                state.toolbar_passthrough = toolbar_passthrough;
                if !surface_layout_matches(state, &monitors)
                    && !rebuild_overlay_surfaces(state, &monitors)
                {
                    return;
                }
                state.visible = true;
                for surface in &state.surfaces {
                    let _ = SetWindowPos(
                        surface.display_hwnd,
                        HWND_TOPMOST,
                        surface.bounds.left,
                        surface.bounds.top,
                        (surface.bounds.right - surface.bounds.left).max(1),
                        (surface.bounds.bottom - surface.bounds.top).max(1),
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                }
                sync_input_surface_visibility(state);
                if matches!(state.tool, NativeTool::Number) {
                    replace_tool_cursor(state, true);
                }
                if state.cursor.0 != 0 {
                    SetCursor(state.cursor);
                    destroy_retired_cursors(state);
                }
                emit_history(&state.app, !state.drawings.is_empty());
                refresh_overlay(state);
            }
            DrawingCommand::Resize { monitors } => {
                if state.visible {
                    return;
                }
                let _ = rebuild_overlay_surfaces(state, &monitors);
            }
            DrawingCommand::Hide => {
                cancel_text_editor(state);
                state.drawings.clear();
                state.active = None;
                state.selected.clear();
                state.selection = None;
                state.next_number = 1;
                if restore_system_cursor() {
                    destroy_retired_cursors(state);
                }
                state.visible = false;
                state.input_hwnd = None;
                destroy_overlay_surfaces(&mut state.surfaces);
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
                sync_input_surface_visibility(state);
                replace_tool_cursor(state, false);
                if state.visible {
                    raise_toolbar(&state.app);
                }
                refresh_overlay(state);
                emit_selection_state(state);
            }
            DrawingCommand::SetColor(color) => {
                state.color = parse_color(&color);
                if matches!(
                    state.tool,
                    NativeTool::Number | NativeTool::CheckMark | NativeTool::CrossMark
                ) {
                    replace_tool_cursor(state, false);
                }
            }
            DrawingCommand::SetWidth(width) => {
                let width = width.clamp(1, 15);
                state.width = width;
                if matches!(
                    state.tool,
                    NativeTool::Eraser
                        | NativeTool::Number
                        | NativeTool::CheckMark
                        | NativeTool::CrossMark
                ) {
                    replace_tool_cursor(state, false);
                }
            }
            DrawingCommand::Clear => {
                cancel_text_editor(state);
                state.drawings.clear();
                state.active = None;
                state.selected.clear();
                state.selection = None;
                state.next_number = 1;
                if matches!(state.tool, NativeTool::Number) {
                    replace_tool_cursor(state, false);
                }
                emit_history(&state.app, false);
                emit_selection_state(state);
                refresh_overlay(state);
            }
            DrawingCommand::Undo => {
                if state.edit.is_none() {
                    state.selected.clear();
                    state.selection = None;
                    state.drawings.pop();
                    sync_next_number(state);
                    if matches!(state.tool, NativeTool::Number) {
                        replace_tool_cursor(state, false);
                    }
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
                    sync_next_number(state);
                } else {
                    state.drawings.clear();
                    state.active = None;
                    state.selected.clear();
                    state.selection = None;
                    state.next_number = 1;
                    if matches!(state.tool, NativeTool::Number) {
                        replace_tool_cursor(state, false);
                    }
                }
                emit_history(&state.app, !state.drawings.is_empty());
                emit_selection_state(state);
                refresh_overlay(state);
            }
            DrawingCommand::SetClickThrough(enabled) => {
                state.click_through = enabled;
                sync_input_surface_visibility(state);
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
                    if let Some(hwnd) = state.input_hwnd.or_else(|| first_surface_hwnd(state)) {
                        let _ = SetFocus(hwnd);
                    }
                }
            }
            DrawingCommand::Raise => {
                if !state.visible {
                    return;
                }
                for surface in &state.surfaces {
                    let _ = SetWindowPos(
                        surface.display_hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                    );
                    if !state.click_through {
                        let _ = SetWindowPos(
                            surface.input_hwnd,
                            HWND_TOPMOST,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                        );
                    }
                }
                raise_toolbar(&state.app);
            }
            DrawingCommand::PointerDown { x, y } => {
                if let Some(point) = global_point_for_drawing(state, x, y) {
                    let capture_hwnd = surface_hwnd_for_point(state, point);
                    begin_drawing_at(state, point, capture_hwnd);
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
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut paint);
                EndPaint(hwnd, &paint);
                LRESULT(0)
            }
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

    fn is_surface_hwnd(state: &OverlayState, hwnd: HWND) -> bool {
        state
            .surfaces
            .iter()
            .any(|surface| surface.input_hwnd == hwnd)
    }

    fn virtual_point_from_client(state: &OverlayState, hwnd: HWND, point: Point) -> Option<Point> {
        state
            .surfaces
            .iter()
            .find(|surface| surface.input_hwnd == hwnd)
            .map(|surface| Point {
                x: point.x + surface.bounds.left - state.bounds.left,
                y: point.y + surface.bounds.top - state.bounds.top,
            })
    }

    unsafe fn on_left_button_down(hwnd: HWND, lparam: LPARAM) {
        let Ok(mut state_guard) = overlay_state().lock() else {
            return;
        };
        let Some(state) = state_guard.as_mut() else {
            return;
        };
        let Some(point) = virtual_point_from_client(state, hwnd, lparam_point(lparam)) else {
            return;
        };
        state.input_hwnd = Some(hwnd);
        begin_drawing_at(state, point, Some(hwnd));
    }

    unsafe fn begin_drawing_at(state: &mut OverlayState, point: Point, capture_hwnd: Option<HWND>) {
        if !state.visible
            || matches!(state.tool, NativeTool::Pointer)
            || is_toolbar_passthrough_point(state, point, false)
        {
            return;
        }
        if let Some(hwnd) = capture_hwnd {
            state.input_hwnd = Some(hwnd);
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
            NativeTool::Number => {
                let value = state.next_number;
                state.next_number = state.next_number.saturating_add(1).max(1);
                state.drawings.push(DrawingItem::Number {
                    center: point,
                    value,
                    color: state.color,
                    width: state.width.max(1),
                    rotation: 0.0,
                    group: None,
                });
                replace_tool_cursor(state, false);
                emit_history(&state.app, true);
                raise_toolbar(&state.app);
            }
            tool @ (NativeTool::CheckMark | NativeTool::CrossMark) => {
                let radius = stamp_radius(state.width);
                state.drawings.push(DrawingItem::Shape {
                    tool,
                    start: Point {
                        x: point.x - radius,
                        y: point.y - radius,
                    },
                    end: Point {
                        x: point.x + radius,
                        y: point.y + radius,
                    },
                    color: state.color,
                    width: state.width.max(1),
                    rotation: 0.0,
                    group: None,
                });
                emit_history(&state.app, true);
                raise_toolbar(&state.app);
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
        if !is_surface_hwnd(state, hwnd) || (state.active.is_none() && state.selection.is_none()) {
            return;
        }
        if (wparam.0 & MK_LBUTTON_MASK) == 0 {
            return;
        }

        let Some(point) = virtual_point_from_client(state, hwnd, lparam_point(lparam)) else {
            return;
        };
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
        if !is_surface_hwnd(state, hwnd) {
            return;
        }
        let point = virtual_point_from_client(state, hwnd, lparam_point(_lparam))
            .unwrap_or_else(|| lparam_point(_lparam));
        finish_drawing_at(state, point);
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
        if !is_surface_hwnd(state, hwnd) || !state.visible {
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
        if matches!(
            state.tool,
            NativeTool::Number | NativeTool::CheckMark | NativeTool::CrossMark
        ) {
            let width = (state.width + step).clamp(1, 15);
            state.width = width;
            replace_tool_cursor(state, true);
            emit_width(&state.app, width);
            refresh_overlay(state);
            return;
        }
        if matches!(
            state.tool,
            NativeTool::Pen
                | NativeTool::Line
                | NativeTool::Arrow
                | NativeTool::Rectangle
                | NativeTool::Ellipse
        ) {
            let width = (state.width + step).clamp(1, 15);
            state.width = width;
            match state.active.as_mut() {
                Some(ActiveDrawing::Stroke {
                    width: active_width,
                    ..
                })
                | Some(ActiveDrawing::Shape {
                    width: active_width,
                    ..
                }) => *active_width = width,
                None => {}
            }
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
        if !is_surface_hwnd(state, hwnd) {
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
        if !is_surface_hwnd(state, hwnd) {
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
        if !is_surface_hwnd(state, hwnd) {
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
        if !is_surface_hwnd(state, hwnd) {
            return;
        }
        cancel_text_editor(state);
    }

    unsafe fn ensure_overlay_canvas(surface: &mut OverlaySurface) -> bool {
        let width = (surface.bounds.right - surface.bounds.left).max(1);
        let height = (surface.bounds.bottom - surface.bounds.top).max(1);
        if surface
            .canvas
            .as_ref()
            .is_some_and(|canvas| canvas.width == width && canvas.height == height)
        {
            return true;
        }

        release_overlay_canvas(&mut surface.canvas);

        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            return false;
        }
        let memory_dc = CreateCompatibleDC(screen_dc);
        if memory_dc.0 == 0 {
            ReleaseDC(HWND(0), screen_dc);
            return false;
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
            return false;
        };
        if bits.is_null() {
            DeleteObject(bitmap);
            DeleteDC(memory_dc);
            ReleaseDC(HWND(0), screen_dc);
            return false;
        }

        let old_bitmap = SelectObject(memory_dc, bitmap);
        ReleaseDC(HWND(0), screen_dc);
        surface.canvas = Some(OverlayCanvas {
            memory_dc,
            bitmap,
            old_bitmap,
            bits: bits as usize,
            width,
            height,
        });
        true
    }

    unsafe fn release_overlay_canvas(canvas: &mut Option<OverlayCanvas>) {
        let Some(canvas) = canvas.take() else {
            return;
        };
        SelectObject(canvas.memory_dc, canvas.old_bitmap);
        DeleteObject(canvas.bitmap);
        DeleteDC(canvas.memory_dc);
    }

    unsafe fn refresh_overlay(state: &mut OverlayState) {
        if !state.visible || state.surfaces.is_empty() {
            return;
        }

        let mut surfaces = std::mem::take(&mut state.surfaces);
        for surface in &mut surfaces {
            refresh_overlay_surface(state, surface);
        }
        state.surfaces = surfaces;
    }

    unsafe fn refresh_overlay_surface(state: &OverlayState, surface: &mut OverlaySurface) {
        if !ensure_overlay_canvas(surface) {
            return;
        }

        let Some(canvas) = surface.canvas.as_ref() else {
            return;
        };
        let width = canvas.width;
        let height = canvas.height;
        let pixel_count = (width as usize).saturating_mul(height as usize);
        if pixel_count == 0 {
            return;
        }
        let drawing_dc = HDC(canvas.memory_dc.0);
        let pixels = std::slice::from_raw_parts_mut(canvas.bits as *mut u32, pixel_count);
        pixels.fill(0);

        let mut old_origin = WinPoint::default();
        let viewport_x = state.bounds.left - surface.bounds.left;
        let viewport_y = state.bounds.top - surface.bounds.top;
        SetViewportOrgEx(drawing_dc, viewport_x, viewport_y, Some(&mut old_origin));
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
        SetViewportOrgEx(drawing_dc, old_origin.x, old_origin.y, None);

        for (index, pixel) in pixels.iter_mut().enumerate() {
            let x = (index % width as usize) as i32 + surface.bounds.left;
            let y = (index / width as usize) as i32 + surface.bounds.top;
            if is_toolbar_passthrough_point(state, Point { x, y }, true) {
                *pixel = 0;
                continue;
            }
            let rgb = *pixel & 0x00ff_ffff;
            let alpha = *pixel >> 24;
            if rgb == TRANSPARENT_PIXEL || (rgb == 0 && alpha == 0) {
                *pixel = 0;
            } else if alpha == 0 {
                *pixel |= 0xff00_0000;
            }
        }

        let destination = WinPoint {
            x: surface.bounds.left,
            y: surface.bounds.top,
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
        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            return;
        }
        let _ = UpdateLayeredWindow(
            surface.display_hwnd,
            screen_dc,
            Some(&destination),
            Some(&size),
            drawing_dc,
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        ReleaseDC(HWND(0), screen_dc);
    }

    fn drawing_group(drawing: &DrawingItem) -> Option<u64> {
        match drawing {
            DrawingItem::Stroke { group, .. }
            | DrawingItem::Shape { group, .. }
            | DrawingItem::Text { group, .. }
            | DrawingItem::Number { group, .. } => *group,
        }
    }

    fn set_drawing_group(drawing: &mut DrawingItem, value: Option<u64>) {
        match drawing {
            DrawingItem::Stroke { group, .. }
            | DrawingItem::Shape { group, .. }
            | DrawingItem::Text { group, .. }
            | DrawingItem::Number { group, .. } => *group = value,
        }
    }

    fn drawing_width(drawing: &DrawingItem) -> i32 {
        match drawing {
            DrawingItem::Stroke { width, .. }
            | DrawingItem::Shape { width, .. }
            | DrawingItem::Text { width, .. }
            | DrawingItem::Number { width, .. } => *width,
        }
    }

    fn set_drawing_width(drawing: &mut DrawingItem, value: i32) {
        let value = value.clamp(1, 100);
        match drawing {
            DrawingItem::Stroke { width, .. }
            | DrawingItem::Shape { width, .. }
            | DrawingItem::Text { width, .. }
            | DrawingItem::Number { width, .. } => *width = value,
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
            DrawingItem::Number { center, .. } => {
                center.x += dx;
                center.y += dy;
            }
        }
    }

    fn sync_next_number(state: &mut OverlayState) {
        state.next_number = state
            .drawings
            .iter()
            .filter_map(|drawing| match drawing {
                DrawingItem::Number { value, .. } => Some(*value),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
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
            DrawingItem::Stroke { rotation, .. }
            | DrawingItem::Text { rotation, .. }
            | DrawingItem::Number { rotation, .. } => *rotation,
            DrawingItem::Shape {
                tool,
                start,
                end,
                rotation,
                ..
            } => {
                if matches!(
                    tool,
                    NativeTool::Rectangle
                        | NativeTool::Ellipse
                        | NativeTool::CheckMark
                        | NativeTool::CrossMark
                ) {
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
            } if matches!(
                tool,
                NativeTool::Rectangle
                    | NativeTool::Ellipse
                    | NativeTool::CheckMark
                    | NativeTool::CrossMark
            ) =>
            {
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
            DrawingItem::Number {
                center,
                width,
                rotation,
                ..
            } => number_corners(*center, *width, *rotation).to_vec(),
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
            DrawingItem::Text { .. } | DrawingItem::Number { .. } => 5.0,
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
                if matches!(
                    tool,
                    NativeTool::Rectangle
                        | NativeTool::Ellipse
                        | NativeTool::CheckMark
                        | NativeTool::CrossMark
                ) {
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
            DrawingItem::Number {
                center,
                width,
                rotation,
                ..
            } => {
                *center = transform_point_oriented(*center, anchor, scale_x, scale_y, angle);
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
                if matches!(
                    tool,
                    NativeTool::Rectangle
                        | NativeTool::Ellipse
                        | NativeTool::CheckMark
                        | NativeTool::CrossMark
                ) {
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
            DrawingItem::Number {
                center: marker_center,
                rotation,
                ..
            } => {
                *marker_center = rotate_point(*marker_center, center, angle);
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
                let points = if matches!(
                    tool,
                    NativeTool::Rectangle
                        | NativeTool::Ellipse
                        | NativeTool::CheckMark
                        | NativeTool::CrossMark
                ) {
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
            DrawingItem::Number {
                center,
                width,
                rotation,
                ..
            } => {
                let corners = number_corners(*center, *width, *rotation);
                RECT {
                    left: corners
                        .iter()
                        .map(|point| point.x)
                        .min()
                        .unwrap_or(center.x)
                        - 5,
                    top: corners
                        .iter()
                        .map(|point| point.y)
                        .min()
                        .unwrap_or(center.y)
                        - 5,
                    right: corners
                        .iter()
                        .map(|point| point.x)
                        .max()
                        .unwrap_or(center.x)
                        + 5,
                    bottom: corners
                        .iter()
                        .map(|point| point.y)
                        .max()
                        .unwrap_or(center.y)
                        + 5,
                }
            }
        }
    }

    fn number_radius(width: i32) -> i32 {
        14 + width.clamp(1, 100) * 2
    }

    fn stamp_radius(width: i32) -> i32 {
        number_radius(width)
    }

    fn number_corners(center: Point, width: i32, rotation: f64) -> [Point; 4] {
        let radius = number_radius(width);
        [
            Point {
                x: center.x - radius,
                y: center.y - radius,
            },
            Point {
                x: center.x + radius,
                y: center.y - radius,
            },
            Point {
                x: center.x + radius,
                y: center.y + radius,
            },
            Point {
                x: center.x - radius,
                y: center.y + radius,
            },
        ]
        .map(|point| rotate_point(point, center, rotation))
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
                erase,
                ..
            } => draw_polyline(dc, points, *color, *width, *erase),
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
            DrawingItem::Number {
                center,
                value,
                color,
                width,
                rotation,
                ..
            } => draw_number_marker(dc, *center, *value, *color, *width, *rotation),
        }
    }

    unsafe fn draw_active(dc: windows::Win32::Graphics::Gdi::HDC, drawing: &ActiveDrawing) {
        match drawing {
            ActiveDrawing::Stroke {
                points,
                color,
                width,
                erase,
            } => draw_polyline(dc, points, *color, *width, *erase),
            ActiveDrawing::Shape {
                tool,
                start,
                end,
                color,
                width,
            } => draw_shape(dc, tool.clone(), *start, *end, *color, *width, 0.0),
        }
    }

    fn ensure_gdiplus() -> bool {
        static TOKEN: OnceLock<Option<usize>> = OnceLock::new();
        TOKEN
            .get_or_init(|| unsafe {
                let input = GdiplusStartupInput {
                    version: 1,
                    debug_event_callback: std::ptr::null_mut(),
                    suppress_background_thread: 0,
                    suppress_external_codecs: 0,
                };
                let mut token = 0usize;
                if GdiplusStartup(&mut token, &input, std::ptr::null_mut()) == 0 {
                    Some(token)
                } else {
                    None
                }
            })
            .is_some()
    }

    unsafe fn with_gdiplus_graphics<T>(
        dc: HDC,
        source_copy: bool,
        draw: impl FnOnce(*mut c_void) -> T,
    ) -> Option<T> {
        if !ensure_gdiplus() {
            return None;
        }
        let mut graphics = std::ptr::null_mut();
        if GdipCreateFromHDC(dc, &mut graphics) != 0 || graphics.is_null() {
            return None;
        }
        let _ = GdipSetSmoothingMode(graphics, 4);
        let _ = GdipSetPixelOffsetMode(graphics, 4);
        let _ = GdipSetCompositingMode(graphics, if source_copy { 1 } else { 0 });
        let result = draw(graphics);
        GdipDeleteGraphics(graphics);
        Some(result)
    }

    unsafe fn create_gdiplus_pen(color: u32, width: i32) -> Option<*mut c_void> {
        let mut pen = std::ptr::null_mut();
        if GdipCreatePen1(color, width.max(1) as f32, 2, &mut pen) != 0 || pen.is_null() {
            return None;
        }
        let _ = GdipSetPenStartCap(pen, 2);
        let _ = GdipSetPenEndCap(pen, 2);
        let _ = GdipSetPenLineJoin(pen, 2);
        Some(pen)
    }

    unsafe fn draw_smooth_path(
        dc: HDC,
        points: &[Point],
        color: COLORREF,
        width: i32,
        erase: bool,
    ) -> bool {
        with_gdiplus_graphics(dc, erase, |graphics| {
            let Some(pen) =
                create_gdiplus_pen(if erase { 0 } else { argb_from_color(color) }, width)
            else {
                return false;
            };
            let mut path = std::ptr::null_mut();
            if GdipCreatePath(0, &mut path) != 0 || path.is_null() {
                GdipDeletePen(pen);
                return false;
            }

            let status = if points.len() == 2 {
                GdipAddPathLineI(path, points[0].x, points[0].y, points[1].x, points[1].y)
            } else {
                let mut status = 0;
                for index in 0..points.len() - 1 {
                    let p0 = points[index.saturating_sub(1)];
                    let p1 = points[index];
                    let p2 = points[index + 1];
                    let p3 = points[(index + 2).min(points.len() - 1)];
                    let c1x = p1.x as f32 + (p2.x - p0.x) as f32 / 6.0;
                    let c1y = p1.y as f32 + (p2.y - p0.y) as f32 / 6.0;
                    let c2x = p2.x as f32 - (p3.x - p1.x) as f32 / 6.0;
                    let c2y = p2.y as f32 - (p3.y - p1.y) as f32 / 6.0;
                    status = GdipAddPathBezier(
                        path,
                        p1.x as f32,
                        p1.y as f32,
                        c1x,
                        c1y,
                        c2x,
                        c2y,
                        p2.x as f32,
                        p2.y as f32,
                    );
                    if status != 0 {
                        break;
                    }
                }
                status
            };
            let drawn = status == 0 && GdipDrawPath(graphics, pen, path) == 0;
            GdipDeletePath(path);
            GdipDeletePen(pen);
            drawn
        })
        .unwrap_or(false)
    }

    unsafe fn draw_antialiased_shape(
        dc: HDC,
        tool: &NativeTool,
        start: Point,
        end: Point,
        color: COLORREF,
        width: i32,
        rotation: f64,
    ) -> bool {
        with_gdiplus_graphics(dc, false, |graphics| {
            let Some(pen) = create_gdiplus_pen(argb_from_color(color), width) else {
                return false;
            };
            let left = start.x.min(end.x);
            let top = start.y.min(end.y);
            let shape_width = (end.x - start.x).abs().max(1);
            let shape_height = (end.y - start.y).abs().max(1);
            let status = match tool {
                NativeTool::Line => GdipDrawLineI(graphics, pen, start.x, start.y, end.x, end.y),
                NativeTool::Rectangle if rotation.abs() < f64::EPSILON => {
                    GdipDrawRectangleI(graphics, pen, left, top, shape_width, shape_height)
                }
                NativeTool::Ellipse if rotation.abs() < f64::EPSILON => {
                    GdipDrawEllipseI(graphics, pen, left, top, shape_width, shape_height)
                }
                NativeTool::Rectangle => {
                    let points =
                        rotated_shape_corners(start, end, rotation).map(|point| WinPoint {
                            x: point.x,
                            y: point.y,
                        });
                    GdipDrawPolygonI(graphics, pen, points.as_ptr(), points.len() as i32)
                }
                NativeTool::Ellipse => {
                    let center_x = (start.x + end.x) as f64 / 2.0;
                    let center_y = (start.y + end.y) as f64 / 2.0;
                    let radius_x = shape_width as f64 / 2.0;
                    let radius_y = shape_height as f64 / 2.0;
                    let cos = rotation.cos();
                    let sin = rotation.sin();
                    let points: Vec<WinPoint> = (0..96)
                        .map(|index| {
                            let angle = std::f64::consts::TAU * index as f64 / 96.0;
                            let x = radius_x * angle.cos();
                            let y = radius_y * angle.sin();
                            WinPoint {
                                x: (center_x + x * cos - y * sin).round() as i32,
                                y: (center_y + x * sin + y * cos).round() as i32,
                            }
                        })
                        .collect();
                    GdipDrawPolygonI(graphics, pen, points.as_ptr(), points.len() as i32)
                }
                NativeTool::CheckMark => {
                    let center = Point {
                        x: (start.x + end.x) / 2,
                        y: (start.y + end.y) / 2,
                    };
                    let points = [
                        Point {
                            x: left,
                            y: top + shape_height * 52 / 100,
                        },
                        Point {
                            x: left + shape_width * 38 / 100,
                            y: top + shape_height,
                        },
                        Point {
                            x: left + shape_width,
                            y: top,
                        },
                    ]
                    .map(|point| rotate_point(point, center, rotation));
                    let first = GdipDrawLineI(
                        graphics,
                        pen,
                        points[0].x,
                        points[0].y,
                        points[1].x,
                        points[1].y,
                    );
                    let second = GdipDrawLineI(
                        graphics,
                        pen,
                        points[1].x,
                        points[1].y,
                        points[2].x,
                        points[2].y,
                    );
                    if first == 0 {
                        second
                    } else {
                        first
                    }
                }
                NativeTool::CrossMark => {
                    let center = Point {
                        x: (start.x + end.x) / 2,
                        y: (start.y + end.y) / 2,
                    };
                    let points = [
                        Point { x: left, y: top },
                        Point {
                            x: left + shape_width,
                            y: top + shape_height,
                        },
                        Point {
                            x: left + shape_width,
                            y: top,
                        },
                        Point {
                            x: left,
                            y: top + shape_height,
                        },
                    ]
                    .map(|point| rotate_point(point, center, rotation));
                    let first = GdipDrawLineI(
                        graphics,
                        pen,
                        points[0].x,
                        points[0].y,
                        points[1].x,
                        points[1].y,
                    );
                    let second = GdipDrawLineI(
                        graphics,
                        pen,
                        points[2].x,
                        points[2].y,
                        points[3].x,
                        points[3].y,
                    );
                    if first == 0 {
                        second
                    } else {
                        first
                    }
                }
                _ => 1,
            };
            GdipDeletePen(pen);
            status == 0
        })
        .unwrap_or(false)
    }

    fn argb_from_color(color: COLORREF) -> u32 {
        let red = color.0 & 0xff;
        let green = (color.0 >> 8) & 0xff;
        let blue = (color.0 >> 16) & 0xff;
        0xff00_0000 | (red << 16) | (green << 8) | blue
    }

    unsafe fn draw_polyline(
        dc: windows::Win32::Graphics::Gdi::HDC,
        points: &[Point],
        color: COLORREF,
        width: i32,
        erase: bool,
    ) {
        if points.len() < 2 {
            return;
        }
        if draw_smooth_path(dc, points, color, width, erase) {
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
        if matches!(tool, NativeTool::Arrow) {
            draw_tapered_arrow(dc, start, end, color, width.max(1));
            return;
        }
        if draw_antialiased_shape(dc, &tool, start, end, color, width, rotation) {
            return;
        }
        let pen = CreatePen(PS_SOLID, width.max(1), color);
        let old_pen = SelectObject(dc, pen);
        let old_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
        match tool {
            NativeTool::Line => {
                MoveToEx(dc, start.x, start.y, None);
                LineTo(dc, end.x, end.y);
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
            NativeTool::CheckMark => {
                let center = Point {
                    x: (start.x + end.x) / 2,
                    y: (start.y + end.y) / 2,
                };
                let left = start.x.min(end.x);
                let top = start.y.min(end.y);
                let shape_width = (end.x - start.x).abs().max(1);
                let shape_height = (end.y - start.y).abs().max(1);
                let points = [
                    Point {
                        x: left,
                        y: top + shape_height * 52 / 100,
                    },
                    Point {
                        x: left + shape_width * 38 / 100,
                        y: top + shape_height,
                    },
                    Point {
                        x: left + shape_width,
                        y: top,
                    },
                ]
                .map(|point| rotate_point(point, center, rotation));
                MoveToEx(dc, points[0].x, points[0].y, None);
                LineTo(dc, points[1].x, points[1].y);
                LineTo(dc, points[2].x, points[2].y);
            }
            NativeTool::CrossMark => {
                let center = Point {
                    x: (start.x + end.x) / 2,
                    y: (start.y + end.y) / 2,
                };
                let corners = rotated_shape_corners(start, end, rotation);
                MoveToEx(dc, corners[0].x, corners[0].y, None);
                LineTo(dc, corners[2].x, corners[2].y);
                MoveToEx(dc, corners[1].x, corners[1].y, None);
                LineTo(dc, corners[3].x, corners[3].y);
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
        if with_gdiplus_graphics(dc, false, |graphics| {
            let mut brush = std::ptr::null_mut();
            if GdipCreateSolidFill(argb_from_color(color), &mut brush) != 0 || brush.is_null() {
                return false;
            }
            let status = GdipFillPolygonI(graphics, brush, points.as_ptr(), points.len() as i32, 0);
            GdipDeleteBrush(brush);
            status == 0
        })
        .unwrap_or(false)
        {
            return;
        }
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
        // GDI font escapement uses the opposite rotation direction from our
        // screen-coordinate geometry (where positive Y points downward).
        let escapement = (-rotation.to_degrees() * 10.0).round() as i32;
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
        let old_color = windows::Win32::Graphics::Gdi::SetTextColor(dc, gdi_visible_color(color));
        let wide_text: Vec<u16> = text.encode_utf16().collect();
        let _ = TextOutW(dc, start.x, start.y, &wide_text);
        windows::Win32::Graphics::Gdi::SetTextColor(dc, old_color);
        SelectObject(dc, old_font);
        DeleteObject(font);
    }

    fn text_font_size(width: i32) -> i32 {
        18.max(width * 4)
    }

    fn gdi_visible_color(color: COLORREF) -> COLORREF {
        if color.0 & 0x00ff_ffff == 0 {
            COLORREF(0x0001_0101)
        } else {
            color
        }
    }

    unsafe fn draw_number_marker(
        dc: HDC,
        center: Point,
        value: u32,
        color: COLORREF,
        width: i32,
        rotation: f64,
    ) {
        let radius = number_radius(width);
        let smooth_circle = with_gdiplus_graphics(dc, false, |graphics| {
            let mut brush = std::ptr::null_mut();
            if GdipCreateSolidFill(argb_from_color(color), &mut brush) != 0 || brush.is_null() {
                return false;
            }
            let status = GdipFillEllipseI(
                graphics,
                brush,
                center.x - radius,
                center.y - radius,
                radius * 2,
                radius * 2,
            );
            GdipDeleteBrush(brush);
            status == 0
        })
        .unwrap_or(false);
        if !smooth_circle {
            let brush = CreateSolidBrush(color);
            let old_brush = SelectObject(dc, brush);
            let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
            Ellipse(
                dc,
                center.x - radius,
                center.y - radius,
                center.x + radius,
                center.y + radius,
            );
            SelectObject(dc, old_pen);
            SelectObject(dc, old_brush);
            DeleteObject(brush);
        }

        let text = value.to_string();
        let digit_count = text.chars().count() as i32;
        let font_size = if digit_count >= 3 {
            radius.max(16)
        } else {
            (radius * 6 / 5).max(16)
        };
        let escapement = (-rotation.to_degrees() * 10.0).round() as i32;
        let font = CreateFontW(
            -font_size,
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
        let old_color = windows::Win32::Graphics::Gdi::SetTextColor(dc, COLORREF(0x00ff_ffff));
        let wide_text: Vec<u16> = text.encode_utf16().collect();
        let mut text_size = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, &wide_text, &mut text_size);
        let origin = rotate_point(
            Point {
                x: center.x - text_size.cx / 2,
                y: center.y - text_size.cy / 2,
            },
            center,
            rotation,
        );
        let _ = TextOutW(dc, origin.x, origin.y, &wide_text);
        windows::Win32::Graphics::Gdi::SetTextColor(dc, old_color);
        SelectObject(dc, old_font);
        DeleteObject(font);
    }

    unsafe fn create_text_editor(state: &mut OverlayState, point: Point) {
        cancel_text_editor(state);
        state.edit = Some(EditSession {
            start: point,
            text: String::new(),
            color: state.color,
            width: state.width.max(1),
        });
        if let Some(hwnd) = state
            .input_hwnd
            .or_else(|| surface_hwnd_for_point(state, point))
        {
            state.input_hwnd = Some(hwnd);
            let _ = SetFocus(hwnd);
        }
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
                    .filter(|state| is_surface_hwnd(state, hwnd))
                    .map(|state| {
                        state.click_through || is_toolbar_passthrough_point(state, point, true)
                    })
            })
            .unwrap_or(true)
    }

    unsafe fn sync_input_surface_visibility(state: &mut OverlayState) {
        let capture_input = state.visible && !state.click_through;
        if !capture_input {
            ReleaseCapture();
            state.input_hwnd = None;
        }

        for surface in &state.surfaces {
            if capture_input {
                let _ = SetWindowPos(
                    surface.input_hwnd,
                    HWND_TOPMOST,
                    surface.bounds.left,
                    surface.bounds.top,
                    (surface.bounds.right - surface.bounds.left).max(1),
                    (surface.bounds.bottom - surface.bounds.top).max(1),
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            } else {
                ShowWindow(surface.input_hwnd, SW_HIDE);
            }
        }
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

    unsafe fn create_tool_cursor(
        tool: NativeTool,
        width: i32,
        next_number: u32,
        color: COLORREF,
    ) -> (HCURSOR, bool) {
        let custom = match tool {
            NativeTool::Pen => create_pen_cursor(),
            NativeTool::Eraser => create_eraser_cursor(width),
            NativeTool::Number => create_number_cursor(next_number, color, width),
            NativeTool::CheckMark | NativeTool::CrossMark => {
                create_stamp_cursor(tool, color, width)
            }
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
        let (cursor, owned) =
            create_tool_cursor(state.tool, state.width, state.next_number, state.color);
        if cursor.0 == 0 {
            return;
        }
        let previous = state.cursor;
        let previous_owned = state.cursor_owned;
        state.cursor = cursor;
        state.cursor_owned = owned;
        let cursor_replaced = apply_now || state.visible;
        if cursor_replaced {
            SetCursor(cursor);
        }
        if previous_owned && previous.0 != 0 && previous != cursor {
            state.retired_cursors.push(previous);
        }
        if cursor_replaced {
            destroy_retired_cursors(state);
        }
    }

    unsafe fn restore_system_cursor() -> bool {
        if let Ok(cursor) = LoadCursorW(None, IDC_ARROW) {
            SetCursor(cursor);
            true
        } else {
            false
        }
    }

    unsafe fn destroy_retired_cursors(state: &mut OverlayState) {
        for cursor in state.retired_cursors.drain(..) {
            if cursor.0 != 0 && cursor != state.cursor {
                let _ = DestroyCursor(cursor);
            }
        }
    }

    unsafe fn set_overlay_cursor(hwnd: HWND) -> bool {
        let Ok(state_guard) = overlay_state().lock() else {
            return false;
        };
        let Some(state) = state_guard.as_ref() else {
            return false;
        };
        if !is_surface_hwnd(state, hwnd) || !state.visible || state.cursor.0 == 0 {
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

    unsafe fn create_number_cursor(value: u32, color: COLORREF, width: i32) -> Option<HCURSOR> {
        let radius = number_radius(width).clamp(16, 44);
        let cursor_size = radius * 2 + 8;
        let center = cursor_size / 2;

        create_argb_cursor(cursor_size, center as u32, center as u32, |dc| {
            // Pure black has zero RGB bits and would be treated as transparent
            // by the ARGB cursor conversion below, so use a visually black value.
            let fill_color = if color.0 & 0x00ff_ffff == 0 {
                COLORREF(0x0001_0101)
            } else {
                color
            };
            let brush = CreateSolidBrush(fill_color);
            let old_brush = SelectObject(dc, brush);
            let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
            Ellipse(
                dc,
                center - radius,
                center - radius,
                center + radius,
                center + radius,
            );
            SelectObject(dc, old_pen);
            SelectObject(dc, old_brush);
            DeleteObject(brush);

            let text = value.to_string();
            let digit_count = text.chars().count();
            let font_size = if digit_count >= 3 {
                radius.max(16)
            } else {
                (radius * 6 / 5).max(16)
            };
            let font = CreateFontW(
                -font_size,
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
            let old_color = windows::Win32::Graphics::Gdi::SetTextColor(dc, COLORREF(0x00ff_ffff));
            let wide_text: Vec<u16> = text.encode_utf16().collect();
            let mut text_size = SIZE::default();
            let _ = GetTextExtentPoint32W(dc, &wide_text, &mut text_size);
            let _ = TextOutW(
                dc,
                center - text_size.cx / 2,
                center - text_size.cy / 2,
                &wide_text,
            );
            windows::Win32::Graphics::Gdi::SetTextColor(dc, old_color);
            SelectObject(dc, old_font);
            DeleteObject(font);
        })
    }

    unsafe fn create_stamp_cursor(
        tool: NativeTool,
        color: COLORREF,
        width: i32,
    ) -> Option<HCURSOR> {
        let radius = stamp_radius(width).clamp(16, 44);
        let cursor_size = radius * 2 + 8;
        let center = cursor_size / 2;
        let line_color = if color.0 & 0x00ff_ffff == 0 {
            COLORREF(0x0001_0101)
        } else {
            color
        };

        create_argb_cursor(cursor_size, center as u32, center as u32, |dc| {
            let pen = CreatePen(PS_SOLID, width.clamp(2, 15), line_color);
            let old_pen = SelectObject(dc, pen);
            let left = center - radius;
            let top = center - radius;
            let right = center + radius;
            let bottom = center + radius;
            match tool {
                NativeTool::CheckMark => {
                    MoveToEx(dc, left, top + radius, None);
                    LineTo(dc, left + radius * 3 / 4, bottom);
                    LineTo(dc, right, top);
                }
                NativeTool::CrossMark => {
                    MoveToEx(dc, left, top, None);
                    LineTo(dc, right, bottom);
                    MoveToEx(dc, right, top, None);
                    LineTo(dc, left, bottom);
                }
                _ => {}
            }
            SelectObject(dc, old_pen);
            DeleteObject(pen);
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
        Number,
        CheckMark,
        CrossMark,
    }

    impl NativeDrawingOverlay {
        pub fn new(_app: &AppHandle) -> Self {
            Self
        }
        pub fn show(
            &self,
            _monitors: Vec<(i32, i32, i32, i32)>,
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
        pub fn resize(&self, _monitors: Vec<(i32, i32, i32, i32)>) {}
    }

    pub fn parse_tool(_value: &str) -> Option<NativeTool> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub use platform_stub::*;
