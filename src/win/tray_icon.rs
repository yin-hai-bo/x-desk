use std::{
    io,
    mem::zeroed,
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, POINT},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Shell::{
            NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION, NOTIFYICON_VERSION_4,
            NOTIFYICONDATAW, Shell_NotifyIconW,
        },
        WindowsAndMessaging::{
            DestroyMenu, GetCursorPos, GetSubMenu, IDI_APPLICATION, LoadIconW, LoadMenuW, MB_ICONINFORMATION, MB_OK,
            MessageBoxW, PostQuitMessage, SW_RESTORE, SetForegroundWindow, SetMenuDefaultItem, ShowWindow,
            TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP, WM_CONTEXTMENU, WM_LBUTTONDBLCLK, WM_RBUTTONUP,
        },
    },
};

use super::{
    resource_ids::{IDI_APP_ICON, IDR_TRAY_MENU, MENU_ABOUT, MENU_EXIT, MENU_SHOW_MAIN_WINDOW},
    wide_null,
};

const TRAY_ICON_ID: u32 = 1;
pub(super) const TRAY_ICON_MESSAGE: u32 = WM_APP + 1;

pub(super) struct TrayIcon {
    data: NOTIFYICONDATAW,
}

impl TrayIcon {
    pub(super) fn new(window: HWND, tip: &str) -> io::Result<Self> {
        let instance = unsafe { GetModuleHandleW(null()) };
        let mut icon = unsafe { LoadIconW(instance, IDI_APP_ICON as usize as _) };
        if icon.is_null() {
            icon = unsafe { LoadIconW(null_mut(), IDI_APPLICATION) };
        }
        if icon.is_null() {
            return Err(io::Error::last_os_error());
        }

        let mut data = unsafe { zeroed::<NOTIFYICONDATAW>() };
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = window;
        data.uID = TRAY_ICON_ID;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = TRAY_ICON_MESSAGE;
        data.hIcon = icon;
        write_tip(&mut data.szTip, tip);

        if unsafe { Shell_NotifyIconW(NIM_ADD, &data) } == 0 {
            return Err(io::Error::last_os_error());
        }

        unsafe {
            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &data);
        }

        Ok(Self { data })
    }
}

pub(super) fn handle_message(window: HWND, lparam: LPARAM) {
    match tray_event(lparam) {
        WM_LBUTTONDBLCLK => show_main_window(window),
        WM_RBUTTONUP | WM_CONTEXTMENU => show_context_menu(window),
        _ => {}
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &self.data);
        }
    }
}

fn write_tip(buffer: &mut [u16; 128], tip: &str) {
    let encoded = wide_null(tip);
    let length = encoded.len().min(buffer.len());
    buffer[..length].copy_from_slice(&encoded[..length]);
    buffer[buffer.len() - 1] = 0;
}

fn tray_event(lparam: LPARAM) -> u32 {
    (lparam as u32) & 0xffff
}

fn show_context_menu(window: HWND) {
    let menu = unsafe { LoadMenuW(GetModuleHandleW(null()), IDR_TRAY_MENU as usize as _) };
    if menu.is_null() {
        return;
    }

    let popup_menu = unsafe { GetSubMenu(menu, 0) };
    if popup_menu.is_null() {
        unsafe { DestroyMenu(menu) };
        return;
    }

    unsafe { SetMenuDefaultItem(popup_menu, MENU_SHOW_MAIN_WINDOW as u32, 0) };

    let mut cursor = POINT { x: 0, y: 0 };
    let command = unsafe {
        if GetCursorPos(&mut cursor) == 0 {
            DestroyMenu(menu);
            return;
        }

        SetForegroundWindow(window);
        TrackPopupMenu(
            popup_menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            cursor.x,
            cursor.y,
            0,
            window,
            null(),
        )
    };

    unsafe {
        DestroyMenu(menu);
    }

    match command as usize {
        MENU_SHOW_MAIN_WINDOW => show_main_window(window),
        MENU_ABOUT => show_about(window),
        MENU_EXIT => unsafe { PostQuitMessage(0) },
        _ => {}
    }
}

fn show_main_window(window: HWND) {
    unsafe {
        ShowWindow(window, SW_RESTORE);
        SetForegroundWindow(window);
    }
}

fn show_about(window: HWND) {
    let title = wide_null("About X-Desk");
    let message = wide_null("X-Desk");

    unsafe {
        MessageBoxW(window, message.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
    }
}
