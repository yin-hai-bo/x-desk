use super::{desktop::Desktop, monitor::MonitorManager};
use crate::{
    config::Config,
    win::{
        dock::Dock,
        win_utils::{self, height_of_rect, width_of_rect},
        window::Window,
    },
};
use anyhow::{Context, Result, anyhow, bail};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::MapWindowPoints,
    UI::WindowsAndMessaging::{
        HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, WS_CAPTION, WS_CHILD,
        WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP,
        WS_SYSMENU, WS_THICKFRAME,
    },
};

pub struct WallpaperManager {
    desktop: Option<Desktop>,
    dock_list: DockList,
}

impl WallpaperManager {
    pub fn new() -> Self {
        Self {
            desktop: None,
            dock_list: DockList::default(),
        }
    }

    fn refresh_wallpapers<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(usize) -> Option<String>,
    {
        if self.desktop.is_none() {
            self.desktop = Some(Desktop::new().context("Desktop::new() failed")?);
        }
        let desktop = self.desktop.as_ref().ok_or(anyhow!("self.desktop is none"))?;
        let monitors = MonitorManager::refresh_monitors()?;
        self.dock_list.remove_from(monitors.len());
        for (index, monitor) in monitors.iter().enumerate() {
            if let Some(video_url) = f(index) {
                match self.dock_list.get_or_create(index, monitor.rect()) {
                    Ok(dock) => {
                        log::info!("Create dock success, index={}", index);
                        if let Err(e) = Self::set_wallpaper(desktop, dock, monitor.rect(), &video_url) {
                            log::error!("Error when set wallpaper: {}", e);
                        }
                    }
                    Err(e) => log::error!("Create dock failed, index={}: {}", index, e),
                }
            } else {
                let _ = self.dock_list.remove(index);
            }
        }
        Ok(())
    }

    pub fn refresh_wallpapers_from_config(&mut self, config: &Config) -> Result<()> {
        self.refresh_wallpapers(|index| config.video_url_for_monitor(index).map(str::to_string))
    }

    pub fn reset_wallpapers_from_config(&mut self, config: &Config) -> Result<()> {
        self.dock_list.clear();
        self.desktop = None;
        self.refresh_wallpapers_from_config(config)
    }

    pub(super) fn desktop_worker_w(&self) -> Option<HWND> {
        self.desktop.as_ref().map(Desktop::worker_w)
    }

    pub(super) fn is_desktop_valid(&self) -> bool {
        self.desktop
            .as_ref()
            .map(Desktop::is_wallpaper_parent_valid)
            .unwrap_or(false)
    }

    fn set_wallpaper(desktop: &Desktop, dock: &mut Box<Window<Dock>>, rc: &RECT, video_url: &str) -> Result<()> {
        let dock_hwnd = dock.hwnd();
        win_utils::set_window_pos(
            dock_hwnd,
            Some(HWND_BOTTOM),
            rc.left,
            rc.top,
            win_utils::width_of_rect(rc),
            win_utils::height_of_rect(rc),
            SWP_NOACTIVATE,
        )?;

        let mut pt = [POINT::default()];
        unsafe {
            let _ = MapWindowPoints(Some(dock_hwnd), Some(desktop.parent_of_wallpaper()), &mut pt);
        }

        if desktop.is_raised_desktop() {
            win_utils::set_window_transparency(dock_hwnd, 255)?;
            Self::set_to_child_window(dock_hwnd, desktop.parent_of_wallpaper())?;
            win_utils::set_window_pos(
                dock_hwnd,
                Some(desktop.shell_dll_def_view()),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )?;
            desktop.ensure_children_zorder()?;
        } else {
            Self::set_to_child_window(dock_hwnd, desktop.parent_of_wallpaper())?;
        }

        win_utils::set_window_pos(
            dock_hwnd,
            None,
            pt[0].x,
            pt[0].y,
            width_of_rect(rc),
            height_of_rect(rc),
            SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_NOZORDER,
        )?;
        dock.component_mut()
            .set_video_source(dock_hwnd, video_url)
            .context("Set dock video source failed")?;

        log::info!(
            "{}: pos=({},{}), size={}x{}, video: {}",
            dock.name(),
            pt[0].x,
            pt[0].y,
            width_of_rect(rc),
            height_of_rect(rc),
            video_url
        );

        Ok(())
    }

    fn set_to_child_window(hwnd: HWND, parent: HWND) -> Result<()> {
        let mut style = win_utils::get_window_style(hwnd)?;
        style &= !(WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_POPUP);
        style |= WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS;
        win_utils::set_window_style(hwnd, style)?;

        let mut ex_style = win_utils::get_window_ex_style(hwnd)?;
        ex_style |= WS_EX_TOOLWINDOW;
        ex_style &= !WS_EX_APPWINDOW;
        win_utils::set_window_ex_style(hwnd, ex_style)?;

        win_utils::set_parent(hwnd, Some(parent)).map(|_| ())
    }
}

/// 可以用索引快速访问的 [`Dock`] 列表
#[derive(Default)]
struct DockList {
    array: [Option<Box<Window<Dock>>>; 16],
}

impl DockList {
    /// 根据下标取得一个 [`Dock`]，若不存在则新建一个。
    pub fn get_or_create(&mut self, index: usize, rect: &RECT) -> Result<&mut Box<Window<Dock>>> {
        self.check_index(index)?;
        if self.array[index].is_none() {
            let dock = Dock::create(format!("x-desk-dock-{}", index), rect)?;
            self.array[index] = Some(dock);
        }
        Ok(self.array[index].as_mut().unwrap())
    }

    pub fn remove(&mut self, index: usize) -> Result<()> {
        self.check_index(index)?;
        self.array[index] = None;
        Ok(())
    }

    pub fn remove_from(&mut self, start: usize) {
        for dock in self.array.iter_mut().skip(start) {
            *dock = None;
        }
    }

    pub fn clear(&mut self) {
        self.remove_from(0);
    }

    fn check_index(&self, index: usize) -> Result<()> {
        if index >= self.array.len() {
            bail!("Too large index: {}", index);
        }
        Ok(())
    }
}
