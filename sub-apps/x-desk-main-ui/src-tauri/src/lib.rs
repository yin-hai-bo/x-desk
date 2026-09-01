// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
};

use anyhow::{bail, Context};
use config::WallpaperKind;
use serde::Serialize;
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

const APP_NAME: &str = "x-desk";
const MAIN_UI_INSTANCE_NAME: &str = "x-desk-main-ui";
#[allow(dead_code)]
const VIDEO_SOURCE_TEMPLATE: &str = r#"<html>
<head>
<style>
  html, body {
    margin: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: black;
  }

  video {
    width: 100vw;
    height: 100vh;
    object-fit: contain;
    display: block;
  }
</style>
</head>
<body>
<video
  src="{{VIDEO_FILE_URL}}"
  autoplay loop muted playsinline />
</body></html>"#;

#[allow(dead_code)]
const HTML_EXTENSIONS: &[&str] = &["html", "htm"];
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum MonitorPreviewKind {
    Html,
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

    if HTML_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(SelectedLocalFileSource {
            kind: WallpaperKind::WebView,
            source: path.display().to_string(),
            preview_source: path.display().to_string(),
        });
    }

    if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        let video_file_url = path_to_file_url(path);
        return Ok(SelectedLocalFileSource {
            kind: WallpaperKind::WebView,
            source: VIDEO_SOURCE_TEMPLATE.replace("{{VIDEO_FILE_URL}}", &video_file_url),
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

fn file_url_from_source(source: &str) -> String {
    if source.to_ascii_lowercase().starts_with("file://") {
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

fn preview_for_webview_source(source: &str, preview_source: Option<&str>) -> Option<MonitorPreviewViewModel> {
    let preview_source = preview_source.unwrap_or(source).trim();
    let extension = extension_from_source(preview_source)?;
    let kind = if HTML_EXTENSIONS.contains(&extension.as_str()) {
        MonitorPreviewKind::Html
    } else if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        MonitorPreviewKind::Video
    } else {
        return None;
    };

    Some(MonitorPreviewViewModel {
        kind,
        url: file_url_from_source(preview_source),
    })
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
                        WallpaperKind::WebView => {
                            preview_for_webview_source(&content.source, config.preview_source_for_monitor(index))
                        }
                        WallpaperKind::Video => None,
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
    let monitors = enumerate_display_monitors().map_err(|error| format!("{error:#}"))?;
    let config = state
        .config
        .lock()
        .map_err(|_| "Main UI config state is unavailable".to_string())?;

    Ok(monitor_layout_view_model_from_parts(monitors, &config))
}

#[tauri::command]
fn refresh_monitor_layout_view_model(state: tauri::State<MainUiState>) -> Result<MonitorLayoutViewModel, String> {
    let monitors = enumerate_display_monitors().map_err(|error| format!("{error:#}"))?;
    let config = config::Config::load_from_file(&state.config_file_path).map_err(|error| format!("{error:#}"))?;
    let view_model = monitor_layout_view_model_from_parts(monitors, &config);

    *state
        .config
        .lock()
        .map_err(|_| "Main UI config state is unavailable".to_string())? = config;

    Ok(view_model)
}

fn load_main_ui_state() -> anyhow::Result<MainUiState> {
    let config_file_path = config::Config::config_file_path(APP_NAME)?;
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
    let mut single_instanceinstance = match single_instance::SingleInstance::acquire(MAIN_UI_INSTANCE_NAME) {
        Ok(Some(instance)) => instance,
        Ok(None) => return,
        Err(error) => {
            eprintln!("Create single-instance guard failed: {error:#}");
            return;
        }
    };
    let receiver = single_instanceinstance.take_message_receiver();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            exit_main_ui,
            config_file_path,
            has_wallpaper_config,
            monitor_layout_view_model,
            refresh_monitor_layout_view_model
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
    use super::{path_to_file_url, source_for_selected_local_file, VIDEO_SOURCE_TEMPLATE};
    use config::WallpaperKind;
    use std::path::Path;

    #[test]
    fn html_file_source_uses_direct_path() {
        let source = source_for_selected_local_file(Path::new("C:\\pages\\index.html")).unwrap();

        assert_eq!(source.kind, WallpaperKind::WebView);
        assert_eq!(source.source, "C:\\pages\\index.html");
        assert_eq!(source.preview_source, "C:\\pages\\index.html");
    }

    #[test]
    fn html_extension_matching_is_case_insensitive() {
        let source = source_for_selected_local_file(Path::new("C:\\pages\\INDEX.HTM")).unwrap();

        assert_eq!(source.kind, WallpaperKind::WebView);
        assert_eq!(source.source, "C:\\pages\\INDEX.HTM");
        assert_eq!(source.preview_source, "C:\\pages\\INDEX.HTM");
    }

    #[test]
    fn video_file_source_uses_html_template_with_file_url() {
        let source = source_for_selected_local_file(Path::new("C:\\videos\\one clip.mp4")).unwrap();

        assert_eq!(source.kind, WallpaperKind::WebView);
        assert_eq!(source.preview_source, "C:\\videos\\one clip.mp4");
        assert!(source.source.starts_with("<html>"));
        assert!(source.source.contains("src=\"file:///C:/videos/one%20clip.mp4\""));
        assert!(!source.source.contains("{{VIDEO_FILE_URL}}"));
    }

    #[test]
    fn all_supported_video_extensions_generate_template_source() {
        for extension in ["mp4", "webm", "mov", "m4v"] {
            let path = format!("C:\\videos\\sample.{extension}");
            let source = source_for_selected_local_file(Path::new(&path)).unwrap();

            assert_eq!(source.kind, WallpaperKind::WebView);
            assert!(source.source.contains("<video"));
            assert!(source.source.contains(format!("sample.{extension}").as_str()));
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

    #[test]
    fn video_template_uses_expected_placeholder() {
        assert!(VIDEO_SOURCE_TEMPLATE.contains("{{VIDEO_FILE_URL}}"));
    }
}
