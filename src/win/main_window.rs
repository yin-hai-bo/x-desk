use std::{
    io,
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{HDC, UpdateWindow},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Shell::ShellExecuteW,
        WindowsAndMessaging::{
            AdjustWindowRectEx, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW,
            GetClientRect, GetMessageW, GetSystemMetrics, IDC_ARROW, IDI_APPLICATION, LoadCursorW, LoadIconW, MSG,
            PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, SW_SHOWNORMAL, ShowWindow,
            TranslateMessage, WM_DESTROY, WM_ERASEBKGND, WM_SETTINGCHANGE, WNDCLASSW, WS_CAPTION, WS_MINIMIZEBOX,
            WS_OVERLAPPED, WS_SYSMENU,
        },
    },
};

use crate::config::Config;

use super::{
    hyperlink_text::{HyperLinkFont, HyperLinkText},
    resource_ids::IDI_APP_ICON,
    theme,
    tray_icon::{self, TrayIcon},
    wide_null,
};

pub fn run(_config: Option<Config>, config_dir: PathBuf) -> io::Result<()> {
    unsafe {
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(io::Error::last_os_error());
        }

        let mut icon = LoadIconW(instance, IDI_APP_ICON as usize as _);
        if icon.is_null() {
            icon = LoadIconW(null_mut(), IDI_APPLICATION);
        }

        let class_name = wide_null("YHB-XDeskMainWindow");
        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: icon,
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
        let mut client_rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetClientRect(window, &mut client_rect) == 0 {
            return Err(io::Error::last_os_error());
        }

        let _config_dir_link = HyperLinkText::new_right_aligned(
            window,
            "Open configuration directory",
            RECT {
                left: 24,
                top: client_rect.bottom - 16 - 28,
                right: client_rect.right - 24,
                bottom: client_rect.bottom - 16,
            },
            HyperLinkFont::new("Segoe UI", 10),
            move || open_config_dir(&config_dir),
        )?;

        theme::apply_system_theme(window);
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);

        run_message_loop()
    }
}

fn open_config_dir(config_dir: &PathBuf) {
    let operation = wide_null("open");
    let path = wide_os_null(config_dir.as_os_str());
    unsafe {
        ShellExecuteW(
            null_mut(),
            operation.as_ptr(),
            path.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        );
    }
}

fn wide_os_null(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
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
        tray_icon::TRAY_ICON_MESSAGE => {
            tray_icon::handle_message(window, lparam);
            0
        }
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
