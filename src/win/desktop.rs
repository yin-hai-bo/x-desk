use crate::win::win_utils;
use anyhow::{Context, Result};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, TRUE, WPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, FindWindowExW, FindWindowW, GetDesktopWindow, SMTO_NORMAL, SendMessageTimeoutW,
            WS_EX_NOREDIRECTIONBITMAP,
        },
    },
    core::{BOOL, PCWSTR, w},
};

const PROGMAN_NAME: PCWSTR = w!("Progman");
const WORKER_W_NAME: PCWSTR = w!("WorkerW");
const SHELL_DLL_DEF_VIEW_NAME: PCWSTR = w!("SHELLDLL_DefView");

#[derive(Default)]
struct WorkerWInfo {
    worker_w: HWND,
    shell_dll_def_view: HWND,
}

impl WorkerWInfo {
    pub fn find() -> Result<WorkerWInfo> {
        let mut info = WorkerWInfo::default();
        unsafe {
            EnumWindows(
                Some(Self::enum_windows_proc),
                LPARAM(&mut info as *mut WorkerWInfo as isize),
            )
        }?;
        Ok(info)
    }

    unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if let Ok(def_view) = unsafe { FindWindowExW(Some(hwnd), None, SHELL_DLL_DEF_VIEW_NAME, None) } {
            let info = unsafe { &mut *(lparam.0 as *mut WorkerWInfo) };
            info.worker_w = unsafe { FindWindowExW(None, Some(hwnd), WORKER_W_NAME, None) }.unwrap_or_default();
            info.shell_dll_def_view = def_view;
        }
        TRUE
    }

    pub fn get_desktop_worker_w(progman: HWND) -> HWND {
        unsafe {
            if FindWindowExW(Some(progman), None, SHELL_DLL_DEF_VIEW_NAME, None).is_ok() {
                return progman;
            }
            let desktop = GetDesktopWindow();
            let mut origin: HWND = HWND::default();
            loop {
                let after = if origin.is_invalid() { None } else { Some(origin) };
                if let Ok(o) = FindWindowExW(Some(desktop), after, WORKER_W_NAME, None) {
                    origin = o;
                    if FindWindowExW(Some(origin), None, SHELL_DLL_DEF_VIEW_NAME, None).is_ok() {
                        return origin;
                    }
                } else {
                    return progman;
                }
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Desktop {
    progman: HWND,
    is_raised_desktop: bool,
    original_worker_w: HWND,
    worker_w: HWND,
    shell_dll_def_view: HWND,
}

impl Desktop {
    pub fn new() -> Result<Self> {
        let progman = unsafe { FindWindowW(PROGMAN_NAME, PCWSTR::null()) }.context("Find 'Progman' failed")?;
        let is_raised_desktop = win_utils::has_hwnd_extended_style(progman, WS_EX_NOREDIRECTIONBITMAP);
        unsafe {
            let _ = SendMessageTimeoutW(progman, 0x052c, WPARAM(13), LPARAM(1), SMTO_NORMAL, 1000, None);
        }
        let mut info = WorkerWInfo::find().context("Find workerW failed")?;
        if is_raised_desktop {
            if let Ok(w) = unsafe { FindWindowExW(Some(progman), None, WORKER_W_NAME, None) } {
                info.worker_w = w;
            }
        }
        Ok(Self {
            progman,
            is_raised_desktop,
            original_worker_w: WorkerWInfo::get_desktop_worker_w(progman),
            worker_w: info.worker_w,
            shell_dll_def_view: info.shell_dll_def_view,
        })
    }

    pub fn parent_of_wallpaper(&self) -> HWND {
        if self.is_raised_desktop {
            self.progman
        } else {
            self.worker_w
        }
    }
}
