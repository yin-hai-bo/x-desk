#![allow(dead_code)]

use std::io;
use windows::{
    Win32::{
        Foundation::{FALSE, LPARAM, RECT, TRUE},
        Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
        UI::WindowsAndMessaging::MONITORINFOF_PRIMARY,
    },
    core::BOOL,
};

pub struct Monitor {
    pub is_primary: bool,
    pub rect_work: RECT,
}

impl Monitor {
    pub fn new(h: HMONITOR) -> io::Result<Self> {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if FALSE == unsafe { GetMonitorInfoW(h, &mut info) } {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            is_primary: (info.dwFlags & MONITORINFOF_PRIMARY) != 0,
            rect_work: info.rcWork.clone(),
        })
    }
}

pub struct MonitorManager {
    multi_monitor: bool,
}

impl MonitorManager {
    unsafe extern "system" fn enum_monitors_callback(h: HMONITOR, _dc: HDC, _rect: *mut RECT, lparam: LPARAM) -> BOOL {
        if let Ok(monitor) = Monitor::new(h) {
            let monitors = unsafe { &mut *(lparam.0 as *mut Vec<Monitor>) };
            monitors.push(monitor);
        }
        return TRUE;
    }

    pub fn refresh_monitors() -> io::Result<Vec<Monitor>> {
        let mut monitors: Vec<Monitor> = Vec::with_capacity(4);
        if FALSE
            == unsafe {
                EnumDisplayMonitors(
                    None,
                    None,
                    Some(Self::enum_monitors_callback),
                    LPARAM(&mut monitors as *mut Vec<Monitor> as isize),
                )
            }
        {
            return Err(io::Error::last_os_error());
        }
        Ok(monitors)
    }
}
