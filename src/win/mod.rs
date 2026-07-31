use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};

use windows::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

pub mod const_define;
pub mod desktop;
pub mod dock;
pub mod hyperlink_text;
pub mod main_window;
pub mod menu;
pub mod monitor;
pub mod resource_ids;
pub mod theme;
pub mod tray_icon;
pub mod wallpaper_manager;
pub mod wide_string;
pub mod win_utils;
pub mod window;

pub(crate) fn appdata_dir() -> anyhow::Result<PathBuf> {
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None)? };
    let ws = unsafe { path.as_wide() };
    let os = OsString::from_wide(ws);
    unsafe { CoTaskMemFree(Some(path.as_ptr() as *mut _)) };
    Ok(PathBuf::from(os))
}
