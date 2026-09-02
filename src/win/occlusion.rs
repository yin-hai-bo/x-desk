use std::{
    ffi::c_void,
    mem::size_of,
    sync::atomic::{AtomicIsize, Ordering},
};

use anyhow::{Context, Result};
use windows::{
    Win32::{
        Foundation::{FALSE, HWND, LPARAM, RECT, WPARAM},
        Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
            WindowsAndMessaging::{
                EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW,
                EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, EnumWindows,
                GetClassNameW, GetWindowRect, IsIconic, IsWindowVisible, PostMessageW, WINEVENT_OUTOFCONTEXT,
                WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
            },
        },
    },
    core::BOOL,
};

use wnd::win_utils;

const VISIBLE_AREA_PERCENT_THRESHOLD: i64 = 5;
const CLASS_PROGMAN: &str = "Progman";
const CLASS_WORKER_W: &str = "WorkerW";
const CLASS_SHELL_TRAY: &str = "Shell_TrayWnd";
const CLASS_SECONDARY_TRAY: &str = "Shell_SecondaryTrayWnd";
const CLASS_DOCK: &str = "X-Desk-Dock-Class";
const CLASS_VIDEO_HOST: &str = "X-Desk-VideoHost-Class";

static EVENT_OWNER: AtomicIsize = AtomicIsize::new(0);
static EVENT_MESSAGE: AtomicIsize = AtomicIsize::new(0);

pub(super) struct OcclusionWatcher {
    hooks: Vec<HWINEVENTHOOK>,
}

impl OcclusionWatcher {
    pub fn new(owner: HWND, message: u32) -> Self {
        EVENT_OWNER.store(owner.0 as isize, Ordering::Relaxed);
        EVENT_MESSAGE.store(message as isize, Ordering::Relaxed);

        let mut hooks = Vec::with_capacity(3);
        for (event_min, event_max) in [
            (EVENT_OBJECT_CREATE, EVENT_OBJECT_LOCATIONCHANGE),
            (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
            (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
        ] {
            let hook = unsafe {
                SetWinEventHook(
                    event_min,
                    event_max,
                    None,
                    Some(win_event_proc),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                )
            };
            if hook.is_invalid() {
                log::error!("SetWinEventHook failed for events {}..{}", event_min, event_max);
            } else {
                hooks.push(hook);
            }
        }

        Self { hooks }
    }
}

impl Drop for OcclusionWatcher {
    fn drop(&mut self) {
        for hook in self.hooks.drain(..) {
            unsafe {
                let _ = UnhookWinEvent(hook);
            }
        }
    }
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if id_object != 0 {
        return;
    }
    if event == EVENT_OBJECT_CREATE || event == EVENT_OBJECT_DESTROY || event == EVENT_OBJECT_SHOW {
        if hwnd.is_invalid() {
            return;
        }
    }

    let owner = HWND(EVENT_OWNER.load(Ordering::Relaxed) as *mut c_void);
    let message = EVENT_MESSAGE.load(Ordering::Relaxed) as u32;
    if !owner.is_invalid() && message != 0 {
        unsafe {
            let _ = PostMessageW(Some(owner), message, WPARAM(0), LPARAM(0));
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct DockRegion {
    pub hwnd: HWND,
    pub rect: RECT,
}

pub(super) fn collect_dock_occlusions(main_window: HWND, docks: &[DockRegion]) -> Vec<(HWND, bool)> {
    if docks.is_empty() {
        return Vec::new();
    }

    let occluders = collect_occluder_rects(main_window);
    docks
        .iter()
        .map(|dock| (dock.hwnd, is_dock_occluded(&dock.rect, &occluders)))
        .collect()
}

fn collect_occluder_rects(main_window: HWND) -> Vec<RectArea> {
    let mut context = EnumContext {
        main_window,
        rects: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut context as *mut EnumContext as isize),
        );
    }
    context.rects
}

struct EnumContext {
    main_window: HWND,
    rects: Vec<RectArea>,
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = unsafe { &mut *(lparam.0 as *mut EnumContext) };
    if let Some(rect) = occluder_rect(hwnd, context.main_window) {
        context.rects.push(rect);
    }
    true.into()
}

fn occluder_rect(hwnd: HWND, main_window: HWND) -> Option<RectArea> {
    if hwnd == main_window || !is_regular_visible_window(hwnd) {
        return None;
    }

    if let Ok(class_name) = window_class_name(hwnd) {
        if matches!(
            class_name.as_str(),
            CLASS_PROGMAN | CLASS_WORKER_W | CLASS_SHELL_TRAY | CLASS_SECONDARY_TRAY | CLASS_DOCK | CLASS_VIDEO_HOST
        ) {
            return None;
        }
    }

    window_rect(hwnd).ok().and_then(RectArea::from_rect)
}

fn is_regular_visible_window(hwnd: HWND) -> bool {
    if unsafe { IsWindowVisible(hwnd) } == FALSE || unsafe { IsIconic(hwnd) } != FALSE {
        return false;
    }
    if is_dwm_cloaked(hwnd) {
        return false;
    }

    let ex_style = match win_utils::get_window_ex_style(hwnd) {
        Ok(style) => style,
        Err(_) => return false,
    };
    if ex_style.contains(WS_EX_TOOLWINDOW) || ex_style.contains(WS_EX_TRANSPARENT) {
        return false;
    }
    true
}

fn is_dwm_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0
    }
}

fn window_rect(hwnd: HWND) -> Result<RECT> {
    let mut rect = RECT::default();
    if unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut c_void,
            size_of::<RECT>() as u32,
        )
    }
    .is_ok()
    {
        return Ok(rect);
    }

    unsafe { GetWindowRect(hwnd, &mut rect) }.context("GetWindowRect() failed")?;
    Ok(rect)
}

