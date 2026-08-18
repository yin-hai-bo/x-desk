use crate::win::{desktop::Desktop, win_utils};
use anyhow::Result;
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::MapWindowPoints,
    UI::WindowsAndMessaging::{
        HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, WS_CAPTION, WS_CHILD,
        WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP,
        WS_SYSMENU, WS_THICKFRAME,
    },
};

pub(super) struct AttachedContentWindow {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

pub(super) fn attach_content_window(hwnd: HWND, desktop: &Desktop, rect: &RECT) -> Result<AttachedContentWindow> {
    win_utils::set_window_pos(
        hwnd,
        Some(HWND_BOTTOM),
        rect.left,
        rect.top,
        win_utils::width_of_rect(rect),
        win_utils::height_of_rect(rect),
        SWP_NOACTIVATE,
    )?;

    let mut pt = [POINT::default()];
    unsafe {
        let _ = MapWindowPoints(Some(hwnd), Some(desktop.parent_of_wallpaper()), &mut pt);
    }

    if desktop.is_raised_desktop() {
        win_utils::set_window_transparency(hwnd, 255)?;
        set_to_child_window(hwnd, desktop.parent_of_wallpaper())?;
        win_utils::set_window_pos(
            hwnd,
            Some(desktop.shell_dll_def_view()),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )?;
        desktop.ensure_children_zorder()?;
    } else {
        set_to_child_window(hwnd, desktop.parent_of_wallpaper())?;
    }

    let attached = AttachedContentWindow {
        x: pt[0].x,
        y: pt[0].y,
        width: win_utils::width_of_rect(rect),
        height: win_utils::height_of_rect(rect),
    };

    win_utils::set_window_pos(
        hwnd,
        None,
        attached.x,
        attached.y,
        attached.width,
        attached.height,
        SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_NOZORDER,
    )?;

    Ok(attached)
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
