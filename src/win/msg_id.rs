use windows::Win32::UI::WindowsAndMessaging::WM_APP;

pub(super) const TRAY_ICON_MESSAGE: u32 = WM_APP + 1;
pub(super) const WORKER_W_DESTROY_MESSAGE: u32 = WM_APP + 2;
