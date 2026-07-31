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
    Foundation::{HWND, POINT, RECT}, Graphics::Gdi::{InvalidateRect, MapWindowPoints, UpdateWindow}, UI::WindowsAndMessaging::{HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetParent},
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

        let parent_of_wallpaper = desktop.parent_of_wallpaper();
        let monitors = MonitorManager::refresh_monitors()?;
        for (index, monitor) in monitors.iter().enumerate() {
            if let Some(video_url) = f(index) {
                match self
                    .dock_list
                    .get_or_create(index, parent_of_wallpaper, monitor.rect())
                {
                    Ok(dock) => {
                        log::info!("Create dock success, index={}", index);
                        Self::set_wallpaper(desktop, dock, monitor.rect(), &video_url);
                    }
                    Err(e) => log::error!("Create dock failed, index={}: {}", index, e),
                }
            } else {
                let _ = self.dock_list.remove(index);
            }
        }
        Ok(())
    }

    fn set_wallpaper(desktop: &Desktop, dock: &mut Box<Window<Dock>>, rc: &RECT, _video_url: &str) {
        let _ = win_utils::set_window_pos(
            dock.hwnd(),
            Some(HWND_BOTTOM),
            rc.left,
            rc.top,
            win_utils::width_of_rect(rc),
            win_utils::height_of_rect(rc),
            SWP_NOACTIVATE,
        );

        let mut pt = [POINT::default()];
        unsafe {
            MapWindowPoints(Some(dock.hwnd()), Some(desktop.parent_of_wallpaper()), &mut pt);
        }

        if desktop.is_raised_desktop() {
            let _ = win_utils::set_window_transparency(dock.hwnd(), 255);
            let _ = win_utils::set_window_pos(
                dock.hwnd(),
                Some(desktop.shell_dll_def_view()),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            desktop.ensure_workerw_zorder();
        } else {
            unsafe {
                let _ = SetParent(dock.hwnd(), Some(desktop.worker_w()));
            }
        }

        let _ = win_utils::set_window_pos(
            dock.hwnd(),
            None,
            pt[0].x,
            pt[0].y,
            width_of_rect(rc),
            height_of_rect(rc),
            SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_NOZORDER,
        );
        log::info!("{}: pos=({},{}), size={}x{}", dock.name(), pt[0].x, pt[0].y, width_of_rect(rc), height_of_rect(rc));

        unsafe {
            let _ = InvalidateRect(Some(dock.hwnd()), None, true);
            let _ = UpdateWindow(dock.hwnd());
        }
    }
}

/// 可以用索引快速访问的 [`Dock`] 列表
#[derive(Default)]
struct DockList {
    array: [Option<Box<Window<Dock>>>; 16],
}

impl DockList {
    /// 根据下标取得一个 [`Dock`]，若不存在则新建一个。
    ///
    /// # Parameters
    /// - parent 如果要新建，则为 [`Dock`] 的 `parent` 参数
    /// - rect 如果要新建，则为 [`Dock`] 的大小尺寸
    pub fn get_or_create(&mut self, index: usize, parent: HWND, rect: &RECT) -> Result<&mut Box<Window<Dock>>> {
        self.check_index(index)?;
        if self.array[index].is_none() {
            let dock = Dock::create(format!("x-desk-dock-{}", index), parent, rect)?;
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
