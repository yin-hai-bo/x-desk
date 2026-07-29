use std::{io, sync::OnceLock};

use ::windows::Win32::UI::WindowsAndMessaging::{RegisterClassExW, WNDCLASSEXW};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            CLEARTYPE_QUALITY, CreateFontW, DT_CALCRECT, DT_SINGLELINE, DrawTextW, FONT_CHARSET, FONT_CLIP_PRECISION,
            FONT_OUTPUT_PRECISION, FW_NORMAL, GetDC, HBRUSH, HFONT, InvalidateRect, ReleaseDC, SelectObject, SetBkMode,
            SetTextColor, TRANSPARENT,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CS_HREDRAW, CS_VREDRAW, DefWindowProcW, GetClientRect, HCURSOR, HICON, IDC_ARROW, IDC_HAND, LoadCursorW,
            SW_SHOW, SetCursor, WM_ERASEBKGND, WM_LBUTTONUP, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SETCURSOR,
            WM_SETTINGCHANGE, WS_CHILD, WS_EX_TRANSPARENT, WS_VISIBLE,
        },
    },
    core::{Owned, PCWSTR, w},
};

use crate::win::{wide_string::WideString, win_utils::PaintDC, window::Window};

use super::theme;

const CLASS_NAME: PCWSTR = w!("YHB-XDeskHyperLinkText");

pub struct HyperLinkFont {
    name: String,
    point_size: i32,
}

impl HyperLinkFont {
    pub fn new(name: impl Into<String>, point_size: i32) -> Self {
        Self {
            name: name.into(),
            point_size,
        }
    }
}

pub struct Anchor {
    pub horizontal: HorizontalAnchor,
    pub vertical: VerticalAnchor,
}

impl Anchor {
    pub fn new(horizontal: HorizontalAnchor, vertical: VerticalAnchor) -> Self {
        Self { horizontal, vertical }
    }
}

#[allow(dead_code)]
pub enum HorizontalAnchor {
    Left(i32),
    Center(i32),
    Right(i32),
}

#[allow(dead_code)]
pub enum VerticalAnchor {
    Top(i32),
    Center(i32),
    Bottom(i32),
}

pub struct HyperLinkText {
    font: Owned<HFONT>,
    on_click: Option<Box<dyn Fn() + 'static>>,
}

impl HyperLinkText {
    pub fn create(
        parent: HWND,
        text: &str,
        anchor: Anchor,
        font: HyperLinkFont,
        on_click: Option<impl Fn() + 'static>,
    ) -> anyhow::Result<Box<Window<HyperLinkText>>> {
        let instance = unsafe { GetModuleHandleW(PCWSTR::null())?.into() };
        register_window_class(instance)?;

        let font = unsafe { Owned::new(create_underline_font(parent, &font)?) };
        let bounds = autosize_bounds(parent, text, anchor, *font)?;
        let component = Self {
            font,
            on_click: if let Some(cb) = on_click {
                Some(Box::new(cb))
            } else {
                None
            },
        };

        let window = Window::create(
            WS_EX_TRANSPARENT,
            CLASS_NAME,
            WideString::new(text).as_pcwstr(),
            WS_CHILD | WS_VISIBLE,
            bounds.left,
            bounds.top,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            Some(parent),
            None,
            Some(instance.into()),
            component,
        )?;
        window.show_window(SW_SHOW);
        return Ok(window);
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match message {
            WM_NCCREATE => Window::<HyperLinkText>::on_wm_nccreate(hwnd, lparam),
            WM_PAINT => {
                let _ = paint(hwnd);
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_SETTINGCHANGE => {
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, true);
                };
                return LRESULT(0);
            }
            WM_SETCURSOR => {
                unsafe {
                    SetCursor(Some(cursor()));
                }
                return LRESULT(1);
            }
            WM_LBUTTONUP => {
                if let Some(p) = unsafe { Window::<HyperLinkText>::get_self_from_hwnd(hwnd) } {
                    let w = unsafe { &mut *p };
                    if let Some(on_click) = &w.component().on_click {
                        on_click();
                    };
                }
                return LRESULT(0);
            }
            WM_NCDESTROY => Window::<HyperLinkText>::on_wm_ncdestroy(hwnd),
            _ => {}
        }
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
}

