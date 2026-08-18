#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = x_desk::run_player() {
        log::error!("{}", error);
        std::process::exit(1);
    }
}
