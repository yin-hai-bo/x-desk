use std::path::PathBuf;

use anyhow::{Context, anyhow};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{CreateSolidBrush, HDC, UpdateWindow},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            HiDpi::{GetDpiForSystem, GetDpiForWindow},
            WindowsAndMessaging::{
                CS_HREDRAW, CS_VREDRAW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, GetSystemMetrics,
                IDC_ARROW, KillTimer, LoadCursorW, LoadIconW, MSG, PostQuitMessage, RegisterClassExW, SM_CXSCREEN,
                SM_CYSCREEN, SW_HIDE, SW_SHOW, SetTimer, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_CLOSE,
                WM_DESTROY, WM_ERASEBKGND, WM_NCCREATE, WM_NCDESTROY, WM_SETTINGCHANGE, WM_TIMER, WNDCLASSEXW,
                WS_CAPTION, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
            },
        },
    },
    core::{PCWSTR, w},
};

use crate::{
    config::Config,
    win::{
        hyperlink_text::{Anchor, HorizontalAnchor, HyperLinkFont, HyperLinkText, VerticalAnchor},
        msg_id,
        occlusion::{self, OcclusionWatcher},
        resource_ids::IDI_APP_ICON,
        settings_process, theme,
        tray_icon::TrayIcon,
        wallpaper_manager::WallpaperManager,
        watcher::{WatchEvent, Watcher},
        wide_string::WideString,
        window::Window,
    },
};

const CLASS_NAME: PCWSTR = w!("YHB-XDeskMainWindow");
const DEFAULT_WINDOW_WIDTH: i32 = 480;
const DEFAULT_WINDOW_HEIGHT: i32 = 320;
const CONFIG_LINK_MARGIN_RIGHT: i32 = 24;
const CONFIG_LINK_MARGIN_BOTTOM: i32 = 16;
const DEFAULT_DPI: u32 = 96;
const WORKER_W_RESET_TIMER_ID: usize = 1;
const WORKER_W_RESET_DELAY_MS: u32 = 500;
const OCCLUSION_CHECK_TIMER_ID: usize = 2;
const OCCLUSION_CHECK_DELAY_MS: u32 = 100;

pub struct MainWindow {
    app_name: String,
    config: Config,
    config_file_path: PathBuf,
    wallpaper_manager: WallpaperManager,
    watcher: Option<Watcher>,
    occlusion_watcher: Option<OcclusionWatcher>,
    reset_scheduler: WallpaperResetScheduler,
    occlusion_check_pending: bool,
    tray_icon: Option<TrayIcon>,
    config_dir_hyper_link: Option<Box<Window<HyperLinkText>>>,
}

impl MainWindow {
    pub fn create(app_name: &str, config: Config, config_file_path: PathBuf) -> anyhow::Result<Box<Window<Self>>> {
        let instance = unsafe { GetModuleHandleW(PCWSTR::null())?.into() };
        Self::register_class(instance)?;

        let component = Self {
            app_name: app_name.to_string(),
            config,
            config_file_path,
            wallpaper_manager: WallpaperManager::new(),
            watcher: None,
            occlusion_watcher: None,
            reset_scheduler: WallpaperResetScheduler::default(),
            occlusion_check_pending: false,
            tray_icon: None,
            config_dir_hyper_link: None,
        };
        let window = Self::create_window(instance, WideString::new(app_name).as_pcwstr(), component)?;
        Ok(window)
    }

