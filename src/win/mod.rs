mod content_process;
mod desktop;
mod desktop_attachment;
mod dock;
mod main_ui_process;
pub mod main_window;
mod menu;
mod monitor;
mod msg_id;
mod occlusion;
mod resource_ids;
mod tray_icon;
pub mod wallpaper_manager;
mod watcher;

use std::{sync::mpsc::Receiver, thread};

use single_instance::SingleInstanceMessage;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::PostMessageW,
};

pub(crate) fn start_single_instance_message_forwarder(receiver: Receiver<SingleInstanceMessage>, hwnd: HWND) {
    let hwnd_value = hwnd.0 as isize;
    thread::spawn(move || {
        for message in receiver {
            match message {
                SingleInstanceMessage::ConfigReloadRequested => {
                    let result = unsafe {
                        PostMessageW(
                            Some(HWND(hwnd_value as *mut _)),
                            msg_id::CONFIG_RELOAD_REQUESTED_MESSAGE,
                            WPARAM(0),
                            LPARAM(0),
                        )
                    };
                    if let Err(e) = result {
                        log::error!("Post config reload message failed: {:#}", e);
                    }
                }
                SingleInstanceMessage::SecondInstanceStarted => {
                    log::info!("Another x-desk main app instance was started");
                }
                SingleInstanceMessage::ExitRequested => {}
            }
        }
    });
}
