#[cfg(target_os = "windows")]
mod platform {
    use std::{
        sync::{
            mpsc::{self, Receiver, Sender},
            Mutex, OnceLock,
        },
        thread,
        time::Duration,
    };

    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, WPARAM},
            Graphics::Gdi::{
                BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, Ellipse, EndPaint, FillRect,
                GetStockObject, InvalidateRect, MonitorFromPoint, SelectObject, UpdateWindow,
                HMONITOR, HOLLOW_BRUSH, MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, PS_SOLID,
            },
            System::LibraryLoader::GetModuleHandleW,
            UI::{
                HiDpi::GetDpiForWindow,
                WindowsAndMessaging::{
                    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
                    GetClientRect, PeekMessageW, RegisterClassW, SetLayeredWindowAttributes,
                    SetWindowPos, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, HWND_TOP,
                    HWND_TOPMOST, LWA_ALPHA, LWA_COLORKEY, MSG, PM_REMOVE, SWP_NOACTIVATE,
                    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, WM_DESTROY, WM_ERASEBKGND,
                    WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
                    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
                },
            },
        },
    };

    const TRANSPARENT_KEY: COLORREF = COLORREF(1 | (2 << 8) | (3 << 16));

    #[derive(Clone)]
    pub struct NativeCursorOverlay {
        sender: Option<Sender<CursorCommand>>,
    }

    enum CursorCommand {
        Update(CursorVisual),
    }

    #[derive(Clone)]
    struct CursorVisual {
        x: f64,
        y: f64,
        size: f64,
        color: COLORREF,
        opacity: f64,
        thickness: f64,
        visible: bool,
    }

    struct PaintState {
        color: COLORREF,
        thickness: i32,
    }

    static PAINT_STATE: OnceLock<Mutex<PaintState>> = OnceLock::new();

    impl Default for NativeCursorOverlay {
        fn default() -> Self {
            Self { sender: None }
        }
    }

    impl NativeCursorOverlay {
        pub fn new() -> Self {
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                if let Err(error) = run_window(receiver) {
                    eprintln!("Native cursor overlay failed: {error}");
                }
            });
            Self {
                sender: Some(sender),
            }
        }

        pub fn update(
            &self,
            x: f64,
            y: f64,
            size: f64,
            color: &str,
            opacity: f64,
            thickness: f64,
            visible: bool,
        ) {
            let Some(sender) = &self.sender else {
                return;
            };
            let _ = sender.send(CursorCommand::Update(CursorVisual {
                x,
                y,
                size,
                color: parse_color(color),
                opacity,
                thickness,
                visible,
            }));
        }
    }

    fn parse_color(value: &str) -> COLORREF {
        let value = value.trim_start_matches('#');
        let parsed = u32::from_str_radix(value, 16).unwrap_or(0x009dff);
        let red = (parsed >> 16) & 0xff;
        let green = (parsed >> 8) & 0xff;
        let blue = parsed & 0xff;
        COLORREF(red | (green << 8) | (blue << 16))
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn run_window(receiver: Receiver<CursorCommand>) -> Result<(), String> {
        let class_name = wide("KeyvizNativeCursorOverlay");

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

            PAINT_STATE.get_or_init(|| {
                Mutex::new(PaintState {
                    color: COLORREF(0x00ff0000),
                    thickness: 6,
                })
            });

            let mut windows = Vec::new();
            message_loop(receiver, &mut windows);
            for (_, hwnd) in windows {
                DestroyWindow(hwnd);
            }
        }

        Ok(())
    }

    unsafe fn create_cursor_window() -> Result<HWND, String> {
        let module = GetModuleHandleW(None).map_err(|error| error.to_string())?;
        let class_name = wide("KeyvizNativeCursorOverlay");
        let window_name = wide("Keyviz Cursor");
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
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
        if !SetLayeredWindowAttributes(hwnd, TRANSPARENT_KEY, 255, LWA_COLORKEY | LWA_ALPHA)
            .as_bool()
        {
            DestroyWindow(hwnd);
            return Err(std::io::Error::last_os_error().to_string());
        }
        ShowWindow(hwnd, SW_HIDE);
        Ok(hwnd)
    }

    unsafe fn message_loop(receiver: Receiver<CursorCommand>, windows: &mut Vec<(HMONITOR, HWND)>) {
        let mut message = MSG::default();

        loop {
            let mut visual = match receiver.recv_timeout(Duration::from_millis(16)) {
                Ok(CursorCommand::Update(visual)) => Some(visual),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => None,
            };
            while let Ok(CursorCommand::Update(next)) = receiver.try_recv() {
                visual = Some(next);
            }
            if let Some(visual) = visual {
                apply_visual(windows, visual);
            }

            while PeekMessageW(&mut message, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    unsafe fn apply_visual(windows: &mut Vec<(HMONITOR, HWND)>, visual: CursorVisual) {
        if !visual.visible {
            for (_, hwnd) in windows.iter() {
                ShowWindow(*hwnd, SW_HIDE);
            }
            return;
        }

        let monitor = MonitorFromPoint(
            POINT {
                x: visual.x.round() as i32,
                y: visual.y.round() as i32,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let hwnd =
            if let Some((_, hwnd)) = windows.iter().find(|(candidate, _)| *candidate == monitor) {
                *hwnd
            } else {
                let Ok(hwnd) = create_cursor_window() else {
                    return;
                };
                windows.push((monitor, hwnd));
                hwnd
            };
        for (_, candidate) in windows.iter() {
            if *candidate != hwnd {
                ShowWindow(*candidate, SW_HIDE);
            }
        }

        if let Some(state) = PAINT_STATE.get() {
            if let Ok(mut state) = state.lock() {
                state.color = visual.color;
                let dpi = GetDpiForWindow(hwnd).max(96) as f64;
                state.thickness = (visual.thickness.clamp(1.0, 30.0) * dpi / 96.0).round() as i32;
            }
        }

        let dpi = GetDpiForWindow(hwnd).max(96) as f64;
        let padding = visual.thickness.clamp(1.0, 30.0) * 2.0 + 8.0;
        let physical_size = ((visual.size.max(32.0) + padding) * dpi / 96.0).round() as i32;
        let left = (visual.x - physical_size as f64 / 2.0).round() as i32;
        let top = (visual.y - physical_size as f64 / 2.0).round() as i32;
        let alpha = (visual.opacity.clamp(10.0, 100.0) * 2.55).round() as u8;

        SetLayeredWindowAttributes(hwnd, TRANSPARENT_KEY, alpha, LWA_COLORKEY | LWA_ALPHA);

        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            left,
            top,
            physical_size,
            physical_size,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        // HWND_TOPMOST keeps the overlay in the topmost band. HWND_TOP then
        // moves it above other topmost application windows without activation.
        SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        InvalidateRect(hwnd, None, true);
        UpdateWindow(hwnd);
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
                paint_ring(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe fn paint_ring(hwnd: HWND) {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(hwnd, &mut paint);
        let mut rect = Default::default();
        GetClientRect(hwnd, &mut rect);

        let background = CreateSolidBrush(TRANSPARENT_KEY);
        FillRect(dc, &rect, background);
        DeleteObject(background);

        let (color, stroke) = PAINT_STATE
            .get()
            .and_then(|state| state.lock().ok())
            .map(|state| (state.color, state.thickness.max(1)))
            .unwrap_or((COLORREF(0x00ff0000), 10));
        let inset = stroke / 2 + 3;

        let pen = CreatePen(PS_SOLID, stroke, color);
        let previous_pen = SelectObject(dc, pen);
        let previous_brush = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
        Ellipse(
            dc,
            rect.left + inset,
            rect.top + inset,
            rect.right - inset,
            rect.bottom - inset,
        );
        SelectObject(dc, previous_pen);
        SelectObject(dc, previous_brush);
        DeleteObject(pen);
        EndPaint(hwnd, &paint);
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    #[derive(Clone, Default)]
    pub struct NativeCursorOverlay;

    impl NativeCursorOverlay {
        pub fn new() -> Self {
            Self
        }

        pub fn update(
            &self,
            _x: f64,
            _y: f64,
            _size: f64,
            _color: &str,
            _opacity: f64,
            _thickness: f64,
            _visible: bool,
        ) {
        }
    }
}

pub use platform::NativeCursorOverlay;