fn window_class_name(hwnd: HWND) -> Result<String> {
    let mut buffer = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if len == 0 {
        return Err(windows::core::Error::from_thread().into());
    }
    Ok(String::from_utf16_lossy(&buffer[..len as usize]))
}

fn is_dock_occluded(dock: &RECT, occluders: &[RectArea]) -> bool {
    RectArea::from_rect(*dock)
        .map(|dock| visible_area_after_occlusion(dock, occluders) * 100 <= dock.area() * VISIBLE_AREA_PERCENT_THRESHOLD)
        .unwrap_or(false)
}

fn visible_area_after_occlusion(dock: RectArea, occluders: &[RectArea]) -> i64 {
    let mut visible = vec![dock];
    for occluder in occluders {
        let mut next = Vec::with_capacity(visible.len() + 2);
        for rect in visible {
            next.extend(rect.subtract(*occluder));
        }
        visible = next;
        if visible.is_empty() {
            break;
        }
    }
    visible.iter().map(RectArea::area).sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RectArea {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl RectArea {
    fn from_rect(rect: RECT) -> Option<Self> {
        (rect.left < rect.right && rect.top < rect.bottom).then_some(Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        })
    }

    fn area(&self) -> i64 {
        i64::from(self.right - self.left) * i64::from(self.bottom - self.top)
    }

    fn subtract(self, other: Self) -> Vec<Self> {
        let left = self.left.max(other.left);
        let top = self.top.max(other.top);
        let right = self.right.min(other.right);
        let bottom = self.bottom.min(other.bottom);
        if left >= right || top >= bottom {
            return vec![self];
        }

        let mut parts = Vec::with_capacity(4);
        if self.top < top {
            parts.push(Self {
                left: self.left,
                top: self.top,
                right: self.right,
                bottom: top,
            });
        }
        if bottom < self.bottom {
            parts.push(Self {
                left: self.left,
                top: bottom,
                right: self.right,
                bottom: self.bottom,
            });
        }
        if self.left < left {
            parts.push(Self {
                left: self.left,
                top,
                right: left,
                bottom,
            });
        }
        if right < self.right {
            parts.push(Self {
                left: right,
                top,
                right: self.right,
                bottom,
            });
        }
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RectArea {
        RectArea {
            left,
            top,
            right,
            bottom,
        }
    }

    fn win_rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn single_window_covering_dock_is_occluded() {
        assert!(is_dock_occluded(&win_rect(0, 0, 100, 100), &[rect(0, 0, 100, 100)]));
    }

    #[test]
    fn multiple_windows_can_cover_dock_together() {
        assert!(is_dock_occluded(
            &win_rect(0, 0, 100, 100),
            &[rect(0, 0, 50, 100), rect(50, 0, 100, 100)]
        ));
    }

    #[test]
    fn ninety_five_percent_covered_is_occluded() {
        assert!(is_dock_occluded(&win_rect(0, 0, 100, 100), &[rect(0, 0, 95, 100)]));
    }

    #[test]
    fn ninety_four_percent_covered_is_visible() {
        assert!(!is_dock_occluded(&win_rect(0, 0, 100, 100), &[rect(0, 0, 94, 100)]));
    }

    #[test]
    fn non_intersecting_window_does_not_occlude_dock() {
        assert!(!is_dock_occluded(&win_rect(0, 0, 100, 100), &[rect(100, 0, 200, 100)]));
    }

    #[test]
    fn dock_regions_are_evaluated_independently() {
        let docks = [win_rect(0, 0, 100, 100), win_rect(100, 0, 200, 100)];
        let occluders = [rect(0, 0, 100, 100)];

        assert!(is_dock_occluded(&docks[0], &occluders));
        assert!(!is_dock_occluded(&docks[1], &occluders));
    }
}
