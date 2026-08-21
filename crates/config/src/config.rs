use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
};
use windows::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorConfig {
    kind: WallpaperKind,
    source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preview_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WallpaperKind {
    Video,
    WebView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperContentSpec {
    pub kind: WallpaperKind,
    pub source: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    monitors: Vec<MonitorConfig>,
}

impl Config {
    pub fn content_for_monitor(&self, index: usize) -> Option<WallpaperContentSpec> {
        let monitor = self.monitors.get(index)?;
        let source = monitor.source.trim();
        if source.is_empty() {
            return None;
        }
        Some(WallpaperContentSpec {
            kind: monitor.kind,
            source: source.to_string(),
        })
    }

    pub fn preview_source_for_monitor(&self, index: usize) -> Option<&str> {
        self.monitors
            .get(index)?
            .preview_source
            .as_deref()
            .map(str::trim)
            .filter(|source| !source.is_empty())
    }

    pub fn set_webview_monitor_source(&mut self, index: usize, source: String, preview_source: Option<String>) {
        self.ensure_monitor_entry(index);
        self.monitors[index] = MonitorConfig {
            kind: WallpaperKind::WebView,
            source,
            preview_source: preview_source.filter(|source| !source.trim().is_empty()),
        };
    }

    pub fn clear_monitor_source(&mut self, index: usize) {
        self.ensure_monitor_entry(index);
        self.monitors[index] = MonitorConfig {
            kind: WallpaperKind::WebView,
            source: String::new(),
            preview_source: None,
        };
    }

    fn ensure_monitor_entry(&mut self, index: usize) {
        while self.monitors.len() <= index {
            self.monitors.push(MonitorConfig {
                kind: WallpaperKind::WebView,
                source: String::new(),
                preview_source: None,
            });
        }
    }

    pub fn load_from_file<P>(path: &P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        if !path.exists() {
            let default_config = Self::default();
            Self::create_default_file(path, &default_config)?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    fn create_default_file(path: &Path, default_config: &Config) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(default_config)?;
        fs::write(path, content).context("Write configuration file failed")
    }

    pub fn config_file_path(_app_name: &str) -> anyhow::Result<PathBuf> {
        let dir = appdata_dir()?.join("yinhaibo").join(_app_name);
        Ok(dir.join("config.toml"))
    }
}

fn appdata_dir() -> anyhow::Result<PathBuf> {
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None)? };
    let ws = unsafe { path.as_wide() };
    let os = OsString::from_wide(ws);
    unsafe { CoTaskMemFree(Some(path.as_ptr() as *mut _)) };
    Ok(PathBuf::from(os))
}

#[cfg(test)]
mod tests {
    use super::{Config, MonitorConfig, WallpaperContentSpec, WallpaperKind};
    use std::{fs, path::PathBuf};

    #[test]
    fn monitor_index_maps_to_content() {
        let config = Config {
            monitors: vec![
                MonitorConfig {
                    kind: WallpaperKind::Video,
                    source: "C:\\videos\\one.mp4".to_string(),
                    preview_source: None,
                },
                MonitorConfig {
                    kind: WallpaperKind::Video,
                    source: "C:\\videos\\two.mp4".to_string(),
                    preview_source: None,
                },
            ],
        };

        assert_eq!(
            config.content_for_monitor(1),
            Some(WallpaperContentSpec {
                kind: WallpaperKind::Video,
                source: "C:\\videos\\two.mp4".to_string(),
            })
        );
    }

    #[test]
    fn empty_monitor_source_is_disabled() {
        let config = Config {
            monitors: vec![MonitorConfig {
                kind: WallpaperKind::Video,
                source: "   ".to_string(),
                preview_source: None,
            }],
        };

        assert_eq!(config.content_for_monitor(0), None);
    }

    #[test]
    fn out_of_range_monitor_content_is_disabled() {
        let config = Config {
            monitors: vec![MonitorConfig {
                kind: WallpaperKind::Video,
                source: "C:\\videos\\one.mp4".to_string(),
                preview_source: None,
            }],
        };

        assert_eq!(config.content_for_monitor(1), None);
    }

    #[test]
    fn load_from_file_reads_toml_config() {
        let path = temp_config_path();
        fs::write(
            &path,
            r#"
[[monitors]]
kind = "video"
source = "C:\\videos\\one.mp4"

[[monitors]]
kind = "video"
source = "C:\\videos\\two.mp4"
"#,
        )
        .unwrap();

        let config = Config::load_from_file(&path).unwrap();

        assert_eq!(
            config.content_for_monitor(0),
            Some(WallpaperContentSpec {
                kind: WallpaperKind::Video,
                source: "C:\\videos\\one.mp4".to_string(),
            })
        );
        assert_eq!(
            config.content_for_monitor(1),
            Some(WallpaperContentSpec {
                kind: WallpaperKind::Video,
                source: "C:\\videos\\two.mp4".to_string(),
            })
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_from_file_reads_optional_preview_source() {
        let path = temp_config_path();
        fs::write(
            &path,
            r#"
[[monitors]]
kind = "webView"
source = "<html></html>"
previewSource = "C:\\videos\\one.mp4"
"#,
        )
        .unwrap();

        let config = Config::load_from_file(&path).unwrap();

        assert_eq!(config.preview_source_for_monitor(0), Some("C:\\videos\\one.mp4"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn set_webview_monitor_source_pads_missing_entries() {
        let mut config = Config::default();

        config.set_webview_monitor_source(
            2,
            "<html></html>".to_string(),
            Some("C:\\videos\\three.mp4".to_string()),
        );

        assert_eq!(config.content_for_monitor(0), None);
        assert_eq!(config.content_for_monitor(1), None);
        assert_eq!(
            config.content_for_monitor(2),
            Some(WallpaperContentSpec {
                kind: WallpaperKind::WebView,
                source: "<html></html>".to_string(),
            })
        );
        assert_eq!(config.preview_source_for_monitor(2), Some("C:\\videos\\three.mp4"));
    }

    #[test]
    fn set_webview_monitor_source_discards_empty_preview_source() {
        let mut config = Config::default();

        config.set_webview_monitor_source(0, "C:\\pages\\index.html".to_string(), Some("   ".to_string()));

        assert_eq!(config.preview_source_for_monitor(0), None);
    }

    #[test]
    fn clear_monitor_source_preserves_index_as_disabled_webview_entry() {
        let mut config = Config {
            monitors: vec![MonitorConfig {
                kind: WallpaperKind::WebView,
                source: "C:\\pages\\index.html".to_string(),
                preview_source: Some("C:\\pages\\index.html".to_string()),
            }],
        };

        config.clear_monitor_source(0);

        assert_eq!(config.content_for_monitor(0), None);
        assert_eq!(config.preview_source_for_monitor(0), None);
        assert_eq!(config.monitors.len(), 1);
        assert_eq!(config.monitors[0].kind, WallpaperKind::WebView);
        assert_eq!(config.monitors[0].source, "");
    }

    #[test]
    fn clear_monitor_source_preserves_entries_after_cleared_index() {
        let mut config = Config {
            monitors: vec![
                MonitorConfig {
                    kind: WallpaperKind::WebView,
                    source: "C:\\pages\\one.html".to_string(),
                    preview_source: Some("C:\\pages\\one.html".to_string()),
                },
                MonitorConfig {
                    kind: WallpaperKind::WebView,
                    source: "C:\\pages\\two.html".to_string(),
                    preview_source: Some("C:\\pages\\two.html".to_string()),
                },
            ],
        };

        config.clear_monitor_source(0);

        assert_eq!(
            config.content_for_monitor(1),
            Some(WallpaperContentSpec {
                kind: WallpaperKind::WebView,
                source: "C:\\pages\\two.html".to_string(),
            })
        );
        assert_eq!(config.preview_source_for_monitor(1), Some("C:\\pages\\two.html"));
        assert_eq!(config.monitors.len(), 2);
    }

    fn temp_config_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "x-desk-config-test-{}.toml",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
