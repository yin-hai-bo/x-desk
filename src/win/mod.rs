pub mod main_window;
mod theme;

pub(crate) fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
