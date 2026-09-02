#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = webview::run_webview() {
        log::error!("{}", error);
        std::process::exit(1);
    }
}

mod webview;
