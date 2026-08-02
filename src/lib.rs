mod config;
mod logger;

#[cfg(target_os = "windows")]
mod win;

use anyhow::Result;

const APP_NAME: &str = "x-desk";

pub fn run_app() -> Result<()> {
    logger::init();
    match do_run_app() {
        Ok(_) => Ok(()),
        Err(e) => {
            log::error!("{:#}", e);
            Err(e)
        }
    }
}

#[cfg(target_os = "windows")]
fn do_run_app() -> Result<()> {
    use win::main_window::MainWindow;
    use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let config_file_path = config::Config::config_file_path(APP_NAME)?;
    let config = config::Config::load_from_file(&config_file_path)?;
    let mut main_window = MainWindow::create(APP_NAME, config, config_file_path)?;
    main_window.run()
}

#[cfg(not(target_os = "windows"))]
fn do_run_app() {
    anyhow::bail!("Unsupported OS.");
}
