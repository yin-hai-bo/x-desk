use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, RECT},
        Graphics::{
            Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
            Gdi::{
                CreateSolidBrush, DeleteObject, FillRect, HDC, InvalidateRect, RDW_ALLCHILDREN, RDW_ERASE,
                RDW_INVALIDATE, RedrawWindow,
            },
        },
        System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW},
        UI::WindowsAndMessaging::GetClientRect,
    },
    core::w,
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum SystemTheme {
    Light,
    Dark,
}

pub(super) fn apply_system_theme(window: HWND) {
    if !is_windows_build_or_later(17763) {
        return;
    }

    let use_dark_mode = match system_theme() {
        SystemTheme::Light => 0i32,
        SystemTheme::Dark => 1i32,
    };

    unsafe {
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &use_dark_mode as *const i32 as *const _,
            size_of::<i32>() as u32,
        );
    }
}

fn is_windows_build_or_later(min_build: u32) -> bool {
    let subkey = w!("Software\\Microsoft\\Windows NT\\CurrentVersion");
    let value_name = w!("CurrentBuildNumber");
    let mut value = [0u16; 32];
    let mut value_size = (value.len() * size_of::<u16>()) as u32;

    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey,
            value_name,
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr() as *mut _),
            Some(&mut value_size),
        )
    };
    if result.is_err() {
        return false;
    }

    let len = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len])
        .parse::<u32>()
        .is_ok_and(|build| build >= min_build)
}

pub(super) unsafe fn paint_background(window: HWND, device_context: HDC) {
    let brush = unsafe { CreateSolidBrush(background_color()) };
    if brush.is_invalid() {
        return;
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    unsafe {
        if GetClientRect(window, &mut rect).is_ok() {
            let _ = FillRect(device_context, &rect, brush);
        }
        let _ = DeleteObject(brush.into());
    }
}

pub(super) fn system_theme_changed(window: HWND) {
    apply_system_theme(window);
    unsafe {
        let _ = InvalidateRect(Some(window), None, true);
        let _ = RedrawWindow(Some(window), None, None, RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN);
    }
}

pub(super) fn background_color() -> COLORREF {
    match system_theme() {
        SystemTheme::Light => COLORREF(0x00ffffff),
        SystemTheme::Dark => COLORREF(0x00202020),
    }
}

pub(super) fn hyperlink_text_color() -> COLORREF {
    match system_theme() {
        SystemTheme::Light => COLORREF(0x00cc6600),
        SystemTheme::Dark => COLORREF(0x00ffb56b),
    }
}

fn system_theme() -> SystemTheme {
    let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value_name = w!("AppsUseLightTheme");
    let mut value = 1u32;
    let mut value_size = size_of::<u32>() as u32;

    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey,
            value_name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut value_size),
        )
    };

    if result.is_ok() && value == 0 {
        SystemTheme::Dark
    } else {
        SystemTheme::Light
    }
}
