use std::path::PathBuf;

use anyhow::{Context, anyhow};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DefWindowProcW, DispatchMessageW, GetMessageW, KillTimer, MSG, PostQuitMessage, RegisterClassExW, SetTimer,
            TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WM_NCCREATE, WM_NCDESTROY, WM_TIMER,
            WNDCLASSEXW,
        },
    },
    core::{PCWSTR, w},
};

use config::Config;

use crate::win::{
    main_ui_process, msg_id,
    occlusion::{self, OcclusionWatcher},
    tray_icon::{TrayCommand, TrayIcon},
    wallpaper_manager::WallpaperManager,
    watcher::{WatchEvent, Watcher},
    window::Window,
};

const CLASS_NAME: PCWSTR = w!("YHB-XDeskMainWindow");
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
        };
        let window = Self::create_window(instance, component)?;
        Ok(window)
    }

    pub fn run(&mut self, hwnd: HWND) -> anyhow::Result<()> {
        self.recreate_tray_icon(hwnd)?;
        self.refresh_wallpapers();
        self.recreate_watcher(hwnd);
        self.recreate_occlusion_watcher(hwnd);
        self.refresh_occlusions(hwnd);

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
            let window_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: Default::default(),
                lpfnWndProc: Some(Self::window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance.into(),
                hIcon: Default::default(),
                hIconSm: Default::default(),
                hCursor: Default::default(),
                hbrBackground: Default::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: CLASS_NAME,
            };
            match RegisterClassExW(&window_class) {
                0 => Err(anyhow!(windows::core::Error::from_thread()).context("RegisterClass() failed")),
                _ => Ok(()),
            }
        }
    }

    fn create_window(instance: HINSTANCE, component: Self) -> anyhow::Result<Box<Window<Self>>> {
        Window::create(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            PCWSTR::null(),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            component,
        )
        .context("CreateWindowEx() failed")
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

    fn handle_tray_command(&self, command: TrayCommand) {
        match command {
            TrayCommand::ShowMainUi => {
                if let Err(e) = main_ui_process::launch_main_ui_process(&self.config_file_path) {
                    log::error!("Launch main UI process failed: {:#}", e);
                }
            }
        }
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
                        if let Some(command) = tray_icon.handle_message(hwnd, lparam) {
                            window.component().handle_tray_command(command);
                        }
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
