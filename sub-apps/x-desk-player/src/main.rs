#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod player;
mod video_host;

fn main() {
    if let Err(error) = player::run_player() {
        log::error!("{}", error);
        std::process::exit(1);
    }
}
