#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use crate::config::Config;
#[cfg(not(target_os = "windows"))]
use std::env;
use std::io;
#[cfg(not(target_os = "windows"))]
use std::{io::Error, path::PathBuf};

mod config;
#[cfg(target_os = "windows")]
mod win;

const APP_NAME: &str = "x-desk";

fn load_config() -> io::Result<Config> {
    #[cfg(target_os = "windows")]
    let base_dir = win::appdata_dir()?;

    #[cfg(not(target_os = "windows"))]
    let base_dir = current_exe_dir()?;

    let path = base_dir.join("yinhaibo").join(APP_NAME).join("config.json");
    Config::load_from_file(path)
}

#[cfg(not(target_os = "windows"))]
fn current_exe_dir() -> io::Result<PathBuf> {
    let path = env::current_exe()?;
    path.parent()
        .map(PathBuf::from)
        .ok_or_else(|| Error::new(io::ErrorKind::NotFound, "Get application directory failed."))
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        let config = load_config().ok();
        if let Err(error) = win::main_window::run(config) {
            eprintln!("failed to create main window: {error}");
            std::process::exit(1);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("Unsupported OS.");
        std::process::exit(1);
    }
}
