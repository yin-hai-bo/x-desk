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
    url: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    monitors: Vec<MonitorConfig>,
}

impl Config {
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
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    fn create_default_file(path: &Path, default_config: &Config) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(default_config)?;
        fs::write(path, content).context("Write configuration file failed")
    }

    pub fn config_file_path(_app_name: &str) -> anyhow::Result<PathBuf> {
        #[cfg(target_os = "windows")]
        let dir = win::appdata_dir()?.join("yinhaibo").join(_app_name);

        #[cfg(not(target_os = "windows"))]
        let dir = current_exe_dir()?;

        Ok(dir.join("config.json"))
    }
}

#[cfg(not(target_os = "windows"))]
fn current_exe_dir() -> anyhow::Result<PathBuf> {
    let path = std::env::current_exe()?;
    path.parent()
        .map(PathBuf::from)
        .context("Get application directory failed.")
}
