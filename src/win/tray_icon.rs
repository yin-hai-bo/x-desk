use std::{io, mem::zeroed};

use windows::{
    Win32::{
        Foundation::{FALSE, HWND, LPARAM, POINT},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION, NOTIFYICON_VERSION_4,
                NOTIFYICONDATAW, Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                GetCursorPos, IDI_APPLICATION, LoadIconW, MB_ICONINFORMATION, MB_OK, MessageBoxW, PostQuitMessage,
                SW_RESTORE, SetForegroundWindow, ShowWindow, WM_CONTEXTMENU, WM_LBUTTONDBLCLK, WM_RBUTTONUP,
            },
        },
    },
    core::{PCWSTR, Result},
};

use crate::win::wide_string::WideString;

use super::{
    menu::Menu,
    resource_ids::{IDI_APP_ICON, IDR_TRAY_MENU, MENU_ABOUT, MENU_EXIT, MENU_SHOW_MAIN_WINDOW},
};

const TRAY_ICON_ID: u32 = 1;

pub(super) struct TrayIcon {
    data: NOTIFYICONDATAW,
    app_name: String,
}

impl TrayIcon {
    pub fn new(app_name: &str, hwnd: HWND, callback_message_id: u32) -> anyhow::Result<Self> {
        let instance = unsafe { GetModuleHandleW(PCWSTR::null()) }?.into();
        let mut icon = unsafe { LoadIconW(Some(instance), PCWSTR(IDI_APP_ICON as usize as *const u16)) };
        if icon.is_err() {
            icon = unsafe { LoadIconW(None, IDI_APPLICATION) };
        }
        if icon.is_err() {
            return Err(io::Error::last_os_error().into());
        }
        let icon = icon.unwrap();

        let mut data = unsafe { zeroed::<NOTIFYICONDATAW>() };
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = TRAY_ICON_ID;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = callback_message_id;
        data.hIcon = icon;

        WideString::new(app_name).copy_to(&mut data.szTip);

        if unsafe { Shell_NotifyIconW(NIM_ADD, &data) } == FALSE {
            return Err(io::Error::last_os_error().into());
        }

        unsafe {
            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
        }

        Ok(Self {
            data,
            app_name: app_name.to_string(),
        })
    }

    pub fn handle_message(&self, window: HWND, lparam: LPARAM) {
        match lparam.0 as u32 & 0xffff {
            WM_LBUTTONDBLCLK => Self::show_main_window(window),
            WM_RBUTTONUP | WM_CONTEXTMENU => {
                let _ = self.show_context_menu(window);
            }
            _ => {}
        }
    }

    fn show_context_menu(&self, window: HWND) -> Result<()> {
        let menu = Menu::load(IDR_TRAY_MENU)?;
        let popup_menu = menu.get_sub_menu()?;
        Menu::set_default_item(popup_menu, MENU_SHOW_MAIN_WINDOW);

        let mut cursor = POINT::default();
        unsafe {
            GetCursorPos(&mut cursor)?;
            let _ = SetForegroundWindow(window);
        }

        let command = Menu::track_popup_menu(popup_menu, cursor.x, cursor.y, window);
        match command {
            MENU_SHOW_MAIN_WINDOW => Self::show_main_window(window),
            MENU_ABOUT => self.show_about(window),
            MENU_EXIT => unsafe { PostQuitMessage(0) },
            _ => {}
        };
        Ok(())
    }

    fn show_main_window(window: HWND) {
        unsafe {
            let _ = ShowWindow(window, SW_RESTORE);
            let _ = SetForegroundWindow(window);
        }
    }

    fn show_about(&self, window: HWND) {
        let title = format!("About {}", self.app_name);
        let message = format!("{} {}", self.app_name, env!("CARGO_PKG_VERSION"));
        unsafe {
            MessageBoxW(
                Some(window),
                WideString::new(&message).as_pcwstr(),
                WideString::new(&title).as_pcwstr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
        }
    }
}
