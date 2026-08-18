mod config;
mod logger;

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

pub fn run_player() -> Result<()> {
    win::player::run_player()
}

pub fn run_webview() -> Result<()> {
    win::webview::run_webview()
}

pub fn run_settings() -> Result<()> {
    win::settings::run_settings()
}

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
