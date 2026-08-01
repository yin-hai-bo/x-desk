use std::ops::Deref;

use anyhow::{Context, Result, bail};
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, RECT, TRUE},
        Graphics::Gdi::{BeginPaint, EndPaint, HDC, HGDIOBJ, PAINTSTRUCT, SelectObject},
        UI::WindowsAndMessaging::{
            EnumChildWindows, GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, LWA_ALPHA, SET_WINDOW_POS_FLAGS,
            SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW, SetWindowPos, WINDOW_EX_STYLE,
            WINDOW_LONG_PTR_INDEX, WINDOW_STYLE, WS_EX_LAYERED,
        },
    },
    core::{BOOL, Error, HRESULT},
};

pub fn width_of_rect(rect: &RECT) -> i32 {
    rect.right - rect.left
}

pub fn height_of_rect(rect: &RECT) -> i32 {
    rect.bottom - rect.top
}

fn check_hwnd(hwnd: HWND) -> Result<()> {
    if hwnd.is_invalid() {
        bail!("Invalid window");
    }
    Ok(())
}

pub fn get_window_long_ptr(hwnd: HWND, index: WINDOW_LONG_PTR_INDEX) -> Result<isize> {
    check_hwnd(hwnd)?;
    let result = unsafe { GetWindowLongPtrW(hwnd, index) };
    Ok(result)
}

pub fn set_window_long_ptr(hwnd: HWND, index: WINDOW_LONG_PTR_INDEX, value: isize) -> Result<isize> {
    check_hwnd(hwnd)?;
    let result = unsafe { SetWindowLongPtrW(hwnd, index, value) };
    Ok(result)
}

pub fn get_window_style(hwnd: HWND) -> Result<WINDOW_STYLE> {
    get_window_long_ptr(hwnd, GWL_STYLE).map(|r| WINDOW_STYLE(r as u32))
}

pub fn set_window_style(hwnd: HWND, style: WINDOW_STYLE) -> Result<WINDOW_STYLE> {
    set_window_long_ptr(hwnd, GWL_STYLE, style.0 as isize).map(|r| WINDOW_STYLE(r as u32))
}

pub fn get_window_ex_style(hwnd: HWND) -> Result<WINDOW_EX_STYLE> {
    get_window_long_ptr(hwnd, GWL_EXSTYLE).map(|r| WINDOW_EX_STYLE(r as u32))
}

pub fn set_window_ex_style(hwnd: HWND, ex_style: WINDOW_EX_STYLE) -> Result<WINDOW_EX_STYLE> {
    set_window_long_ptr(hwnd, GWL_EXSTYLE, ex_style.0 as isize).map(|r| WINDOW_EX_STYLE(r as u32))
}

pub fn add_window_style(hwnd: HWND, style_to_add: WINDOW_STYLE) -> Result<WINDOW_STYLE> {
    let old = get_window_style(hwnd)?;
    set_window_style(hwnd, old | style_to_add)
}

pub fn add_window_ex_style(hwnd: HWND, ex_style_to_add: WINDOW_EX_STYLE) -> Result<WINDOW_EX_STYLE> {
    let old = get_window_ex_style(hwnd)?;
    set_window_ex_style(hwnd, old | ex_style_to_add)
}

unsafe extern "system" fn get_last_child_window_callback(hwnd: HWND, param: LPARAM) -> BOOL {
    let p = unsafe { &mut *(param.0 as *mut HWND) };
    *p = hwnd;
    return TRUE;
}

pub struct ChildWindowInfo {
    pub first_child: Option<HWND>,
    pub last_child: Option<HWND>,
}

unsafe extern "system" fn get_first_and_last_child_window_callback(hwnd: HWND, param: LPARAM) -> BOOL {
    let info = unsafe { &mut *(param.0 as *mut ChildWindowInfo) };
    if info.first_child.is_none() {
        info.first_child = Some(hwnd);
    }
    info.last_child = Some(hwnd);
    TRUE
}

