#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
mod win;

fn main() {
    #[cfg(target_os = "windows")]
    {
        if let Err(error) = win::main_window::run() {
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
