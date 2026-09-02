use std::{
    ffi::c_void,
    mem::size_of,
    sync::{
        atomic::{AtomicIsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use windows::{
    core::{Result as WinResult, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_POINT_2F,
                    D2D_RECT_F,
                },
                D2D1CreateFactory, ID2D1Factory, ID2D1RenderTarget, ID2D1SolidColorBrush,
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_NONE,
                D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
                D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
                D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
            },
            DirectWrite::{
                DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
                DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                DWRITE_TEXT_ALIGNMENT_CENTER,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{
                CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
                SelectObject, AC_SRC_ALPHA, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
                DIB_RGB_COLORS, HDC,
            },
            Imaging::{
                CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
                WICBitmapCacheOnLoad,
            },
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                COINIT_MULTITHREADED,
            },
            LibraryLoader::GetModuleHandleW,
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
            RegisterClassW, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
            HWND_TOPMOST, MSG, PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
            SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_DESTROY, WM_ERASEBKGND, WNDCLASSW,
            WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
            WS_POPUP,
        },
    },
};

use super::NativeKeyVisual;
use crate::app::diagnostics::record_error;

#[derive(Clone, Default)]
pub struct NativeKeyOverlay {
    sender: Option<Sender<KeyCommand>>,
}

static KEY_WINDOW_HWND: AtomicIsize = AtomicIsize::new(0);

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
    font_size: f32,
    corner_radius: f32,
    background_enabled: bool,
    background_color: Color,
}

#[derive(Clone, Copy)]
struct Theme {
    key: Color,
    text: Color,
    border: Color,
    border_width: f32,
    shadow: Color,
    shadow_y: i32,
}

#[derive(Clone, Copy)]
struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

struct SoftwareRenderer {
    d2d_factory: ID2D1Factory,
    wic_factory: IWICImagingFactory,
    dwrite_factory: IDWriteFactory,
}

struct DibSurface {
    memory_dc: windows::Win32::Graphics::Gdi::CreatedHDC,
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    old_bitmap: windows::Win32::Graphics::Gdi::HGDIOBJ,
    bits: *mut c_void,
}

impl Drop for DibSurface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.memory_dc, self.old_bitmap);
            DeleteObject(self.bitmap);
            DeleteDC(self.memory_dc);
        }
    }
}

