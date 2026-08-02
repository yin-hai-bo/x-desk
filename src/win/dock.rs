use std::{
    ops::{Deref, DerefMut},
    sync::Mutex,
};

use anyhow::Result;
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DefWindowProcW, RegisterClassExW, WM_NCCREATE, WM_NCDESTROY, WM_SIZE, WNDCLASSEXW, WS_CLIPCHILDREN,
            WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_POPUP,
        },
    },
    core::{PCWSTR, w},
};

use crate::win::{video_host::VideoHost, wide_string::WideString, win_utils, window::Window};

const DOCK_CLASS_NAME: PCWSTR = w!("X-Desk-Dock-Class");
static DOCK_CLASS_REGISTERED: Mutex<bool> = Mutex::new(false);

/// 挂接在桌面的窗口，在这个窗口里可进行图片显示、视频渲染等，从而显示特殊的 Wallpaper
pub(super) struct Dock {
    name: String,
    video_host: Option<Box<Window<VideoHost>>>,
}

impl Dock {
    pub fn new(name: String) -> Self {
        Self { name, video_host: None }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_video_source(&mut self, hwnd: HWND, source: &str) -> Result<()> {
        match self.video_host.as_mut() {
            Some(video_host) => {
                let video_host_hwnd = video_host.hwnd();
                video_host.component_mut().set_source(video_host_hwnd, source)
            }
            None => {
                self.video_host = Some(VideoHost::create(hwnd, source)?);
                Ok(())
            }
        }
    }

    fn resize_video_host(&self, hwnd: HWND) {
        if let Some(video_host) = &self.video_host {
            if let Err(e) = video_host.resize_to_parent(video_host.hwnd(), hwnd) {
                log::error!("Resize video host failed: {}", e);
            }
        }
    }

    pub fn create(name: String, rect: &RECT) -> Result<Box<Window<Dock>>> {
        let inst = unsafe { GetModuleHandleW(PCWSTR::null()) }?.into();
        Self::register_class(inst)?;
        Window::create(
            WS_EX_NOACTIVATE, // Do not use WS_EX_LAYERED，it's used only when "Raised Desktop"
            DOCK_CLASS_NAME,
            WideString::new(&name).as_pcwstr(),
            // Do not set WS_CHILD for now, because if WS_EX_LAYERED needs to be set later, the window muse be a top-level window
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            rect.left,
            rect.top,
            win_utils::width_of_rect(rect),
            win_utils::height_of_rect(rect),
            None,
            None,
            Some(inst),
            Dock::new(name),
        )
    }

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
            hbrBackground: unsafe { HBRUSH(GetStockObject(BLACK_BRUSH).0) },
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
            WM_SIZE => {
                if let Some(mut ptr) = Window::<Dock>::get_self_from_hwnd(hwnd) {
                    let window = unsafe { ptr.as_mut() };
                    window.component().resize_video_host(hwnd);
                }
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
