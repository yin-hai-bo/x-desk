use std::{path::Path, process::Command};

use anyhow::{Context, Result};

pub(super) fn launch_main_ui_process(config_path: &Path) -> Result<()> {
    Command::new(main_ui_exe_path()?)
        .arg("--config")
        .arg(config_path)
        .spawn()
        .context("Start main UI process failed")?;
    Ok(())
}

fn main_ui_exe_path() -> Result<std::path::PathBuf> {
    Ok(std::env::current_exe()?.with_file_name("x-desk-main-ui.exe"))
}
