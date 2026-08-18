use std::{path::Path, process::Command};

use anyhow::{Context, Result};

pub(super) fn launch_settings_process(config_path: &Path) -> Result<()> {
    Command::new(settings_exe_path()?)
        .arg("--config")
        .arg(config_path)
        .spawn()
        .context("Start settings process failed")?;
    Ok(())
}

fn settings_exe_path() -> Result<std::path::PathBuf> {
    Ok(std::env::current_exe()?.with_file_name("x-desk-settings.exe"))
}