impl NativeKeyOverlay {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            if let Err(error) = run_window(receiver) {
                record_error(format!("Native Direct2D key overlay failed: {error}"));
            }
        });
        Self {
            sender: Some(sender),
        }
    }

    pub fn diagnostic_hwnds(&self) -> Vec<i64> {
        let hwnd = KEY_WINDOW_HWND.load(Ordering::Relaxed);
        if hwnd == 0 {
            Vec::new()
        } else {
            vec![hwnd as i64]
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

impl SoftwareRenderer {
    unsafe fn new() -> WinResult<Self> {
        let d2d_factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let wic_factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        Ok(Self {
            d2d_factory,
            wic_factory,
            dwrite_factory,
        })
    }

    unsafe fn render_and_present(
        &self,
        hwnd: HWND,
        destination: POINT,
        width: i32,
        height: i32,
        model: &PaintModel,
    ) -> WinResult<()> {
        let bitmap = self.wic_factory.CreateBitmap(
            width as u32,
            height as u32,
            &GUID_WICPixelFormat32bppPBGRA,
            WICBitmapCacheOnLoad,
        )?;
        let properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let target = self
            .d2d_factory
            .CreateWicBitmapRenderTarget(&bitmap, &properties)?;
        target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        self.draw_model(&target, model)?;

        let stride = width as u32 * 4;
        let mut pixels = vec![0u8; stride as usize * height as usize];
        bitmap.CopyPixels(std::ptr::null(), stride, &mut pixels)?;
        self.present_pixels(hwnd, destination, width, height, &pixels)
    }

    unsafe fn draw_model(&self, target: &ID2D1RenderTarget, model: &PaintModel) -> WinResult<()> {
        let family = wide("Microsoft JhengHei");
        let locale = wide("zh-TW");
        let text_format = self.dwrite_factory.CreateTextFormat(
            PCWSTR(family.as_ptr()),
            None,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            model.font_size,
            PCWSTR(locale.as_ptr()),
        )?;
        text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
        text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

        let key_brush = solid_brush(target, model.theme.key)?;
        let text_brush = solid_brush(target, model.theme.text)?;
        let border_brush = solid_brush(target, model.theme.border)?;
        let shadow_brush = solid_brush(target, model.theme.shadow)?;
        let background_brush = solid_brush(target, model.background_color)?;

        target.BeginDraw();
        let transparent = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        target.Clear(Some(&transparent));

        for group in &model.groups {
            if model.background_enabled {
                fill_rounded_rect(target, group.rect, model.corner_radius, &background_brush);
            }
            for key in &group.keys {
                self.draw_key(
                    target,
                    key,
                    model,
                    &text_format,
                    &key_brush,
                    &text_brush,
                    &border_brush,
                    &shadow_brush,
                );
            }
            for (x, center_y) in &group.plus_positions {
                let rect = D2D_RECT_F {
                    left: (*x - model.font_size as i32) as f32,
                    top: (*center_y - model.font_size as i32) as f32,
                    right: (*x + model.font_size as i32) as f32,
                    bottom: (*center_y + model.font_size as i32) as f32,
                };
                target.DrawText(
                    &['+' as u16],
                    &text_format,
                    &rect,
                    &text_brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }

        target.EndDraw(None, None)
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_key(
        &self,
        target: &ID2D1RenderTarget,
        key: &KeyLayout,
        model: &PaintModel,
        text_format: &IDWriteTextFormat,
        key_brush: &ID2D1SolidColorBrush,
        text_brush: &ID2D1SolidColorBrush,
        border_brush: &ID2D1SolidColorBrush,
        shadow_brush: &ID2D1SolidColorBrush,
    ) {
        let shadow_y = if key.pressed {
            model.theme.shadow_y.min(2)
        } else {
            model.theme.shadow_y
        };
        if shadow_y > 0 && model.theme.shadow.alpha > 0 {
            let shadow = RECT {
                top: key.rect.top + shadow_y,
                bottom: key.rect.bottom + shadow_y,
                ..key.rect
            };
            fill_rounded_rect(target, shadow, model.corner_radius, shadow_brush);
        }

        let rounded = rounded_rect(key.rect, model.corner_radius);
        target.FillRoundedRectangle(&rounded, key_brush);
        if model.theme.border_width > 0.0 && model.theme.border.alpha > 0 {
            target.DrawRoundedRectangle(&rounded, border_brush, model.theme.border_width, None);
        }

        if let Some(kind) = &key.mouse_kind {
            draw_mouse_icon(target, key.rect, kind, text_brush);
        } else {
            let text: Vec<u16> = key.label.encode_utf16().collect();
            let rect = rect_f(key.rect);
            target.DrawText(
                &text,
                text_format,
                &rect,
                text_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    unsafe fn present_pixels(
        &self,
        hwnd: HWND,
        destination: POINT,
        width: i32,
        height: i32,
        pixels: &[u8],
    ) -> WinResult<()> {
        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            return Err(windows::core::Error::from_win32());
        }
        let memory_dc = CreateCompatibleDC(screen_dc);
        ReleaseDC(HWND(0), screen_dc);
        if memory_dc.0 == 0 {
            return Err(windows::core::Error::from_win32());
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
        let mut bits = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(
            memory_dc,
            &mut bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            HANDLE(0),
            0,
        ) {
            Ok(bitmap) => bitmap,
            Err(error) => {
                DeleteDC(memory_dc);
                return Err(error);
            }
        };
        if bits.is_null() {
            DeleteObject(bitmap);
            DeleteDC(memory_dc);
            return Err(windows::core::Error::from_win32());
        }
        let old_bitmap = SelectObject(memory_dc, bitmap);
        let surface = DibSurface {
            memory_dc,
            bitmap,
            old_bitmap,
            bits,
        };
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), surface.bits as *mut u8, pixels.len());

        let size = SIZE {
            cx: width,
            cy: height,
        };
        let source = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: 0,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            return Err(windows::core::Error::from_win32());
        }
        let updated = UpdateLayeredWindow(
            hwnd,
            screen_dc,
            Some(&destination),
            Some(&size),
            HDC(surface.memory_dc.0),
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        ReleaseDC(HWND(0), screen_dc);
        if updated.as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_win32())
        }
    }
}

fn run_window(receiver: Receiver<KeyCommand>) -> Result<(), String> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).map_err(|error| error.to_string())?;
        let result = run_initialized_window(receiver);
        CoUninitialize();
        result
    }
}

unsafe fn run_initialized_window(receiver: Receiver<KeyCommand>) -> Result<(), String> {
    let renderer = SoftwareRenderer::new().map_err(|error| error.to_string())?;
    let class_name = wide("KeyvizDirect2DKeyOverlay");
    let window_name = wide("Keyviz Keys");
    let module = GetModuleHandleW(None).map_err(|error| error.to_string())?;
    let window_class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: module,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    if RegisterClassW(&window_class) == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }

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
    KEY_WINDOW_HWND.store(hwnd.0, Ordering::Relaxed);

    message_loop(hwnd, receiver, &renderer);
    KEY_WINDOW_HWND.store(0, Ordering::Relaxed);
    DestroyWindow(hwnd);
    Ok(())
}

