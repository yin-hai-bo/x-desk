mod win;

use anyhow::Result;

const MAIN_APP_INSTANCE_NAME: &str = "x-desk-main-app";

pub fn run_app() -> Result<()> {
    common::logger::init();
    match do_run_app() {
        Ok(_) => Ok(()),
        Err(e) => {
            log::error!("{:#}", e);
            Err(e)
        }
    }
}

fn do_run_app() -> Result<()> {
    use win::main_window::MainWindow;
    use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};

    let Some(mut single_instanceinstance) = single_instance::SingleInstance::acquire(MAIN_APP_INSTANCE_NAME)? else {
        log::info!("Another x-desk main app instance is already running");
        return Ok(());
    };
    drop(single_instanceinstance.take_message_receiver());

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let config_file_path = config::Config::config_file_path(common::APP_NAME)?;
    let config = config::Config::load_from_file(&config_file_path)?;
    let mut main_window = MainWindow::create(common::APP_NAME, config, config_file_path)?;
    let hwnd = main_window.hwnd();
    main_window.component_mut().run(hwnd)
}