    pub fn run(&mut self, hwnd: HWND) -> anyhow::Result<()> {
        theme::apply_system_theme(hwnd);
        self.config_dir_hyper_link = self.create_config_dir_hyper_link(hwnd).ok();
        self.recreate_tray_icon(hwnd)?;
        self.refresh_wallpapers();
        self.recreate_watcher(hwnd);
        self.recreate_occlusion_watcher(hwnd);
        self.refresh_occlusions(hwnd);

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
                hbrBackground: CreateSolidBrush(theme::background_color()),
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
        let dpi = unsafe { GetDpiForSystem() };
        let window_width = Self::scale_for_dpi(DEFAULT_WINDOW_WIDTH, dpi);
        let window_height = Self::scale_for_dpi(DEFAULT_WINDOW_HEIGHT, dpi);
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

    fn scale_for_dpi(value: i32, dpi: u32) -> i32 {
        (value * dpi.max(DEFAULT_DPI) as i32 + (DEFAULT_DPI as i32 / 2)) / DEFAULT_DPI as i32
    }

    fn create_config_dir_hyper_link(&self, hwnd: HWND) -> anyhow::Result<Box<Window<HyperLinkText>>> {
        let config_file_path = self.config_file_path.clone();
        let mut client_rect = RECT::default();
        unsafe { GetClientRect(hwnd, &mut client_rect)? };
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let margin_right = Self::scale_for_dpi(CONFIG_LINK_MARGIN_RIGHT, dpi);
        let margin_bottom = Self::scale_for_dpi(CONFIG_LINK_MARGIN_BOTTOM, dpi);
        HyperLinkText::create(
            hwnd,
            "Open settings",
            Anchor::new(
                HorizontalAnchor::Right(client_rect.right - margin_right),
                VerticalAnchor::Bottom(client_rect.bottom - margin_bottom),
            ),
            HyperLinkFont::new("Segoe UI", 12),
            Some(move || {
                if let Err(e) = settings_process::launch_settings_process(&config_file_path) {
                    log::error!("Launch settings process failed: {:#}", e);
                }
            }),
        )
    }

    fn recreate_watcher(&mut self, hwnd: HWND) {
        self.watcher = Some(Watcher::new(
            hwnd,
            self.wallpaper_manager.desktop_worker_w(),
            msg_id::WORKER_W_DESTROY_MESSAGE,
        ));
    }

    fn recreate_occlusion_watcher(&mut self, hwnd: HWND) {
        self.occlusion_watcher = None;
        self.occlusion_watcher = Some(OcclusionWatcher::new(hwnd, msg_id::OCCLUSION_CHECK_MESSAGE));
    }

    fn recreate_tray_icon(&mut self, hwnd: HWND) -> anyhow::Result<()> {
        self.tray_icon = None;
        self.tray_icon = Some(TrayIcon::new(&self.app_name, hwnd, msg_id::TRAY_ICON_MESSAGE)?);
        Ok(())
    }

    fn refresh_wallpapers(&mut self) {
        if let Err(e) = self.wallpaper_manager.refresh_wallpapers_from_config(&self.config) {
            log::error!("Refresh wallpapers failed: {:#}", e);
        }
    }

    fn reset_wallpapers(&mut self, hwnd: HWND) {
        self.watcher = None;
        if let Err(e) = self.wallpaper_manager.reset_wallpapers_from_config(&self.config) {
            log::error!("Reset wallpapers failed: {:#}", e);
        }
        self.recreate_watcher(hwnd);
        self.refresh_occlusions(hwnd);
    }

    fn schedule_worker_w_reset(&mut self, hwnd: HWND) {
        if self.reset_scheduler.worker_w_destroyed() == ResetScheduleAction::StartWorkerWResetTimer {
            let timer_id = unsafe { SetTimer(Some(hwnd), WORKER_W_RESET_TIMER_ID, WORKER_W_RESET_DELAY_MS, None) };
            if timer_id == 0 {
                self.reset_scheduler.clear_worker_w_reset();
                log::error!("Schedule WorkerW reset timer failed");
                self.reset_wallpapers(hwnd);
            }
        }
    }

    fn cancel_worker_w_reset_timer(&mut self, hwnd: HWND) {
        unsafe {
            let _ = KillTimer(Some(hwnd), WORKER_W_RESET_TIMER_ID);
        }
    }

    fn handle_taskbar_created(&mut self, hwnd: HWND) {
        if self.reset_scheduler.taskbar_created() == ResetScheduleAction::CancelWorkerWResetTimerAndResetNow {
            self.cancel_worker_w_reset_timer(hwnd);
        }
        if let Err(e) = self.recreate_tray_icon(hwnd) {
            log::error!("Recreate tray icon failed: {:#}", e);
        }
        self.reset_wallpapers(hwnd);
    }

    fn handle_worker_w_reset_timer(&mut self, hwnd: HWND) {
        if self.reset_scheduler.worker_w_reset_timer_elapsed()
            == ResetScheduleAction::CancelWorkerWResetTimerAndResetNow
        {
            self.cancel_worker_w_reset_timer(hwnd);
            self.reset_wallpapers(hwnd);
        }
    }

    fn schedule_occlusion_check(&mut self, hwnd: HWND) {
        self.occlusion_check_pending = true;
        let timer_id = unsafe { SetTimer(Some(hwnd), OCCLUSION_CHECK_TIMER_ID, OCCLUSION_CHECK_DELAY_MS, None) };
        if timer_id == 0 {
            self.occlusion_check_pending = false;
            log::error!("Schedule occlusion check timer failed");
            self.refresh_occlusions(hwnd);
        }
    }

    fn cancel_occlusion_check_timer(&mut self, hwnd: HWND) {
        unsafe {
            let _ = KillTimer(Some(hwnd), OCCLUSION_CHECK_TIMER_ID);
        }
    }

    fn handle_occlusion_check_timer(&mut self, hwnd: HWND) {
        if !self.occlusion_check_pending {
            return;
        }
        self.occlusion_check_pending = false;
        self.cancel_occlusion_check_timer(hwnd);
        self.refresh_occlusions(hwnd);
    }

    fn refresh_occlusions(&mut self, hwnd: HWND) {
        let regions = self.wallpaper_manager.dock_regions();
        let occlusions = occlusion::collect_dock_occlusions(hwnd, &regions);
        self.wallpaper_manager.apply_dock_occlusions(&occlusions);
    }

    fn handle_watch_event(&mut self, hwnd: HWND, event: WatchEvent) {
        match event {
            WatchEvent::DisplayChanded => {
                self.refresh_wallpapers();
                self.refresh_occlusions(hwnd);
            }
            WatchEvent::WorkerWDestroyed => self.schedule_worker_w_reset(hwnd),
            WatchEvent::TaskbarCreated => self.handle_taskbar_created(hwnd),
            WatchEvent::SessionUnlock => {
                if self.wallpaper_manager.is_desktop_valid() {
                    self.refresh_wallpapers();
                    self.refresh_occlusions(hwnd);
                } else {
                    self.reset_wallpapers(hwnd);
                }
            }
        }
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if let Some(mut ptr) = Window::<Self>::get_self_from_hwnd(hwnd) {
            let window = unsafe { ptr.as_mut() };
            if let Some(watcher) = &window.component().watcher {
                if let Some(event) = watcher.handle_window_message(message, wparam) {
                    window.component_mut().handle_watch_event(hwnd, event);
                }
            }
        }

        match message {
            WM_NCCREATE => Window::<Self>::on_wm_nccreate(hwnd, lparam),
            WM_NCDESTROY => Window::<Self>::on_wm_ncdestroy(hwnd),
            msg_id::TRAY_ICON_MESSAGE => {
                if let Some(ptr) = Window::<Self>::get_self_from_hwnd(hwnd) {
                    let window = unsafe { ptr.as_ref() };
                    if let Some(tray_icon) = &window.component().tray_icon {
                        tray_icon.handle_message(hwnd, lparam);
                    }
                }
                return LRESULT(0);
            }
            msg_id::OCCLUSION_CHECK_MESSAGE => {
                if let Some(mut ptr) = Window::<Self>::get_self_from_hwnd(hwnd) {
                    unsafe { ptr.as_mut() }.component_mut().schedule_occlusion_check(hwnd);
                }
                return LRESULT(0);
            }
            WM_ERASEBKGND => {
                unsafe { theme::paint_background(hwnd, HDC(wparam.0 as *mut _)) };
                return LRESULT(1);
            }
            WM_SETTINGCHANGE => {
                theme::system_theme_changed(hwnd);
                return LRESULT(0);
            }
            WM_TIMER => {
                if wparam.0 == WORKER_W_RESET_TIMER_ID {
                    if let Some(mut ptr) = Window::<Self>::get_self_from_hwnd(hwnd) {
                        unsafe { ptr.as_mut() }
                            .component_mut()
                            .handle_worker_w_reset_timer(hwnd);
                    }
                    return LRESULT(0);
                }
                if wparam.0 == OCCLUSION_CHECK_TIMER_ID {
                    if let Some(mut ptr) = Window::<Self>::get_self_from_hwnd(hwnd) {
                        unsafe { ptr.as_mut() }
                            .component_mut()
                            .handle_occlusion_check_timer(hwnd);
                    }
                    return LRESULT(0);
                }
            }
            WM_CLOSE => {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                return LRESULT(0);
            }
            WM_DESTROY => {
                if let Some(mut ptr) = Window::<Self>::get_self_from_hwnd(hwnd) {
                    let window = unsafe { ptr.as_mut() }.component_mut();
                    window.watcher = None;
                    window.occlusion_watcher = None;
                }
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

#[derive(Debug, Default)]
struct WallpaperResetScheduler {
    worker_w_reset_pending: bool,
}

impl WallpaperResetScheduler {
    fn worker_w_destroyed(&mut self) -> ResetScheduleAction {
        if self.worker_w_reset_pending {
            ResetScheduleAction::None
        } else {
            self.worker_w_reset_pending = true;
            ResetScheduleAction::StartWorkerWResetTimer
        }
    }

    fn taskbar_created(&mut self) -> ResetScheduleAction {
        if self.worker_w_reset_pending {
            self.worker_w_reset_pending = false;
            ResetScheduleAction::CancelWorkerWResetTimerAndResetNow
        } else {
            ResetScheduleAction::ResetNow
        }
    }

    fn worker_w_reset_timer_elapsed(&mut self) -> ResetScheduleAction {
        if self.worker_w_reset_pending {
            self.worker_w_reset_pending = false;
            ResetScheduleAction::CancelWorkerWResetTimerAndResetNow
        } else {
            ResetScheduleAction::None
        }
    }

    fn clear_worker_w_reset(&mut self) {
        self.worker_w_reset_pending = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResetScheduleAction {
    None,
    StartWorkerWResetTimer,
    CancelWorkerWResetTimerAndResetNow,
    ResetNow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_w_destroy_starts_delayed_reset() {
        let mut scheduler = WallpaperResetScheduler::default();

        assert_eq!(
            scheduler.worker_w_destroyed(),
            ResetScheduleAction::StartWorkerWResetTimer
        );
    }

    #[test]
    fn taskbar_created_cancels_pending_worker_w_reset_and_resets_now() {
        let mut scheduler = WallpaperResetScheduler::default();
        scheduler.worker_w_destroyed();

        assert_eq!(
            scheduler.taskbar_created(),
            ResetScheduleAction::CancelWorkerWResetTimerAndResetNow
        );
        assert_eq!(scheduler.worker_w_reset_timer_elapsed(), ResetScheduleAction::None);
    }

    #[test]
    fn worker_w_reset_timer_elapsed_resets_once() {
        let mut scheduler = WallpaperResetScheduler::default();
        scheduler.worker_w_destroyed();

        assert_eq!(
            scheduler.worker_w_reset_timer_elapsed(),
            ResetScheduleAction::CancelWorkerWResetTimerAndResetNow
        );
        assert_eq!(scheduler.worker_w_reset_timer_elapsed(), ResetScheduleAction::None);
    }

    #[test]
    fn multiple_worker_w_destroy_events_keep_one_pending_reset() {
        let mut scheduler = WallpaperResetScheduler::default();

        assert_eq!(
            scheduler.worker_w_destroyed(),
            ResetScheduleAction::StartWorkerWResetTimer
        );
        assert_eq!(scheduler.worker_w_destroyed(), ResetScheduleAction::None);
    }
}
