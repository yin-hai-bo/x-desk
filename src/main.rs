#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = x_desk::run_app() {
        log::error!("{}", error);
        std::process::exit(1);
    }
}
