use std::ops::Deref;

use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::Gdi::{BeginPaint, EndPaint, HDC, HGDIOBJ, PAINTSTRUCT, SelectObject},
        UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongPtrW, WINDOW_EX_STYLE},
    },
    core::{Error, HRESULT, Result},
};

pub fn width_of_rect(rect: &RECT) -> i32 {
    rect.right - rect.left
}

pub fn height_of_rect(rect: &RECT) -> i32 {
    rect.bottom - rect.top
}

pub fn has_hwnd_extended_style(hwnd: HWND, ex_style: WINDOW_EX_STYLE) -> bool {
    if hwnd.is_invalid() {
        return false;
    }
    let old = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    (old as u32) & ex_style.0 != 0
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
            return Err(Error::from_thread());
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
            return Err(Error::new(HRESULT::from_thread(), "Invalid GDI object"));
        }
        let old_obj = unsafe { SelectObject(hdc, h.into()) };
        if old_obj.is_invalid() {
            return Err(Error::from_thread());
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
