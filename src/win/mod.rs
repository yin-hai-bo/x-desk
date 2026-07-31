use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};

use windows::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

pub(super) mod const_define;
pub(super) mod desktop;
pub(super) mod dock;
mod hyperlink_text;
pub(super) mod main_window;
mod menu;
mod monitor;
mod resource_ids;
mod theme;
mod tray_icon;
pub(super) mod wallpaper_manager;
mod wide_string;
mod win_utils;
pub(super) mod window;

pub(crate) fn appdata_dir() -> anyhow::Result<PathBuf> {
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None)? };
    let ws = unsafe { path.as_wide() };
    let os = OsString::from_wide(ws);
    unsafe { CoTaskMemFree(Some(path.as_ptr() as *mut _)) };
    Ok(PathBuf::from(os))
}
