use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};

use windows::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

mod content_process;
mod desktop;
mod desktop_attachment;
mod dock;
mod hyperlink_text;
pub mod main_window;
mod menu;
mod monitor;
mod msg_id;
mod occlusion;
pub(crate) mod player;
mod resource_ids;
pub(crate) mod settings;
mod settings_process;
mod theme;
mod tray_icon;
mod video_host;
pub mod wallpaper_manager;
mod watcher;
pub(crate) mod webview;
mod wide_string;
mod win_utils;
mod window;

pub(crate) fn appdata_dir() -> anyhow::Result<PathBuf> {
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None)? };
    let ws = unsafe { path.as_wide() };
    let os = OsString::from_wide(ws);
    unsafe { CoTaskMemFree(Some(path.as_ptr() as *mut _)) };
    Ok(PathBuf::from(os))
}
