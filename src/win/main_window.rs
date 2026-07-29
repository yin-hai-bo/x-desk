use std::{
    ffi::c_void,
    path::{Path, PathBuf},
};

use anyhow::{Context, anyhow};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{DT_RIGHT, HBRUSH, HDC, UpdateWindow},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::ShellExecuteW,
            WindowsAndMessaging::{
                CS_HREDRAW, CS_VREDRAW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, GetSystemMetrics,
                IDC_ARROW, LoadCursorW, LoadIconW, MSG, PostQuitMessage, RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN,
                SW_SHOW, SW_SHOWNORMAL, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY, WM_ERASEBKGND,
                WM_NCCREATE, WM_NCDESTROY, WM_SETTINGCHANGE, WNDCLASSEXW, WS_CAPTION, WS_MINIMIZEBOX, WS_OVERLAPPED,
                WS_SYSMENU,
            },
        },
    },
    core::{PCWSTR, w},
};

use crate::{
    config::Config,
    win::{
        const_define,
        hyperlink_text::{HyperLinkFont, HyperLinkText},
        resource_ids::IDI_APP_ICON,
        theme,
        tray_icon::TrayIcon,
        wide_string::WideString,
        window::Window,
    },
};

const CLASS_NAME: PCWSTR = w!("YHB-XDeskMainWindow");

#[derive(Default)]
pub struct MainWindow {
    app_name: String,
    _config: Config,
    config_file_path: PathBuf,
    tray_icon: Option<TrayIcon>,
    config_dir_hyper_link: Option<Box<Window<HyperLinkText>>>,
}

impl MainWindow {
    pub fn create(app_name: &str, config: Config, config_file_path: PathBuf) -> anyhow::Result<Box<Window<Self>>> {
        let instance = unsafe { GetModuleHandleW(PCWSTR::null())?.into() };
        Self::register_class(instance)?;

        let component = Self {
            app_name: app_name.to_string(),
            _config: config,
            config_file_path,
            ..Default::default()
        };
        let window = Self::create_window(instance, WideString::new(app_name).as_pcwstr(), component)?;
        Ok(window)
    }

    pub fn run(&mut self, hwnd: HWND) -> anyhow::Result<()> {
        theme::apply_system_theme(hwnd);
        self.config_dir_hyper_link = self.create_config_dir_hyper_link(hwnd).ok();
        self.tray_icon = Some(TrayIcon::new(&self.app_name, hwnd, const_define::TRAY_ICON_MESSAGE)?);

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);
        }

        let mut message = MSG::default();
        loop {
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
            match result.0 {
                0 => {
                    return Ok(());
                }
                -1 => {
                    return Err(anyhow!(windows::core::Error::from_thread()));
                }
                _ => unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                },
            }
        }
    }

    fn register_class(instance: HINSTANCE) -> anyhow::Result<()> {
        unsafe {
            let icon = LoadIconW(Some(instance.into()), PCWSTR(IDI_APP_ICON as usize as *const u16))?;
            let window_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance.into(),
                hIcon: icon,
                hIconSm: icon,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: CLASS_NAME,
            };
            match RegisterClassExW(&window_class) {
                0 => Err(anyhow!(windows::core::Error::from_thread()).context("RegisterClass() failed")),
                _ => Ok(()),
            }
        }
    }

    fn create_window(instance: HINSTANCE, title: PCWSTR, component: Self) -> anyhow::Result<Box<Window<Self>>> {
        let window_width = 480;
        let window_height = 320;
        let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let window_x = (screen_width - window_width) / 2;
        let window_y = (screen_height - window_height) / 2;
        Window::create(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            title,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            window_x,
            window_y,
            window_width,
            window_height,
            None, // HWND of parent
            None, // HMENU
            Some(instance.into()),
            component,
        )
        .context("CreateWindowEx() failed")
    }

    fn create_config_dir_hyper_link(&self, hwnd: HWND) -> anyhow::Result<Box<Window<HyperLinkText>>> {
        let dir = self
            .config_file_path
            .parent()
            .ok_or(anyhow!("Invalid config file directory"))?
            .to_path_buf();
        let mut client_rect = RECT::default();
        unsafe { GetClientRect(hwnd, &mut client_rect)? };
        HyperLinkText::create(
            hwnd,
            "Open configuration directory",
            RECT {
                left: 24,
                top: client_rect.bottom - 16 - 28,
                right: client_rect.right - 24,
                bottom: client_rect.bottom - 16,
            },
            HyperLinkFont::new("Segoe UI", 12),
            DT_RIGHT,
            Some(move || Self::open_dir(&dir)),
        )
    }

    fn open_dir(dir: &Path) {
        let path = WideString::from_os_string(dir.as_os_str());
        unsafe {
            ShellExecuteW(None, w!("open"), path.as_pcwstr(), None, None, SW_SHOWNORMAL);
        }
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match message {
            WM_NCCREATE => Window::<Self>::on_wm_nccreate(hwnd, lparam),
            WM_NCDESTROY => Window::<Self>::on_wm_ncdestroy(hwnd),
            const_define::TRAY_ICON_MESSAGE => {
                if let Some(p) = unsafe { Window::<Self>::get_self_from_hwnd(hwnd) } {
                    let window = unsafe { &*p };
                    if let Some(tray_icon) = &window.component().tray_icon {
                        tray_icon.handle_message(hwnd, lparam);
                    }
                }
                return LRESULT(0);
            }
            WM_ERASEBKGND => {
                unsafe { theme::paint_background(hwnd, HDC(wparam.0 as *mut c_void)) };
                return LRESULT(1);
            }
            WM_SETTINGCHANGE => {
                theme::system_theme_changed(hwnd);
                return LRESULT(0);
            }

            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                return LRESULT(0);
            }
            _ => {}
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

impl Window<MainWindow> {
    pub fn run(&mut self) -> anyhow::Result<()> {
        let hwnd = self.hwnd();
        self.component_mut().run(hwnd)
    }
}
