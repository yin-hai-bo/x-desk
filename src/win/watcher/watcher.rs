use windows::{
    Win32::{
        Foundation::{HWND, WPARAM},
        System::RemoteDesktop::{
            NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
        },
        UI::WindowsAndMessaging::{RegisterWindowMessageW, WM_DISPLAYCHANGE, WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK},
    },
    core::w,
};

use crate::win::msg_id;

use super::window_destroy_watcher::WindowDestroyWatcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WatchEvent {
    DisplayChanded,
    WorkerWDestroied,
    TaskbarCreated,
    SessionUnlock,
}

/// 监视一些 OS 的事件，比如任务栏重建、WorkerW 销毁等……
pub(crate) struct Watcher {
    owner: HWND,
    taskbar_created_message: u32,
    session_notifications_registered: bool,
    _workerw_watcher: Option<WindowDestroyWatcher>,
}

impl Watcher {
    /// 创建监控
    ///
    /// # Parameters
    /// - owner 当事件发生时，向哪个窗口投递消息
    /// - worker_w WorkerW 窗口，要监控它，当它被销毁时，向 Owner 发消息
    pub fn new(owner: HWND, worker_w: Option<HWND>, msg_for_workerw_destroy: u32) -> Self {
        // 注册 Shell 的 “任务栏已创建” 消息
        let taskbar_created_message = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };

        // 注册会话通知
        let session_notifications_registered =
            unsafe { WTSRegisterSessionNotification(owner, NOTIFY_FOR_THIS_SESSION).is_ok() };

        Self {
            owner,
            taskbar_created_message,
            session_notifications_registered,
            _workerw_watcher: if let Some(target) = worker_w {
                Some(WindowDestroyWatcher::new(owner, target, msg_for_workerw_destroy))
            } else {
                None
            },
        }
    }

    pub fn handle_window_message(&self, message: u32, wparam: WPARAM) -> Option<WatchEvent> {
        classify_window_message(message, wparam, self.taskbar_created_message)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        if self.session_notifications_registered {
            unsafe {
                let _ = WTSUnRegisterSessionNotification(self.owner);
            }
        }
    }
}

fn classify_window_message(message: u32, wparam: WPARAM, taskbar_created_message: u32) -> Option<WatchEvent> {
    if message == WM_DISPLAYCHANGE {
        return Some(WatchEvent::DisplayChanded);
    }
    if taskbar_created_message != 0 && message == taskbar_created_message {
        return Some(WatchEvent::TaskbarCreated);
    }
    if message == WM_WTSSESSION_CHANGE && wparam.0 as u32 == WTS_SESSION_UNLOCK {
        return Some(WatchEvent::SessionUnlock);
    }
    if message == msg_id::WORKER_W_DESTROY_MESSAGE {
        return Some(WatchEvent::WorkerWDestroied);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_change_maps_to_refresh() {
        assert_eq!(
            classify_window_message(WM_DISPLAYCHANGE, WPARAM(0), 0),
            Some(WatchEvent::DisplayChanded)
        );
    }

    #[test]
    fn taskbar_created_maps_to_reset() {
        assert_eq!(
            classify_window_message(0xC123, WPARAM(0), 0xC123),
            Some(WatchEvent::TaskbarCreated)
        );
    }

    #[test]
    fn session_unlock_maps_to_session_unlock() {
        assert_eq!(
            classify_window_message(WM_WTSSESSION_CHANGE, WPARAM(WTS_SESSION_UNLOCK as usize), 0),
            Some(WatchEvent::SessionUnlock)
        );
    }

    #[test]
    fn wallpaper_reset_message_maps_to_reset() {
        assert_eq!(
            classify_window_message(msg_id::WORKER_W_DESTROY_MESSAGE, WPARAM(0), 0),
            Some(WatchEvent::WorkerWDestroied)
        );
    }
}
