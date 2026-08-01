use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(target_os = "windows")]
use crate::win;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MonitorConfig {
    video_url: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    monitors: Vec<MonitorConfig>,
}

impl Config {
    pub fn video_url_for_monitor(&self, index: usize) -> Option<&str> {
        self.monitors
            .get(index)
            .map(|monitor| monitor.video_url.trim())
            .filter(|url| !url.is_empty())
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
        #[cfg(target_os = "windows")]
        let dir = win::appdata_dir()?.join("yinhaibo").join(_app_name);

        #[cfg(not(target_os = "windows"))]
        let dir = current_exe_dir()?;

        Ok(dir.join("config.toml"))
    }
}

#[cfg(not(target_os = "windows"))]
fn current_exe_dir() -> anyhow::Result<PathBuf> {
    let path = std::env::current_exe()?;
    path.parent()
        .map(PathBuf::from)
        .context("Get application directory failed.")
}

#[cfg(test)]
mod tests {
    use super::{Config, MonitorConfig};
    use std::{fs, path::PathBuf};

    #[test]
    fn monitor_index_maps_to_monitor_url() {
        let config = Config {
            monitors: vec![
                MonitorConfig {
                    video_url: "C:\\videos\\one.mp4".to_string(),
                },
                MonitorConfig {
                    video_url: "C:\\videos\\two.mp4".to_string(),
                },
            ],
        };

        assert_eq!(config.video_url_for_monitor(1), Some("C:\\videos\\two.mp4"));
    }

    #[test]
    fn empty_monitor_url_is_disabled() {
        let config = Config {
            monitors: vec![MonitorConfig {
                video_url: "   ".to_string(),
            }],
        };

        assert_eq!(config.video_url_for_monitor(0), None);
    }

    #[test]
    fn out_of_range_monitor_url_is_disabled() {
        let config = Config {
            monitors: vec![MonitorConfig {
                video_url: "C:\\videos\\one.mp4".to_string(),
            }],
        };

        assert_eq!(config.video_url_for_monitor(1), None);
    }

    #[test]
    fn load_from_file_reads_toml_config() {
        let path = temp_config_path();
        fs::write(
            &path,
            r#"
[[monitors]]
video_url = "C:\\videos\\one.mp4"

[[monitors]]
video_url = "C:\\videos\\two.mp4"
"#,
        )
        .unwrap();

        let config = Config::load_from_file(&path).unwrap();

        assert_eq!(config.video_url_for_monitor(0), Some("C:\\videos\\one.mp4"));
        assert_eq!(config.video_url_for_monitor(1), Some("C:\\videos\\two.mp4"));
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
