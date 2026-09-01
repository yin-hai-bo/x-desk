use std::path::Path;

use anyhow::{Context, Result};
use windows::Win32::{
    Foundation::CloseHandle,
    System::Threading::{CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, STARTUPINFOW},
};

use crate::win::wide_string::WideString;

pub(super) fn launch_main_ui_process(config_path: &Path) -> Result<()> {
    let _ = config_path;
    let exe_path = main_ui_exe_path()?;
    let exe_path = WideString::from_os_string(exe_path.as_os_str());
    let startup_info = STARTUPINFOW::default();
    let mut process_info = PROCESS_INFORMATION::default();

    unsafe {
        CreateProcessW(
            exe_path.as_pcwstr(),
            None,
            None,
            None,
            false,
            PROCESS_CREATION_FLAGS(0),
            None,
            None,
            &startup_info,
            &mut process_info,
        )
    }
    .context("Start main UI process failed")?;

    unsafe {
        let _ = CloseHandle(process_info.hThread);
        let _ = CloseHandle(process_info.hProcess);
    }

    Ok(())
}

pub(super) fn request_main_ui_exit() -> bool {
    single_instance::SingleInstance::request_exit(common::MAIN_UI_INSTANCE_NAME)
}

fn main_ui_exe_path() -> Result<std::path::PathBuf> {
    Ok(std::env::current_exe()?.with_file_name("x-desk-main-ui.exe"))
}
