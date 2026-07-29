use windows::{
    Win32::{
        Foundation::HWND,
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DestroyMenu, GetSubMenu, HMENU, LoadMenuW, SetMenuDefaultItem, TPM_RETURNCMD, TPM_RIGHTBUTTON,
            TrackPopupMenu,
        },
    },
    core::{Error, PCWSTR, Result},
};

pub struct Menu {
    handle: HMENU,
}

impl Menu {
    pub fn load(menu_id: u16) -> Result<Self> {
        let instance = unsafe { GetModuleHandleW(PCWSTR::null())?.into() };
        let handle = unsafe { LoadMenuW(Some(instance), PCWSTR(menu_id as usize as *const u16))? };
        Ok(Self { handle })
    }

    pub fn get_sub_menu(&self) -> Result<HMENU> {
        let sub = unsafe { GetSubMenu(self.handle, 0) };
        if sub.is_invalid() {
            Err(Error::from_thread())
        } else {
            Ok(sub)
        }
    }

    pub fn set_default_item(menu: HMENU, id: u32) {
        unsafe {
            let _ = SetMenuDefaultItem(menu, id, 0);
        };
    }

    pub fn track_popup_menu(popup_menu: HMENU, x: i32, y: i32, window: HWND) -> u32 {
        unsafe { TrackPopupMenu(popup_menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, x, y, None, window, None).0 as u32 }
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        if !self.handle.0.is_null() {
            let _ = unsafe { DestroyMenu(self.handle) };
            self.handle.0 = std::ptr::null_mut();
        }
    }
}
