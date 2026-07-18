use std::ptr::{null, null_mut};

use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::{
        Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
        Gdi::{CreateSolidBrush, DeleteObject, FillRect, HDC, InvalidateRect},
    },
    System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
    UI::WindowsAndMessaging::GetClientRect,
};

use super::wide_null;

#[derive(Clone, Copy, Eq, PartialEq)]
enum SystemTheme {
    Light,
    Dark,
}

pub(super) fn apply_system_theme(window: HWND) {
    let use_dark_mode = match system_theme() {
        SystemTheme::Light => 0i32,
        SystemTheme::Dark => 1i32,
    };

    unsafe {
        DwmSetWindowAttribute(
            window,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &use_dark_mode as *const i32 as *const _,
            size_of::<i32>() as u32,
        );
    }
}

pub(super) unsafe fn paint_background(window: HWND, device_context: HDC) {
    let color = match system_theme() {
        SystemTheme::Light => 0x00ffffff,
        SystemTheme::Dark => 0x00202020,
    };
    let brush = unsafe { CreateSolidBrush(color) };
    if brush.is_null() {
        return;
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    unsafe {
        if GetClientRect(window, &mut rect) != 0 {
            FillRect(device_context, &rect, brush);
        }
        DeleteObject(brush);
    }
}

pub(super) fn system_theme_changed(window: HWND) {
    apply_system_theme(window);
    unsafe { InvalidateRect(window, null(), 1) };
}

fn system_theme() -> SystemTheme {
    let subkey = wide_null("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value_name = wide_null("AppsUseLightTheme");
    let mut value = 1u32;
    let mut value_size = size_of::<u32>() as u32;

    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            &mut value as *mut u32 as *mut _,
            &mut value_size,
        )
    };

    if result == 0 && value == 0 {
        SystemTheme::Dark
    } else {
        SystemTheme::Light
    }
}
