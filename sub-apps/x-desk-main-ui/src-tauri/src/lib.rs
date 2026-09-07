// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
};

use anyhow::{bail, Context};
use config::WallpaperKind;
use serde::{Deserialize, Serialize};
use single_instance::SingleInstanceMessage;
use tauri::Manager;
#[cfg(all(windows, not(debug_assertions)))]
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
#[cfg(windows)]
use windows::{
    core::BOOL,
    Win32::{
        Foundation::{FALSE, LPARAM, RECT, TRUE},
        Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
        UI::WindowsAndMessaging::MONITORINFOF_PRIMARY,
    },
};
#[cfg(all(windows, not(debug_assertions)))]
use windows_core::Interface;

#[allow(dead_code)]
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "webm", "mov", "m4v"];

struct MainUiState {
    config_file_path: PathBuf,
    config: Mutex<config::Config>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorLayoutViewModel {
    monitors: Vec<MonitorViewModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorViewModel {
    index: usize,
    is_primary: bool,
    rect: MonitorRectViewModel,
    content: Option<MonitorContentViewModel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorRectViewModel {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorContentViewModel {
    kind: WallpaperKind,
    source: String,
    preview: Option<MonitorPreviewViewModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorPreviewViewModel {
    kind: MonitorPreviewKind,
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode")]
enum MonitorContentUpdateRequest {
    LocalVideo { path: String },
    InternetVideo { url: String },
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum MonitorPreviewKind {
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DisplayMonitorInfo {
    is_primary: bool,
    rect: MonitorRectViewModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
struct SelectedLocalFileSource {
    kind: WallpaperKind,
    source: String,
    preview_source: String,
}

#[allow(dead_code)]
fn source_for_selected_local_file(path: &Path) -> anyhow::Result<SelectedLocalFileSource> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .context("Selected file has no supported extension")?;

    if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(SelectedLocalFileSource {
            kind: WallpaperKind::Video,
            source: path.display().to_string(),
            preview_source: path.display().to_string(),
        });
    }

    bail!("Unsupported selected file extension: .{}", extension)
}

#[allow(dead_code)]
fn path_to_file_url(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if let Some(rest) = normalized.strip_prefix("//") {
        format!("file://{}", encode_file_url_spaces(rest))
    } else {
        format!("file:///{}", encode_file_url_spaces(&normalized))
    }
}

fn is_internet_video_url(source: &str) -> bool {
    let source = source.trim().to_ascii_lowercase();
    source.starts_with("http://") || source.starts_with("https://")
}

fn file_url_from_source(source: &str) -> String {
    if source.to_ascii_lowercase().starts_with("file://") {
        source.to_string()
    } else if is_internet_video_url(source) {
        source.to_string()
    } else {
        path_to_file_url(Path::new(source))
    }
}

fn extension_from_source(source: &str) -> Option<String> {
    let source = source.trim();
    let source_without_query = source.split(['?', '#']).next().unwrap_or(source);
    Path::new(source_without_query)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
}

fn preview_for_video_source(source: &str, preview_source: Option<&str>) -> Option<MonitorPreviewViewModel> {
    let preview_source = preview_source.unwrap_or(source).trim();
    if !is_internet_video_url(preview_source) {
        let extension = extension_from_source(preview_source)?;
        if !VIDEO_EXTENSIONS.contains(&extension.as_str()) {
            return None;
        }
    }

    Some(MonitorPreviewViewModel {
        kind: MonitorPreviewKind::Video,
        url: file_url_from_source(preview_source),
    })
}

fn save_monitor_content_update(
    config: &mut config::Config,
    monitor_index: usize,
    request: MonitorContentUpdateRequest,
) -> anyhow::Result<()> {
    match request {
        MonitorContentUpdateRequest::LocalVideo { path } => {
            let path = path.trim();
            if path.is_empty() {
                bail!("Local video path is empty");
            }

            let source = source_for_selected_local_file(Path::new(path))?;
            config.set_video_monitor_source(monitor_index, source.source, Some(source.preview_source));
        }
        MonitorContentUpdateRequest::InternetVideo { url } => {
            let url = url.trim();
            if !is_internet_video_url(url) {
                bail!("Internet video URL must start with http:// or https://");
            }

            config.set_video_monitor_source(monitor_index, url.to_string(), Some(url.to_string()));
        }
        MonitorContentUpdateRequest::None => {
            config.clear_monitor_source(monitor_index);
        }
    }

    Ok(())
}

fn monitor_layout_view_model_from_config(config: &config::Config) -> Result<MonitorLayoutViewModel, String> {
    let monitors = enumerate_display_monitors().map_err(|error| format!("{error:#}"))?;
    Ok(monitor_layout_view_model_from_parts(monitors, config))
}

fn refreshed_monitor_layout_view_model(state: &MainUiState) -> Result<MonitorLayoutViewModel, String> {
    let config = config::Config::load_from_file(&state.config_file_path).map_err(|error| format!("{error:#}"))?;
    let view_model = monitor_layout_view_model_from_config(&config)?;

    *state
        .config
        .lock()
        .map_err(|_| "Main UI config state is unavailable".to_string())? = config;

    Ok(view_model)
}

fn monitor_layout_view_model_from_state(state: &MainUiState) -> Result<MonitorLayoutViewModel, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "Main UI config state is unavailable".to_string())?;

    monitor_layout_view_model_from_config(&config)
}

fn update_monitor_content(
    state: &MainUiState,
    monitor_index: usize,
    request: MonitorContentUpdateRequest,
) -> Result<MonitorLayoutViewModel, String> {
    let mut config = state
        .config
        .lock()
        .map_err(|_| "Main UI config state is unavailable".to_string())?;

    save_monitor_content_update(&mut config, monitor_index, request).map_err(|error| format!("{error:#}"))?;
    config
        .save_to_file(&state.config_file_path)
        .map_err(|error| format!("{error:#}"))?;

    monitor_layout_view_model_from_config(&config)
}

fn monitor_layout_view_model_from_parts(
    monitors: Vec<DisplayMonitorInfo>,
    config: &config::Config,
) -> MonitorLayoutViewModel {
    MonitorLayoutViewModel {
        monitors: monitors
            .into_iter()
            .enumerate()
            .map(|(index, monitor)| {
                let content = config.content_for_monitor(index).map(|content| {
                    let preview = match content.kind {
                        WallpaperKind::Video => {
                            preview_for_video_source(&content.source, config.preview_source_for_monitor(index))
                        }
                    };

                    MonitorContentViewModel {
                        kind: content.kind,
                        source: content.source,
                        preview,
                    }
                });

                MonitorViewModel {
                    index,
                    is_primary: monitor.is_primary,
                    rect: monitor.rect,
                    content,
                }
            })
            .collect(),
    }
}

#[cfg(windows)]
unsafe extern "system" fn enum_display_monitors_callback(
    monitor: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    if FALSE == unsafe { GetMonitorInfoW(monitor, &mut info) } {
        return TRUE;
    }

    let monitors = unsafe { &mut *(lparam.0 as *mut Vec<DisplayMonitorInfo>) };
    monitors.push(DisplayMonitorInfo {
        is_primary: (info.dwFlags & MONITORINFOF_PRIMARY) != 0,
        rect: MonitorRectViewModel::from(info.rcMonitor),
    });

    TRUE
}

#[cfg(windows)]
fn enumerate_display_monitors() -> anyhow::Result<Vec<DisplayMonitorInfo>> {
    let mut monitors = Vec::new();
    if FALSE
        == unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(enum_display_monitors_callback),
                LPARAM(&mut monitors as *mut Vec<DisplayMonitorInfo> as isize),
            )
        }
    {
        bail!("Enumerate display monitors failed: {}", std::io::Error::last_os_error());
    }

    Ok(monitors)
}

impl From<RECT> for MonitorRectViewModel {
    fn from(rect: RECT) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        }
    }
}

#[allow(dead_code)]
fn encode_file_url_spaces(path: &str) -> String {
    path.replace(' ', "%20")
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn exit_main_ui(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn config_file_path(state: tauri::State<MainUiState>) -> String {
    state.config_file_path.display().to_string()
}

#[tauri::command]
fn has_wallpaper_config(state: tauri::State<MainUiState>) -> bool {
    match state.config.lock() {
        Ok(config) => config.content_for_monitor(0).is_some(),
        Err(_) => false,
    }
}

#[tauri::command]
fn monitor_layout_view_model(state: tauri::State<MainUiState>) -> Result<MonitorLayoutViewModel, String> {
    monitor_layout_view_model_from_state(&state)
}

#[tauri::command]
fn refresh_monitor_layout_view_model(state: tauri::State<MainUiState>) -> Result<MonitorLayoutViewModel, String> {
    refreshed_monitor_layout_view_model(&state)
}

#[tauri::command]
fn set_monitor_content(
    state: tauri::State<MainUiState>,
    monitor_index: usize,
    request: MonitorContentUpdateRequest,
) -> Result<MonitorLayoutViewModel, String> {
    update_monitor_content(&state, monitor_index, request)
}

#[tauri::command]
fn path_exists(path: &str) -> bool {
    Path::new(path.trim()).exists()
}

fn load_main_ui_state() -> anyhow::Result<MainUiState> {
    let config_file_path = config::Config::config_file_path(common::APP_NAME)?;
    let config = config::Config::load_from_file(&config_file_path)?;

    Ok(MainUiState {
        config_file_path,
        config: Mutex::new(config),
    })
}

#[cfg(all(windows, not(debug_assertions)))]
fn disable_release_webview_features(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(webview) = app.get_webview_window("main") {
        webview.with_webview(|platform_webview| {
            let controller = platform_webview.controller();
            let result = (|| unsafe {
                let settings = controller.CoreWebView2()?.Settings()?;
                settings.SetAreDevToolsEnabled(false)?;
                settings.SetAreDefaultContextMenusEnabled(false)?;

                if let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>() {
                    settings3.SetAreBrowserAcceleratorKeysEnabled(false)?;
                }

                windows_core::Result::Ok(())
            })();

            if let Err(error) = result {
                eprintln!("Disable release WebView features failed: {error}");
            }
        })?;
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut single_instanceinstance = match single_instance::SingleInstance::acquire(common::MAIN_UI_INSTANCE_NAME) {
        Ok(Some(instance)) => instance,
        Ok(None) => return,
        Err(error) => {
            eprintln!("Create single-instance guard failed: {error:#}");
            return;
        }
    };
    let receiver = single_instanceinstance.take_message_receiver();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            exit_main_ui,
            config_file_path,
            has_wallpaper_config,
            monitor_layout_view_model,
            refresh_monitor_layout_view_model,
            set_monitor_content,
            path_exists
        ])
        .setup(move |_app| {
            _app.manage(load_main_ui_state()?);

            if let Some(receiver) = receiver {
                let app_handle = _app.handle().clone();
                thread::spawn(move || {
                    for message in receiver {
                        match message {
                            SingleInstanceMessage::SecondInstanceStarted => {
                                if let Some(window) = app_handle.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.unminimize();
                                    let _ = window.set_focus();
                                }
                            }
                            SingleInstanceMessage::ExitRequested => {
                                app_handle.exit(0);
                            }
                        }
                    }
                });
            }

            #[cfg(all(windows, not(debug_assertions)))]
            disable_release_webview_features(_app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{path_to_file_url, source_for_selected_local_file};
    use config::WallpaperKind;
    use std::path::Path;

    #[test]
    fn html_file_source_is_rejected() {
        let error = source_for_selected_local_file(Path::new("C:\\pages\\index.html")).unwrap_err();

        assert!(error.to_string().contains("Unsupported selected file extension: .html"));
    }

    #[test]
    fn video_file_source_uses_direct_path() {
        let source = source_for_selected_local_file(Path::new("C:\\videos\\one clip.mp4")).unwrap();

        assert_eq!(source.kind, WallpaperKind::Video);
        assert_eq!(source.source, "C:\\videos\\one clip.mp4");
        assert_eq!(source.preview_source, "C:\\videos\\one clip.mp4");
    }

    #[test]
    fn all_supported_video_extensions_use_direct_path() {
        for extension in ["mp4", "webm", "mov", "m4v"] {
            let path = format!("C:\\videos\\sample.{extension}");
            let source = source_for_selected_local_file(Path::new(&path)).unwrap();

            assert_eq!(source.kind, WallpaperKind::Video);
            assert_eq!(source.source, path);
        }
    }

    #[test]
    fn unsupported_extension_is_rejected() {
        let error = source_for_selected_local_file(Path::new("C:\\files\\notes.txt")).unwrap_err();

        assert!(error.to_string().contains("Unsupported selected file extension: .txt"));
    }

    #[test]
    fn missing_extension_is_rejected() {
        let error = source_for_selected_local_file(Path::new("C:\\files\\notes")).unwrap_err();

        assert!(error.to_string().contains("Selected file has no supported extension"));
    }

    #[test]
    fn path_to_file_url_normalizes_windows_paths_and_spaces() {
        assert_eq!(
            path_to_file_url(Path::new("C:\\videos\\one clip.mp4")),
            "file:///C:/videos/one%20clip.mp4"
        );
    }
}
