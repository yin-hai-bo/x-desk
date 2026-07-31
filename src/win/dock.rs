use std::{
    ops::{Deref, DerefMut},
    sync::Mutex,
};

use anyhow::Result;
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DefWindowProcW, RegisterClassExW, WM_ERASEBKGND, WM_NCCREATE, WM_NCDESTROY, WNDCLASSEXW, WS_CHILD,
            WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_VISIBLE,
        },
    },
    core::{PCWSTR, w},
};

use crate::win::{wide_string::WideString, window::Window};

const DOCK_CLASS_NAME: PCWSTR = w!("X-Desk-Dock-Class");
static DOCK_CLASS_REGISTERED: Mutex<bool> = Mutex::new(false);

/// 挂接在桌面的窗口，在这个窗口里可进行图片显示、视频渲染等，从而显示特殊的 Wallpaper
pub(super) struct Dock {}

impl Dock {
    /// 创建实例
    ///
    /// # Parameters
    /// - parent 父窗口的 HWND，一般是桌面的 workerW 等。
    pub fn create(name: &str, parent: HWND, x: i32, y: i32, width: i32, height: i32) -> Result<Box<Window<Dock>>> {
        let inst = unsafe { GetModuleHandleW(PCWSTR::null()) }?.into();
        Self::register_class(inst)?;
        Window::create(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            DOCK_CLASS_NAME,
            WideString::new(name).as_pcwstr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            x,
            y,
            width,
            height,
            Some(parent),
            None,
            Some(inst),
            Dock {},
        )
    }

    pub fn set_wallpaper(&mut self, _video_url: &str) {}

    fn register_class(inst: HINSTANCE) -> Result<()> {
        let mut registered = DOCK_CLASS_REGISTERED.lock().unwrap();
        if *registered {
            return Ok(());
        }

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(Self::window_proc),
            hInstance: inst,
            lpszClassName: DOCK_CLASS_NAME,
            ..Default::default()
        };
        let atom = unsafe { RegisterClassExW(&wc) };
        if atom == 0 {
            return Err(windows::core::Error::from_thread().into());
        }
        *registered = true;
        Ok(())
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_NCCREATE => Window::<Dock>::on_wm_nccreate(hwnd, lparam),
            WM_NCDESTROY => Window::<Dock>::on_wm_ncdestroy(hwnd),
            WM_ERASEBKGND => {
                return LRESULT(1);
            }
            _ => {}
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

impl Deref for Window<Dock> {
    type Target = Dock;

    fn deref(&self) -> &Self::Target {
        self.component()
    }
}

impl DerefMut for Window<Dock> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.component_mut()
    }
}
