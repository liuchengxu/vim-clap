//! Application state management with persistence.

use markdown_preview_core::frecency::FrecentItems;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;

/// Maximum number of recent files to keep
const MAX_RECENT_FILES: usize = 20;

/// Maximum number of path history entries to keep
const MAX_PATH_HISTORY: usize = 100;

/// Config file name
const CONFIG_FILE: &str = "config.json";

/// Path history file name
const PATH_HISTORY_FILE: &str = "path_history.json";

/// Persisted configuration data
#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedConfig {
    recent_files: Vec<String>,
}

/// Application state shared across commands.
#[derive(Debug, Default)]
pub struct AppState {
    /// Currently open file path
    pub current_file: Option<PathBuf>,
    /// Recently opened files
    pub recent_files: VecDeque<PathBuf>,
    /// Path input history with frecency scoring
    pub path_history: FrecentItems<String>,
    /// Active file watcher handle
    pub watcher_handle: Option<tokio::task::JoinHandle<()>>,
    /// Path to the config directory for persistence
    config_dir: Option<PathBuf>,
}

impl AppState {
    /// Create a new AppState with the given config directory.
    /// Loads persisted data if available.
    pub fn new(config_dir: Option<PathBuf>) -> Self {
        let mut state = Self {
            current_file: None,
            recent_files: VecDeque::new(),
            path_history: FrecentItems::with_max_entries(MAX_PATH_HISTORY),
            watcher_handle: None,
            config_dir,
        };
        state.load_config();
        state.load_path_history();
        state
    }

    /// Get the config file path.
    fn config_path(&self) -> Option<PathBuf> {
        self.config_dir.as_ref().map(|dir| dir.join(CONFIG_FILE))
    }

    /// Load configuration from disk.
    fn load_config(&mut self) {
        let Some(config_path) = self.config_path() else {
            return;
        };

        if !config_path.exists() {
            tracing::debug!(path = %config_path.display(), "No config file found");
            return;
        }

        match std::fs::read_to_string(&config_path) {
            Ok(content) => match serde_json::from_str::<PersistedConfig>(&content) {
                Ok(config) => {
                    self.recent_files = config
                        .recent_files
                        .into_iter()
                        .map(PathBuf::from)
                        .filter(|p| p.exists())
                        .collect();
                    tracing::info!(
                        count = self.recent_files.len(),
                        "Loaded recent files from config"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse config file");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read config file");
            }
        }
    }

    /// Save configuration to disk.
    fn save_config(&self) {
        let Some(config_path) = self.config_path() else {
            return;
        };

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!(error = %e, "Failed to create config directory");
                return;
            }
        }

        let config = PersistedConfig {
            recent_files: self
                .recent_files
                .iter()
                .filter_map(|p| p.to_str().map(String::from))
                .collect(),
        };

        match serde_json::to_string_pretty(&config) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&config_path, content) {
                    tracing::warn!(error = %e, "Failed to write config file");
                } else {
                    tracing::debug!(path = %config_path.display(), "Saved config");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to serialize config");
            }
        }
    }

    /// Add a file to the recent files list.
    pub fn add_recent_file(&mut self, path: PathBuf) {
        // Remove if already in list
        self.recent_files.retain(|p| p != &path);

        // Add to front
        self.recent_files.push_front(path);

        // Keep only MAX_RECENT_FILES
        while self.recent_files.len() > MAX_RECENT_FILES {
            self.recent_files.pop_back();
        }

        // Persist to disk
        self.save_config();
    }

    /// Get the list of recent files as strings.
    pub fn get_recent_files(&self) -> Vec<String> {
        self.recent_files
            .iter()
            .filter_map(|p| p.to_str().map(String::from))
            .collect()
    }

    /// Clear the recent files list.
    pub fn clear_recent_files(&mut self) {
        self.recent_files.clear();
        self.save_config();
    }

    /// Get the path history file path.
    fn path_history_path(&self) -> Option<PathBuf> {
        self.config_dir
            .as_ref()
            .map(|dir| dir.join(PATH_HISTORY_FILE))
    }

    /// Load path history from disk.
    fn load_path_history(&mut self) {
        let Some(path) = self.path_history_path() else {
            return;
        };

        if !path.exists() {
            tracing::debug!(path = %path.display(), "No path history file found");
            return;
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<FrecentItems<String>>(&content) {
                Ok(mut history) => {
                    // Refresh scores based on current time and filter invalid paths
                    history.refresh_scores();
                    history.retain(|entry| {
                        let path = std::path::Path::new(&entry.item);
                        // Keep if it's a URL or an existing file
                        entry.item.starts_with("http://")
                            || entry.item.starts_with("https://")
                            || path.exists()
                    });
                    self.path_history = history;
                    tracing::info!(count = self.path_history.len(), "Loaded path history");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse path history file");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read path history file");
            }
        }
    }

    /// Save path history to disk.
    fn save_path_history(&self) {
        let Some(path) = self.path_history_path() else {
            return;
        };

        // Ensure config directory exists
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!(error = %e, "Failed to create config directory");
                return;
            }
        }

        match serde_json::to_string_pretty(&self.path_history) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::warn!(error = %e, "Failed to write path history file");
                } else {
                    tracing::debug!(path = %path.display(), "Saved path history");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to serialize path history");
            }
        }
    }

    /// Add a path to the history with frecency tracking.
    pub fn add_path_to_history(&mut self, path: String) {
        self.path_history.upsert(path);
        self.save_path_history();
    }

    /// Get path history sorted by frecency.
    /// If cwd is provided, paths under cwd get a boost.
    pub fn get_path_history(&self, cwd: Option<&str>) -> Vec<String> {
        if let Some(cwd) = cwd {
            self.path_history
                .top_n_with_prefix_boost(MAX_PATH_HISTORY, cwd)
                .into_iter()
                .cloned()
                .collect()
        } else {
            self.path_history
                .top_n(MAX_PATH_HISTORY)
                .into_iter()
                .cloned()
                .collect()
        }
    }
}
