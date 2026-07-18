use std::{
    io,
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{HDC, UpdateWindow},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        AdjustWindowRectEx, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
        GetSystemMetrics, IDC_ARROW, LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN,
        SW_SHOW, ShowWindow, TranslateMessage, WM_DESTROY, WM_ERASEBKGND, WM_SETTINGCHANGE, WNDCLASSW, WS_CAPTION,
        WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
    },
};

use super::{theme, tray_icon::TrayIcon, wide_null};

pub fn run() -> io::Result<()> {
    unsafe {
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(io::Error::last_os_error());
        }

        let class_name = wide_null("YHB-XDeskMainWindow");
        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
        };

        if RegisterClassW(&window_class) == 0 {
            return Err(io::Error::last_os_error());
        }

        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 480,
            bottom: 320,
        };

        if AdjustWindowRectEx(&mut rect, style, 0, 0) == 0 {
            return Err(io::Error::last_os_error());
        }

        let window_width = rect.right - rect.left;
        let window_height = rect.bottom - rect.top;
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);
        let window_x = (screen_width - window_width) / 2;
        let window_y = (screen_height - window_height) / 2;

        let title_text = "X-Desk";
        let title = wide_null(title_text);
        let window = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            style,
            window_x,
            window_y,
            window_width,
            window_height,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );

        if window.is_null() {
            return Err(io::Error::last_os_error());
        }

        let _tray_icon = TrayIcon::new(window, title_text)?;

        theme::apply_system_theme(window);
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);

        run_message_loop()
    }
}

fn run_message_loop() -> io::Result<()> {
    let mut message = MSG {
        hwnd: null_mut(),
        message: 0,
        wParam: 0,
        lParam: 0,
        time: 0,
        pt: POINT { x: 0, y: 0 },
    };

    loop {
        let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }

        if result == 0 {
            return Ok(());
        }

        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_ERASEBKGND => {
            unsafe { theme::paint_background(window, wparam as HDC) };
            1
        }
        WM_SETTINGCHANGE => {
            theme::system_theme_changed(window);
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
