// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::{
    path::{Path, PathBuf},
    thread,
};

use anyhow::{bail, Context};
use config::WallpaperKind;
use single_instance::SingleInstanceMessage;
use tauri::Manager;
#[cfg(all(windows, not(debug_assertions)))]
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
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
    config: config::Config,
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
    state.config.content_for_monitor(0).is_some()
}

fn load_main_ui_state() -> anyhow::Result<MainUiState> {
    let config_file_path = config::Config::config_file_path(APP_NAME)?;
    let config = config::Config::load_from_file(&config_file_path)?;

    Ok(MainUiState {
        config_file_path,
        config,
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
            has_wallpaper_config
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
