use super::{desktop::Desktop, desktop_attachment, monitor::MonitorManager};
use crate::win::{dock::Dock, occlusion::DockRegion};
use anyhow::{Context, Result, anyhow, bail};
use config::{Config, WallpaperContentSpec, WallpaperKind};
use windows::Win32::Foundation::{HWND, RECT};
use wnd::Window;

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
        F: Fn(usize) -> Option<WallpaperContentSpec>,
    {
        if self.desktop.is_none() {
            self.desktop = Some(Desktop::new().context("Desktop::new() failed")?);
        }
        let desktop = self.desktop.as_ref().ok_or(anyhow!("self.desktop is none"))?;
        let monitors = MonitorManager::refresh_monitors()?;
        self.dock_list.remove_from(monitors.len());
        for (index, monitor) in monitors.iter().enumerate() {
            if let Some(content) = f(index) {
                match self.dock_list.get_or_create(index, monitor.rect(), monitor.work_rect()) {
                    Ok(dock) => {
                        log::info!("Create dock success, index={}", index);
                        if let Err(e) = Self::set_wallpaper_content(desktop, dock, monitor.rect(), &content) {
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
        self.refresh_wallpapers(|index| config.content_for_monitor(index))
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

    pub(super) fn dock_regions(&self) -> Vec<DockRegion> {
        self.dock_list.regions()
    }

    pub(super) fn apply_dock_occlusions(&mut self, occlusions: &[(HWND, bool)]) {
        self.dock_list.apply_occlusions(occlusions);
    }

    fn set_wallpaper_content(
        desktop: &Desktop,
        dock: &mut Box<Window<Dock>>,
        rc: &RECT,
        content: &WallpaperContentSpec,
    ) -> Result<()> {
        let content_hwnd = match content.kind {
            WallpaperKind::Video => dock
                .component_mut()
                .ensure_content_process(content)
                .context("Set dock content process failed")?,
            WallpaperKind::WebView => dock
                .component_mut()
                .ensure_content_process(content)
                .context("Set dock content process failed")?,
        };
        let attached = desktop_attachment::attach_content_window(content_hwnd, desktop, rc)?;

        log::info!(
            "{}: pos=({},{}), size={}x{}, content: {:?}, source: {}",
            dock.name(),
            attached.x,
            attached.y,
            attached.width,
            attached.height,
            content.kind,
            Self::truncate_with_ellipsis(&content.source, 128)
        );

        Ok(())
    }

    fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
        if s.chars().count() <= max_len {
            s.to_string()
        } else {
            format!("{} ...", s.chars().take(max_len).collect::<String>())
        }
    }
}

/// 可以用索引快速访问的 [`Dock`] 列表
#[derive(Default)]
struct DockList {
    array: [Option<Box<Window<Dock>>>; 16],
    occlusion_rects: [Option<RECT>; 16],
}

impl DockList {
    /// 根据下标取得一个 [`Dock`]，若不存在则新建一个。
    pub fn get_or_create(
        &mut self,
        index: usize,
        rect: &RECT,
        occlusion_rect: &RECT,
    ) -> Result<&mut Box<Window<Dock>>> {
        self.check_index(index)?;
        self.occlusion_rects[index] = Some(*occlusion_rect);
        if self.array[index].is_none() {
            let dock = Dock::create(format!("x-desk-dock-{}", index), rect)?;
            self.array[index] = Some(dock);
        }
        Ok(self.array[index].as_mut().unwrap())
    }

    pub fn remove(&mut self, index: usize) -> Result<()> {
        self.check_index(index)?;
        self.array[index] = None;
        self.occlusion_rects[index] = None;
        Ok(())
    }

    pub fn remove_from(&mut self, start: usize) {
        for index in start..self.array.len() {
            self.array[index] = None;
            self.occlusion_rects[index] = None;
        }
    }

    pub fn clear(&mut self) {
        self.remove_from(0);
    }

    pub fn regions(&self) -> Vec<DockRegion> {
        self.array
            .iter()
            .zip(self.occlusion_rects.iter())
            .filter_map(|(dock, occlusion_rect)| {
                let dock = dock.as_ref()?;
                let rect = *occlusion_rect.as_ref()?;
                Some(DockRegion {
                    hwnd: dock.component().content_hwnd()?,
                    rect,
                })
            })
            .collect()
    }

    pub fn apply_occlusions(&mut self, occlusions: &[(HWND, bool)]) {
        for (hwnd, occluded) in occlusions {
            if let Some(dock) = self
                .array
                .iter_mut()
                .flatten()
                .find(|dock| dock.component().content_hwnd() == Some(*hwnd))
            {
                if let Err(e) = dock.component_mut().set_occluded(*occluded) {
                    log::error!("Set dock occlusion failed: {}", e);
                }
            }
        }
    }

    fn check_index(&self, index: usize) -> Result<()> {
        if index >= self.array.len() {
            bail!("Too large index: {}", index);
        }
        Ok(())
    }
}
