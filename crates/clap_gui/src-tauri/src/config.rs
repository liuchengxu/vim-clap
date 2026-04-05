use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub window: WindowConfig,
    pub search: SearchConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct GeneralConfig {
    pub editor: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub hotkey: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct SearchConfig {
    pub max_results: usize,
    pub hidden_files: bool,
    pub respect_gitignore: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            window: WindowConfig::default(),
            search: SearchConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self { editor: None }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 500,
            hotkey: "CmdOrCtrl+Shift+P".to_string(),
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results: 50,
            hidden_files: false,
            respect_gitignore: true,
        }
    }
}

pub fn load_config() -> Config {
    let config_path = config_path();
    match std::fs::read_to_string(&config_path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

fn config_path() -> PathBuf {
    directories::ProjectDirs::from("com", "vimclap", "clap-gui")
        .map(|dirs| dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("~/.config/clap-gui/config.toml"))
}
