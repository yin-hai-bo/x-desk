use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use windows::{
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    core::w,
};

use crate::{logger, win::wide_string::WideString};

pub(crate) fn run_settings() -> Result<()> {
    logger::init();
    let args = SettingsArgs::parse()?;
    open_config_file(&args.config_path)
}

fn open_config_file(path: &Path) -> Result<()> {
    let path = WideString::from_os_string(path.as_os_str());
    let result = unsafe { ShellExecuteW(None, w!("open"), path.as_pcwstr(), None, None, SW_SHOWNORMAL) };
    if result.0 as isize <= 32 {
        bail!(
            "Open configuration file failed, ShellExecuteW returned {}",
            result.0 as isize
        );
    }
    Ok(())
}

struct SettingsArgs {
    config_path: PathBuf,
}

impl SettingsArgs {
    fn parse() -> Result<Self> {
        let mut args = std::env::args_os().skip(1);
        let mut config_path = None;
        while let Some(arg) = args.next() {
            match arg.to_string_lossy().as_ref() {
                "--config" => config_path = args.next().map(PathBuf::from),
                _ => bail!("Unknown x-desk-settings argument: {}", arg.to_string_lossy()),
            }
        }
        Ok(Self {
            config_path: config_path.context("Missing --config")?,
        })
    }
}
