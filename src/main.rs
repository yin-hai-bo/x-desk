#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
#[cfg(target_os = "windows")]
use x_desk::win::{main_window::MainWindow, wallpaper_manager::WallpaperManager};
use x_desk::{beacon::Beacon, config::Config, logger};

const APP_NAME: &str = "x-desk";

fn main() {
    logger::init();

    if let Err(error) = run() {
        log::error!("{}", error);
        std::process::exit(1);
    }
}

#[cfg(target_os = "windows")]
fn run() -> anyhow::Result<()> {

    unsafe { let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); }

    let beacon = Beacon::new();
    let config_file_path = Config::config_file_path(APP_NAME)?;
    let config = Config::load_from_file(&config_file_path)?;
    let mut main_window = MainWindow::create(&beacon, APP_NAME, config, config_file_path)?;

    let mut wallpaper_manager = WallpaperManager::new(&beacon);
    let _ = wallpaper_manager.refresh_wallpapers(|_| Some("test".to_string()));

    main_window.run()
}

#[cfg(not(target_os = "windows"))]
fn run() -> anyhow::Result<()> {
    anyhow::bail!("Unsupported OS.");
}
