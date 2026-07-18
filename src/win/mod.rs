pub mod main_window;
mod resource_ids;
mod theme;
mod tray_icon;

pub(crate) fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
