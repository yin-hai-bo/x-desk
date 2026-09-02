use std::sync::Mutex;

use anyhow::Result;
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DefWindowProcW, RegisterClassExW, WM_NCCREATE, WM_NCDESTROY, WNDCLASSEXW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
            WS_EX_NOACTIVATE, WS_POPUP,
        },
    },
    core::{PCWSTR, w},
};
use wnd::{Window, wide_string::WideString, win_utils};

use config::WallpaperContentSpec;

use crate::win::content_process::{self, ContentCommand, ContentProcessHandle};

const DOCK_CLASS_NAME: PCWSTR = w!("X-Desk-Dock-Class");
static DOCK_CLASS_REGISTERED: Mutex<bool> = Mutex::new(false);

/// 挂接在桌面的窗口，在这个窗口里可进行图片显示、视频渲染等，从而显示特殊的 Wallpaper
pub(super) struct Dock {
    name: String,
    content_process: Option<ContentProcessHandle>,
    content_spec: Option<WallpaperContentSpec>,
    occluded: bool,
}

impl Dock {
    pub fn new(name: String) -> Self {
        Self {
            name,
            content_process: None,
            content_spec: None,
            occluded: false,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn content_hwnd(&self) -> Option<HWND> {
        self.content_process.as_ref().map(ContentProcessHandle::hwnd)
    }

    pub fn ensure_content_process(&mut self, content: &WallpaperContentSpec) -> Result<HWND> {
        if self.content_spec.as_ref() == Some(content) {
            if let Some(process) = self.content_process.as_mut() {
                if process.is_running()? {
                    return Ok(process.hwnd());
                }
                log::error!(
                    "Content process exited unexpectedly, pid={}, hwnd={:?}",
                    process.process_id(),
                    process.hwnd()
                );
            }
        }

        self.content_process = None;
        let mut process = content_process::start_content_process(content)?;
        if self.occluded {
            process.send_command(ContentCommand::Pause)?;
        }
        let hwnd = process.hwnd();
        log::info!(
            "Started content process, pid={}, hwnd={:?}, content={:?}, source={}",
            process.process_id(),
            hwnd,
            content.kind,
            content.source
        );
        self.content_process = Some(process);
        self.content_spec = Some(content.clone());
        Ok(hwnd)
    }

    pub fn set_occluded(&mut self, occluded: bool) -> Result<()> {
        if self.occluded == occluded {
            return Ok(());
        }
        self.occluded = occluded;
        if let Some(process) = self.content_process.as_mut() {
            if occluded {
                #[cfg(debug_assertions)]
                log::debug!(
                    "Send Pause to content process, pid={}, hwnd={:?}",
                    process.process_id(),
                    process.hwnd()
                );
                process.send_command(ContentCommand::Pause)
            } else {
                #[cfg(debug_assertions)]
                log::debug!(
                    "Send Resume to content process, pid={}, hwnd={:?}",
                    process.process_id(),
                    process.hwnd()
                );
                process.send_command(ContentCommand::Resume)
            }
        } else {
            Ok(())
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
            _ => {}
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}
