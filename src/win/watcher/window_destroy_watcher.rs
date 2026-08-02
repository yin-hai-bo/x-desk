use std::sync::{Mutex, OnceLock};

use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        WindowsAndMessaging::{EVENT_OBJECT_DESTROY, GetWindowThreadProcessId, PostMessageW, WINEVENT_OUTOFCONTEXT},
    },
};

/// 监视给定窗口的“被销毁”事件。
/// 当给定的窗口被销毁时得到通知，并向指定的 Owner 窗口发消息
pub(super) struct WindowDestroyWatcher {
    hook: Option<HWINEVENTHOOK>,
}

impl WindowDestroyWatcher {
    /// 构造.
    ///
    /// # Parameters
    /// - owner 当监视到 target 被销毁时，应该向哪个窗口发消息
    /// - target: 被监视的窗口
    /// - msg 向 owner 发什么消息（PostMessage）
    pub fn new(owner: HWND, target: HWND, msg: u32) -> WindowDestroyWatcher {
        let hook = Self::hook_destroy(owner, target, msg);
        Self { hook }
    }

    fn hook_destroy(owner: HWND, target: HWND, msg: u32) -> Option<HWINEVENTHOOK> {
        if target.is_invalid() {
            return None;
        }

        let mut process_id = 0;
        let thread_id = unsafe { GetWindowThreadProcessId(target, Some(&mut process_id)) };
        if thread_id == 0 {
            return None;
        }

        let hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_DESTROY,
                None,
                Some(Self::destroy_proc),
                process_id,
                thread_id,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_invalid() {
            None
        } else {
            let context = Context {
                hook: hook.0 as isize,
                owner: owner.0 as isize,
                target: target.0 as isize,
                msg,
            };
            hook_contexts().lock().unwrap_or_else(|e| e.into_inner()).push(context);
            Some(hook)
        }
    }

    unsafe extern "system" fn destroy_proc(
        hook: HWINEVENTHOOK,
        event: u32,
        hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _id_event_thread: u32,
        _event_time: u32,
    ) {
        if event != EVENT_OBJECT_DESTROY {
            return;
        }
        if let Some(context) = hook_contexts()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|x| x.hook == hook.0 as isize && x.target == hwnd.0 as isize)
        {
            unsafe {
                let _ = PostMessageW(Some(HWND(context.owner as *mut _)), context.msg, WPARAM(0), LPARAM(0));
            }
        }
    }
}

impl Drop for WindowDestroyWatcher {
    fn drop(&mut self) {
        if let Some(hook) = self.hook.take() {
            unsafe {
                let _ = UnhookWinEvent(hook);
            }
            hook_contexts()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|x| x.hook != hook.0 as isize);
        }
    }
}

#[derive(Clone, Copy)]
struct Context {
    hook: isize,
    owner: isize,
    target: isize,
    msg: u32,
}

fn hook_contexts() -> &'static Mutex<Vec<Context>> {
    static CONTEXTS: OnceLock<Mutex<Vec<Context>>> = OnceLock::new();
    CONTEXTS.get_or_init(|| Mutex::new(Vec::<Context>::with_capacity(2)))
}
