use std::{ffi::OsString, io, os::windows::ffi::OsStringExt, path::PathBuf, ptr::null_mut};

use windows_sys::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

pub mod main_window;
mod resource_ids;
mod theme;
mod tray_icon;

pub(crate) fn appdata_dir() -> io::Result<PathBuf> {
    let mut path = null_mut();
    let result =
        unsafe { SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT as u32, null_mut(), &mut path) };
    if result < 0 {
        return Err(io::Error::other(format!(
            "SHGetKnownFolderPath failed: 0x{:08x}",
            result as u32
        )));
    }

    let length = unsafe {
        let mut length = 0;
        while *path.add(length) != 0 {
            length += 1;
        }
        length
    };
    let value = unsafe { OsString::from_wide(std::slice::from_raw_parts(path, length)) };
    unsafe { CoTaskMemFree(path.cast()) };

    Ok(PathBuf::from(value))
}

pub(crate) fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