unsafe fn message_loop(hwnd: HWND, receiver: Receiver<KeyCommand>, renderer: &SoftwareRenderer) {
    let mut message = MSG::default();
    loop {
        match receiver.recv_timeout(Duration::from_millis(16)) {
            Ok(KeyCommand::Update(update)) => apply_update(hwnd, update, renderer),
            Ok(KeyCommand::Hide) => {
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

unsafe fn apply_update(hwnd: HWND, update: KeyUpdate, renderer: &SoftwareRenderer) {
    if !update.visual.visible || update.visual.groups.is_empty() {
        ShowWindow(hwnd, SW_HIDE);
        return;
    }

    let (model, width, height) = build_layout(&update.visual, update.scale.max(0.5));
    let margin_x = (update.visual.margin_x * update.scale).round() as i32;
    let margin_y = (update.visual.margin_y * update.scale).round() as i32;
    let horizontal = match update.visual.alignment.as_str() {
        "top-left" | "center-left" | "bottom-left" => margin_x,
        "top-right" | "center-right" | "bottom-right" => update.monitor_width - width - margin_x,
        _ => (update.monitor_width - width) / 2,
    };
    let vertical = match update.visual.alignment.as_str() {
        "top-left" | "top-center" | "top-right" => margin_y,
        "center-left" | "center" | "center-right" => (update.monitor_height - height) / 2,
        _ => update.monitor_height - height - margin_y,
    };
    let destination = POINT {
        x: update.monitor_left + horizontal.max(0),
        y: update.monitor_top + vertical.max(0),
    };

    match renderer.render_and_present(hwnd, destination, width, height, &model) {
        Ok(()) => {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
        }
        Err(error) => {
            record_error(format!("Direct2D key overlay render failed: {error}"));
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

fn build_layout(visual: &NativeKeyVisual, scale: f64) -> (PaintModel, i32, i32) {
    let text_size = (visual.text_size.max(12.0) * scale).round() as i32;
    let font_size = (text_size as f64 * 0.72).round() as f32;
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
    let height = (content_height + padding * 2).max(1);
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
                plus_positions.push((
                    x + key_gap + font_size as i32 / 2,
                    group_y + group_height / 2,
                ));
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
            corner_radius: 8.0f32.max((text_size as f64 * 0.28).round() as f32),
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
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn solid_brush(target: &ID2D1RenderTarget, color: Color) -> WinResult<ID2D1SolidColorBrush> {
    let color = d2d_color(color);
    target.CreateSolidColorBrush(&color, None)
}

unsafe fn fill_rounded_rect(
    target: &ID2D1RenderTarget,
    rect: RECT,
    radius: f32,
    brush: &ID2D1SolidColorBrush,
) {
    let rounded = rounded_rect(rect, radius);
    target.FillRoundedRectangle(&rounded, brush);
}

unsafe fn draw_mouse_icon(
    target: &ID2D1RenderTarget,
    rect: RECT,
    kind: &str,
    brush: &ID2D1SolidColorBrush,
) {
    let height = (rect.bottom - rect.top).min(38) as f32;
    let width = (height * 2.0 / 3.0).max(18.0);
    let left = rect.left as f32 + ((rect.right - rect.left) as f32 - width) / 2.0;
    let top = rect.top as f32 + ((rect.bottom - rect.top) as f32 - height) / 2.0;
    let body = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left,
            top,
            right: left + width,
            bottom: top + height,
        },
        radiusX: width / 2.0,
        radiusY: width / 2.0,
    };
    target.DrawRoundedRectangle(&body, brush, 2.0, None);
    target.DrawLine(
        D2D_POINT_2F {
            x: left,
            y: top + height / 3.0,
        },
        D2D_POINT_2F {
            x: left + width,
            y: top + height / 3.0,
        },
        brush,
        2.0,
        None,
    );
    target.DrawLine(
        D2D_POINT_2F {
            x: left + width / 2.0,
            y: top,
        },
        D2D_POINT_2F {
            x: left + width / 2.0,
            y: top + height / 3.0,
        },
        brush,
        2.0,
        None,
    );
    let marker_x = match kind {
        "Left" => left + width / 4.0,
        "Right" => left + width * 3.0 / 4.0,
        _ => left + width / 2.0,
    };
    let marker = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: marker_x - 2.0,
            top: top + 4.0,
            right: marker_x + 2.0,
            bottom: top + 10.0,
        },
        radiusX: 2.0,
        radiusY: 2.0,
    };
    target.FillRoundedRectangle(&marker, brush);
}

fn theme(style: &str, scale: f64) -> Theme {
    let scaled = |value: f32| (value as f64 * scale).max(1.0) as f32;
    let shadow_y = |value: i32| (value as f64 * scale).round().max(1.0) as i32;
    match style {
        "outline" => Theme {
            key: rgb(255, 255, 255),
            text: rgb(36, 39, 43),
            border: rgb(48, 52, 58),
            border_width: scaled(2.0),
            shadow: transparent(),
            shadow_y: 0,
        },
        "raised" => raised_theme(
            rgb(247, 249, 251),
            rgb(36, 39, 43),
            rgba(180, 187, 196, 210),
            scale,
        ),
        "dark" => raised_theme(
            rgb(32, 40, 56),
            rgb(255, 255, 255),
            rgba(8, 12, 22, 220),
            scale,
        ),
        "retro" => raised_theme(
            rgb(246, 232, 184),
            rgb(59, 52, 35),
            rgba(189, 152, 75, 220),
            scale,
        ),
        "mint" => raised_theme(
            rgb(237, 255, 249),
            rgb(16, 63, 55),
            rgba(75, 200, 168, 210),
            scale,
        ),
        "rose" => raised_theme(
            rgb(255, 242, 245),
            rgb(83, 34, 47),
            rgba(239, 134, 161, 210),
            scale,
        ),
        _ => Theme {
            key: rgb(255, 255, 255),
            text: rgb(32, 33, 36),
            border: rgb(226, 231, 238),
            border_width: scaled(1.0),
            shadow: rgba(180, 183, 188, 180),
            shadow_y: shadow_y(3),
        },
    }
}

fn raised_theme(key: Color, text: Color, shadow: Color, scale: f64) -> Theme {
    Theme {
        key,
        text,
        border: shadow,
        border_width: (1.0 * scale).max(1.0) as f32,
        shadow,
        shadow_y: (6.0 * scale).round().max(1.0) as i32,
    }
}

fn parse_color(value: &str) -> Color {
    let value = value.trim_start_matches('#');
    let rgb_value = u32::from_str_radix(value.get(..6).unwrap_or(value), 16).unwrap_or(0xffffff);
    let alpha = value
        .get(6..8)
        .and_then(|alpha| u8::from_str_radix(alpha, 16).ok())
        .unwrap_or(255);
    rgba(
        ((rgb_value >> 16) & 0xff) as u8,
        ((rgb_value >> 8) & 0xff) as u8,
        (rgb_value & 0xff) as u8,
        alpha,
    )
}

const fn rgb(red: u8, green: u8, blue: u8) -> Color {
    rgba(red, green, blue, 255)
}

const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Color {
    Color {
        red,
        green,
        blue,
        alpha,
    }
}

const fn transparent() -> Color {
    rgba(0, 0, 0, 0)
}

fn d2d_color(color: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.red as f32 / 255.0,
        g: color.green as f32 / 255.0,
        b: color.blue as f32 / 255.0,
        a: color.alpha as f32 / 255.0,
    }
}

fn rect_f(rect: RECT) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.left as f32,
        top: rect.top as f32,
        right: rect.right as f32,
        bottom: rect.bottom as f32,
    }
}

fn rounded_rect(rect: RECT, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: rect_f(rect),
        radiusX: radius,
        radiusY: radius,
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
