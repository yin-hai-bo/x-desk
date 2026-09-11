mod win;

use anyhow::Result;

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

    let Some(mut single_instance_instance) = single_instance::SingleInstance::acquire(common::MAIN_APP_INSTANCE_NAME)?
    else {
        log::info!("Another x-desk main app instance is already running");
        return Ok(());
    };
    let single_instance_receiver = single_instance_instance.take_message_receiver();

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let config_file_path = config::Config::config_file_path(common::APP_NAME)?;
    let config = config::Config::load_from_file(&config_file_path)?;
    let mut main_window = MainWindow::create(common::APP_NAME, config, config_file_path)?;
    let hwnd = main_window.hwnd();
    if let Some(receiver) = single_instance_receiver {
        win::start_single_instance_message_forwarder(receiver, hwnd);
    }
    main_window.component_mut().run(hwnd)
}
