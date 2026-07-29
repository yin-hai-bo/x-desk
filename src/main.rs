#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use crate::config::Config;
#[cfg(target_os = "windows")]
use crate::win::main_window::MainWindow;

mod config;
#[cfg(target_os = "windows")]
mod win;

const APP_NAME: &str = "x-desk";

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", error);
        std::process::exit(1);
    }
}

#[cfg(target_os = "windows")]
fn run() -> anyhow::Result<()> {
    let config_file_path = Config::config_file_path(APP_NAME)?;
    let config = Config::load_from_file(&config_file_path)?;
    let mut main_window = MainWindow::create(APP_NAME, config, config_file_path)?;
    main_window.run()
}

#[cfg(not(target_os = "windows"))]
fn run() -> anyhow::Result<()> {
    anyhow::bail!("Unsupported OS.");
}
