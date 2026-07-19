use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MonitorConfig {
    url: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    monitors: Vec<MonitorConfig>,
}

impl Config {
    pub fn load_from_file<P>(path: P) -> io::Result<Self>
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

    fn create_default_file(path: &Path, default_config: &Config) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(default_config)?;
        fs::write(path, content)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::{fs, io, path::PathBuf};

    #[test]
    fn load_from_file_creates_missing_default_config() -> io::Result<()> {
        let path = unique_test_path();
        let config = Config::load_from_file(&path)?;

        assert!(path.exists());
        assert!(config.monitors.is_empty());

        fs::remove_file(&path)?;
        fs::remove_dir_all(path.parent().unwrap())?;
        Ok(())
    }

    fn unique_test_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("x-desk-tests-{}", std::process::id()))
            .join("config.json")
    }
}
