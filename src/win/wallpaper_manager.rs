use super::{desktop::Desktop, monitor::MonitorManager};
use crate::{
    beacon::Beacon,
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

pub struct WallpaperManager<'a> {
    _beacon: &'a Beacon,
    desktop: Option<Desktop>,
    dock_list: DockList,
}

impl<'a> WallpaperManager<'a> {
    pub fn new(_beacon: &'a Beacon) -> Self {
        Self {
            _beacon,
            desktop: None,
            dock_list: DockList::default(),
        }
    }

    pub fn refresh_wallpapers<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(usize) -> Option<String>,
    {
        if self.desktop.is_none() {
            self.desktop = Some(Desktop::new().context("Cannot get desktop information")?);
        }
        let desktop = self.desktop.as_ref().ok_or(anyhow!("self.desktop is none"))?;
        let monitors = MonitorManager::refresh_monitors()?;
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

    fn set_wallpaper(desktop: &Desktop, dock: &mut Box<Window<Dock>>, rc: &RECT, _video_url: &str) -> Result<()> {
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
        log::info!(
            "{}: pos=({},{}), size={}x{}",
            dock.name(),
            pt[0].x,
            pt[0].y,
            width_of_rect(rc),
            height_of_rect(rc)
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

    fn check_index(&self, index: usize) -> Result<()> {
        if index >= self.array.len() {
            bail!("Too large index: {}", index);
        }
        Ok(())
    }
}
