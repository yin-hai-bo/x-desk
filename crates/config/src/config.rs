use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString, fs, os::windows::ffi::OsStringExt, path::{Path, PathBuf},
};
use windows::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitorConfig {
    kind: WallpaperKind,
    source: String,
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
                },
                MonitorConfig {
                    kind: WallpaperKind::Video,
                    source: "C:\\videos\\two.mp4".to_string(),
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
