use std::{io, mem::zeroed, ptr::null_mut};

use windows_sys::Win32::{
    Foundation::HWND,
    UI::{
        Shell::{
            NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION, NOTIFYICON_VERSION_4,
            NOTIFYICONDATAW, Shell_NotifyIconW,
        },
        WindowsAndMessaging::{IDI_APPLICATION, LoadIconW, WM_APP},
    },
};

use super::wide_null;

const TRAY_ICON_ID: u32 = 1;
const TRAY_ICON_MESSAGE: u32 = WM_APP + 1;

pub(super) struct TrayIcon {
    data: NOTIFYICONDATAW,
}

impl TrayIcon {
    pub(super) fn new(window: HWND, tip: &str) -> io::Result<Self> {
        let icon = unsafe { LoadIconW(null_mut(), IDI_APPLICATION) };
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