pub fn get_first_and_last_child_window(hwnd: Option<HWND>) -> Option<ChildWindowInfo> {
    let mut info = ChildWindowInfo {
        first_child: None,
        last_child: None,
    };
    unsafe {
        let _ = EnumChildWindows(
            hwnd,
            Some(get_first_and_last_child_window_callback),
            LPARAM(&mut info as *mut ChildWindowInfo as isize),
        );
    }
    if info.first_child.is_none() && info.last_child.is_none() {
        None
    } else {
        Some(info)
    }
}

pub fn get_last_child_window(hwnd: Option<HWND>) -> Option<HWND> {
    let mut result = HWND::default();
    unsafe {
        let _ = EnumChildWindows(
            hwnd,
            Some(get_last_child_window_callback),
            LPARAM(&mut result as *mut HWND as isize),
        );
    }
    if result.is_invalid() { None } else { Some(result) }
}

pub fn set_window_transparency(hwnd: HWND, transparency: u8) -> Result<()> {
    set_window_layered(hwnd)?;
    unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), transparency, LWA_ALPHA) }
        .context("SetLayeredWindowAttributes() failed")
}

pub fn set_window_layered(hwnd: HWND) -> Result<()> {
    let ex_style = get_window_ex_style(hwnd)?;
    if !ex_style.contains(WS_EX_LAYERED) {
        set_window_ex_style(hwnd, WS_EX_LAYERED | ex_style)?;
    }
    Ok(())
}

pub fn set_window_pos(
    hwnd: HWND,
    insert_after: Option<HWND>,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    flags: SET_WINDOW_POS_FLAGS,
) -> Result<()> {
    unsafe { SetWindowPos(hwnd, insert_after, x, y, cx, cy, flags).context("SetWindowPos() failed") }
}

pub fn set_parent(hwnd: HWND, parent: Option<HWND>) -> Result<HWND> {
    unsafe { SetParent(hwnd, parent).context("SetParent() failed") }
}

pub struct PaintDC {
    hwnd: HWND,
    dc: HDC,
    ps: PAINTSTRUCT,
}

impl PaintDC {
    pub fn new(hwnd: HWND) -> Result<Self> {
        let mut ps = unsafe { std::mem::zeroed::<PAINTSTRUCT>() };
        let dc = unsafe { BeginPaint(hwnd, &mut ps) };
        if dc.is_invalid() {
            return Err(Error::from_thread().into());
        }
        Ok(Self { hwnd, dc, ps })
    }
}

impl Deref for PaintDC {
    type Target = HDC;

    fn deref(&self) -> &Self::Target {
        &self.dc
    }
}

impl Drop for PaintDC {
    fn drop(&mut self) {
        unsafe {
            let _ = EndPaint(self.hwnd, &mut self.ps);
        }
    }
}

#[allow(dead_code)]
pub struct GDIObjectSelector<T>
where
    T: Copy + Default + Into<HGDIOBJ>,
{
    hdc: HDC,
    h: T,
    old_obj: HGDIOBJ,
}

impl<T> GDIObjectSelector<T>
where
    T: Copy + Default + Into<HGDIOBJ>,
{
    #[allow(dead_code)]
    pub fn select_to_hdc(hdc: HDC, h: T) -> Result<Self> {
        let new_obj = h.into();
        if new_obj.is_invalid() {
            return Err(windows::core::Error::new(HRESULT::from_thread(), "Invalid GDI Object").into());
        }
        let old_obj = unsafe { SelectObject(hdc, h.into()) };
        if old_obj.is_invalid() {
            return Err(Error::from_thread().into());
        }
        Ok(Self { hdc, old_obj, h })
    }
}

impl<T> Deref for GDIObjectSelector<T>
where
    T: Copy + Default + Into<HGDIOBJ>,
{
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.h
    }
}

impl<T> Drop for GDIObjectSelector<T>
where
    T: Copy + Default + Into<HGDIOBJ>,
{
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.hdc, self.old_obj);
        };
    }
}