fn cursor() -> HCURSOR {
    unsafe { LoadCursorW(None, IDC_HAND).unwrap_or(LoadCursorW(None, IDC_ARROW).unwrap_or_default()) }
}

fn register_window_class(inst: HINSTANCE) -> anyhow::Result<()> {
    static REGISTERED: OnceLock<io::Result<()>> = OnceLock::new();

    REGISTERED
        .get_or_init(|| {
            let window_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(HyperLinkText::window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: inst,
                hIcon: HICON::default(),
                hIconSm: HICON::default(),
                hCursor: cursor(),
                hbrBackground: HBRUSH::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: CLASS_NAME,
            };
            if unsafe { RegisterClassExW(&window_class) } == 0 {
                Err(io::Error::last_os_error().into())
            } else {
                Ok(())
            }
        })
        .as_ref()
        .map(|_| ())
        .map_err(|error| error.into())
}

fn create_underline_font(hwnd: HWND, font: &HyperLinkFont) -> anyhow::Result<HFONT> {
    let font_name = WideString::new(&font.name);
    let mut dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    if dpi == 0 {
        dpi = 96;
    }
    let height = -((font.point_size.max(1) * dpi as i32) / 72);
    let handle = unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            1,
            0,
            FONT_CHARSET::default(),
            FONT_OUTPUT_PRECISION::default(),
            FONT_CLIP_PRECISION::default(),
            CLEARTYPE_QUALITY,
            0,
            font_name.as_pcwstr(),
        )
    };
    if handle.is_invalid() {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(handle)
    }
}

fn autosize_bounds(hwnd: HWND, text: &str, anchor: Anchor, font: HFONT) -> anyhow::Result<RECT> {
    let dc = unsafe { GetDC(Some(hwnd)) };
    if dc.is_invalid() {
        return Err(io::Error::last_os_error().into());
    }

    let result = (|| {
        let mut text = WideString::new(text).clone_u16_array();
        let mut text_bounds = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe {
            let old_font = SelectObject(dc, font.into());
            DrawTextW(dc, &mut text, &mut text_bounds, DT_SINGLELINE | DT_CALCRECT);
            SelectObject(dc, old_font);
        }

        let width = (text_bounds.right - text_bounds.left).max(1);
        let height = (text_bounds.bottom - text_bounds.top).max(1);
        let left = match anchor.horizontal {
            HorizontalAnchor::Left(x) => x,
            HorizontalAnchor::Center(x) => x - width / 2,
            HorizontalAnchor::Right(x) => x - width,
        };
        let top = match anchor.vertical {
            VerticalAnchor::Top(y) => y,
            VerticalAnchor::Center(y) => y - height / 2,
            VerticalAnchor::Bottom(y) => y - height,
        };

        Ok(RECT {
            left,
            top,
            right: left + width,
            bottom: top + height,
        })
    })();

    unsafe {
        let _ = ReleaseDC(Some(hwnd), dc);
    }
    result
}

fn paint(hwnd: HWND) -> anyhow::Result<()> {
    let dc = PaintDC::new(hwnd)?;
    let w = unsafe { Window::<HyperLinkText>::get_self_from_hwnd(hwnd) }
        .ok_or_else(|| anyhow::anyhow!("Cannot get window object when paint"))?;
    let window = unsafe { &*w };

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        let _ = GetClientRect(hwnd, &mut rect);
    }

    let mut text = window.get_window_text()?;
    unsafe {
        let old_font = SelectObject(*dc, (*window.component().font).into());
        SetBkMode(*dc, TRANSPARENT);
        SetTextColor(*dc, theme::hyperlink_text_color());
        DrawTextW(*dc, &mut text, &mut rect, DT_SINGLELINE);
        SelectObject(*dc, old_font);
    }
    Ok(())
}
