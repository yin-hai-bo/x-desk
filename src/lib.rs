mod beacon;
mod config;
mod logger;

#[cfg(target_os = "windows")]
mod win;

const APP_NAME: &str = "x-desk";

pub fn run_app() -> anyhow::Result<()> {
    logger::init();

    #[cfg(target_os = "windows")]
    {
        use win::{main_window::MainWindow, wallpaper_manager::WallpaperManager};
        use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};

        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }

        let beacon = beacon::Beacon::new();
        let config_file_path = config::Config::config_file_path(APP_NAME)?;
        let config = config::Config::load_from_file(&config_file_path)?;

        let mut wallpaper_manager = WallpaperManager::new(&beacon);
        let _ = wallpaper_manager.refresh_wallpapers(|index| config.video_url_for_monitor(index).map(str::to_string));

        let mut main_window = MainWindow::create(&beacon, APP_NAME, config, config_file_path)?;
        main_window.run()
    }

    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("Unsupported OS.");
    }
}
