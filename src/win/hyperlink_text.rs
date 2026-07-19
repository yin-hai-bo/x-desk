use std::{
    io,
    ptr::{null, null_mut},
    sync::OnceLock,
};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateSolidBrush, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW,
        EndPaint, FW_NORMAL, FillRect, HBRUSH, HFONT, InvalidateRect, PAINTSTRUCT, SelectObject, SetBkMode,
        SetTextColor, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
        GetClientRect, GetWindowLongPtrW, IDC_ARROW, IDC_HAND, IsWindow, LoadCursorW, RegisterClassW, SW_SHOW,
        SetCursor, SetWindowLongPtrW, ShowWindow, WM_ERASEBKGND, WM_LBUTTONUP, WM_NCCREATE, WM_NCDESTROY, WM_PAINT,
        WM_SETCURSOR, WM_SETTINGCHANGE, WNDCLASSW, WS_CHILD, WS_VISIBLE,
    },
};

use super::{theme, wide_null};

const CLASS_NAME: &str = "YHB-XDeskHyperLinkText";

pub struct HyperLinkFont {
    name: String,
    point_size: i32,
}

impl HyperLinkFont {
    pub fn new(name: impl Into<String>, point_size: i32) -> Self {
        Self {
            name: name.into(),
            point_size,
        }
    }
}

pub struct HyperLinkText {
    window: HWND,
    _state: Box<HyperLinkState>,
}

impl HyperLinkText {
    pub fn new_right_aligned<F>(
        parent: HWND,
        text: &str,
        bounds: RECT,
        font: HyperLinkFont,
        on_click: F,
    ) -> io::Result<Self>
    where
        F: FnMut() + 'static,
    {
        Self::create(parent, text, bounds, font, DT_RIGHT, on_click)
    }

    fn create<F>(
        parent: HWND,
        text: &str,
        bounds: RECT,
        font: HyperLinkFont,
        draw_alignment: u32,
        on_click: F,
    ) -> io::Result<Self>
    where
        F: FnMut() + 'static,
    {
        register_window_class()?;

        let mut state = Box::new(HyperLinkState {
            text: wide_null(text),
            font: create_underline_font(&font)?,
            draw_alignment,
            on_click: Box::new(on_click),
        });

        let class_name = wide_null(CLASS_NAME);
        let window = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                state.text.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                bounds.left,
                bounds.top,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                parent,
                null_mut(),
                GetModuleHandleW(null()),
                state.as_mut() as *mut HyperLinkState as *mut _,
            )
        };
        if window.is_null() {
            return Err(io::Error::last_os_error());
        }

        unsafe { ShowWindow(window, SW_SHOW) };

        Ok(Self { window, _state: state })
    }
}

impl Drop for HyperLinkText {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.window) != 0 {
                DestroyWindow(self.window);
            }
        }
    }
}

struct HyperLinkState {
    text: Vec<u16>,
    font: HFONT,
    draw_alignment: u32,
    on_click: Box<dyn FnMut()>,
}

impl Drop for HyperLinkState {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.font);
        }
    }
}

fn register_window_class() -> io::Result<()> {
    static REGISTERED: OnceLock<io::Result<()>> = OnceLock::new();

    REGISTERED
        .get_or_init(|| {
            let class_name = wide_null(CLASS_NAME);
            let instance = unsafe { GetModuleHandleW(null()) };
            let window_class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: null_mut(),
                hCursor: unsafe { LoadCursorW(null_mut(), IDC_HAND) },
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class_name.as_ptr(),
            };

            if unsafe { RegisterClassW(&window_class) } == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        })
        .as_ref()
        .map(|_| ())
        .map_err(|error| io::Error::new(error.kind(), error.to_string()))
}

fn create_underline_font(font: &HyperLinkFont) -> io::Result<HFONT> {
    let font_name = wide_null(&font.name);
    let height = -((font.point_size.max(1) * 96) / 72);
    let handle = unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            font_name.as_ptr(),
        )
    };
    if handle.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(handle)
    }
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_NCCREATE => {
            let create = lparam as *const CREATESTRUCTW;
            unsafe {
                SetWindowLongPtrW(window, GWLP_USERDATA, (*create).lpCreateParams as isize);
            }
            1
        }
        WM_PAINT => {
            unsafe { paint(window) };
            0
        }
        WM_ERASEBKGND => 1,
        WM_SETTINGCHANGE => {
            unsafe { InvalidateRect(window, null(), 1) };
            0
        }
        WM_SETCURSOR => {
            unsafe {
                let mut cursor = LoadCursorW(null_mut(), IDC_HAND);
                if cursor.is_null() {
                    cursor = LoadCursorW(null_mut(), IDC_ARROW);
                }
                SetCursor(cursor);
            }
            1
        }
        WM_LBUTTONUP => {
            if let Some(state) = unsafe { state_mut(window) } {
                (state.on_click)();
            }
            0
        }
        WM_NCDESTROY => {
            unsafe {
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

unsafe fn paint(window: HWND) {
    let mut paint = unsafe { std::mem::zeroed::<PAINTSTRUCT>() };
    let device_context = unsafe { BeginPaint(window, &mut paint) };
    if device_context.is_null() {
        return;
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        GetClientRect(window, &mut rect);
    }

    let brush: HBRUSH = unsafe { CreateSolidBrush(theme::background_color()) };
    if !brush.is_null() {
        unsafe {
            FillRect(device_context, &rect, brush);
            DeleteObject(brush);
        }
    }

    if let Some(state) = unsafe { state_mut(window) } {
        unsafe {
            let old_font = SelectObject(device_context, state.font);
            SetBkMode(device_context, TRANSPARENT as i32);
            SetTextColor(device_context, theme::hyperlink_text_color());
            DrawTextW(
                device_context,
                state.text.as_ptr(),
                -1,
                &mut rect,
                state.draw_alignment | DT_VCENTER | DT_SINGLELINE,
            );
            SelectObject(device_context, old_font);
        }
    }

    unsafe { EndPaint(window, &paint) };
}

unsafe fn state_mut(window: HWND) -> Option<&'static mut HyperLinkState> {
    let value = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) };
    if value == 0 {
        None
    } else {
        Some(unsafe { &mut *(value as *mut HyperLinkState) })
    }
}
