//! Application state management with persistence.

use markdown_preview_core::frecency::FrecentItems;
use markdown_preview_core::DocumentType;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Maximum number of recent files to keep
const MAX_RECENT_FILES: usize = 20;

/// Maximum number of path history entries to keep
const MAX_PATH_HISTORY: usize = 100;

/// Config file name
const CONFIG_FILE: &str = "config.json";

/// Path history file name
const PATH_HISTORY_FILE: &str = "path_history.json";

/// File snapshots file name
const SNAPSHOTS_FILE: &str = "file_snapshots.json";

/// AI summaries cache file name
const AI_SUMMARIES_FILE: &str = "ai_summaries.json";

/// Maximum number of file snapshots to keep (aligned with recent files)
const MAX_SNAPSHOTS: usize = MAX_RECENT_FILES;

/// A snapshot of a file's content at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    /// The file content at the time of snapshot
    pub content: String,
    /// Unix timestamp in milliseconds when the snapshot was taken
    pub timestamp: u64,
}

/// Storage for file snapshots (persisted to disk).
#[derive(Debug, Default, Serialize, Deserialize)]
struct FileSnapshots {
    /// Map from file path to snapshot
    snapshots: HashMap<String, FileSnapshot>,
}

/// A cached AI summary for a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSummaryEntry {
    /// The generated summary text.
    pub summary: String,
    /// File modification time (Unix millis) when the summary was generated.
    pub file_mtime: u64,
}

/// Persisted AI summaries cache.
#[derive(Debug, Default, Serialize, Deserialize)]
struct AiSummaries {
    /// Map from file path to cached summary.
    summaries: HashMap<String, AiSummaryEntry>,
}

/// Normalize a config value: trim whitespace, convert empty to None.
fn normalize_config_value(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Persisted configuration data
#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedConfig {
    #[serde(default)]
    recent_files: Vec<String>,
    #[serde(default)]
    ai_provider: Option<String>,
    #[serde(default)]
    ai_model: Option<String>,
    #[serde(default)]
    github_token: Option<String>,
    #[serde(default)]
    ai_api_key: Option<String>,
    #[serde(default)]
    ollama_url: Option<String>,
}

/// Application state shared across commands.
pub struct AppState {
    /// Currently open file path
    pub current_file: Option<PathBuf>,
    /// Type of the currently open document (reserved for future use)
    #[allow(dead_code)]
    pub current_document_type: Option<DocumentType>,
    /// Recently opened files
    pub recent_files: VecDeque<PathBuf>,
    /// Path input history with frecency scoring
    pub path_history: FrecentItems<String>,
    /// Active file watcher handle
    pub watcher_handle: Option<tokio::task::JoinHandle<()>>,
    /// File snapshots for diff tracking
    file_snapshots: FileSnapshots,
    /// Path to the config directory for persistence
    config_dir: Option<PathBuf>,
    /// Channel for ordered, non-blocking snapshot writes
    snapshot_writer: Option<mpsc::UnboundedSender<(PathBuf, String)>>,
    /// Configured AI provider name (e.g. "ollama", "anthropic", "openai")
    ai_provider: Option<String>,
    /// Configured AI model override
    ai_model: Option<String>,
    /// Persisted GitHub personal access token
    github_token: Option<String>,
    /// Persisted AI API key (for Anthropic/OpenAI)
    ai_api_key: Option<String>,
    /// Persisted Ollama URL override
    ollama_url: Option<String>,
    /// Cached AI-generated summaries
    ai_summaries: AiSummaries,
}

impl AppState {
    /// Create a new AppState with the given config directory.
    /// Loads persisted data if available.
    pub fn new(config_dir: Option<PathBuf>) -> Self {
        let mut state = Self {
            current_file: None,
            current_document_type: None,
            recent_files: VecDeque::new(),
            path_history: FrecentItems::with_max_entries(MAX_PATH_HISTORY),
            watcher_handle: None,
            file_snapshots: FileSnapshots::default(),
            config_dir,
            snapshot_writer: None,
            ai_provider: None,
            ai_model: None,
            github_token: None,
            ai_api_key: None,
            ollama_url: None,
            ai_summaries: AiSummaries::default(),
        };
        state.load_config();
        state.load_path_history();
        state.load_snapshots();
        state.load_ai_summaries();
        state
    }

    /// Set the snapshot writer channel for non-blocking persistence.
    ///
    /// Must be called after the Tokio runtime is available. The receiving end
    /// should be driven by a background task that writes snapshots sequentially.
    pub fn set_snapshot_writer(&mut self, tx: mpsc::UnboundedSender<(PathBuf, String)>) {
        self.snapshot_writer = Some(tx);
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
                    self.ai_provider = config.ai_provider;
                    self.ai_model = config.ai_model;
                    self.github_token = config.github_token;
                    self.ai_api_key = config.ai_api_key;
                    self.ollama_url = config.ollama_url;
                    tracing::info!(
                        count = self.recent_files.len(),
                        ai_provider = ?self.ai_provider,
                        "Loaded config"
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
            ai_provider: self.ai_provider.clone(),
            ai_model: self.ai_model.clone(),
            github_token: self.github_token.clone(),
            ai_api_key: self.ai_api_key.clone(),
            ollama_url: self.ollama_url.clone(),
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
    ///
    /// If the file is already in the list, its position is preserved (no reordering).
    /// New files are added to the front.
    pub fn add_recent_file(&mut self, path: PathBuf) {
        // Don't reorder if already in list
        if self.recent_files.contains(&path) {
            return;
        }

        // Add new file to front
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

    /// Remove a specific file from the recent files list.
    pub fn remove_recent_file(&mut self, path: &std::path::Path) {
        self.recent_files.retain(|p| p != path);
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

    /// Get the snapshots file path.
    fn snapshots_path(&self) -> Option<PathBuf> {
        self.config_dir.as_ref().map(|dir| dir.join(SNAPSHOTS_FILE))
    }

    /// Load file snapshots from disk.
    fn load_snapshots(&mut self) {
        let Some(path) = self.snapshots_path() else {
            return;
        };

        if !path.exists() {
            tracing::debug!(path = %path.display(), "No snapshots file found");
            return;
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<FileSnapshots>(&content) {
                Ok(snapshots) => {
                    self.file_snapshots = snapshots;
                    tracing::info!(
                        count = self.file_snapshots.snapshots.len(),
                        "Loaded file snapshots"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse snapshots file");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read snapshots file");
            }
        }
    }

    /// Save file snapshots to disk.
    ///
    /// If a snapshot writer channel is available, serializes in-place and sends
    /// to the background writer task (non-blocking, ordered). Falls back to
    /// synchronous write if no writer is configured.
    fn save_snapshots(&self) {
        let Some(path) = self.snapshots_path() else {
            return;
        };

        let content = match serde_json::to_string_pretty(&self.file_snapshots) {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to serialize snapshots");
                return;
            }
        };

        // Use background writer if available (non-blocking, ordered)
        if let Some(ref tx) = self.snapshot_writer {
            if tx.send((path, content)).is_err() {
                tracing::warn!("Snapshot writer channel closed");
            }
            return;
        }

        // Fallback: synchronous write (used before runtime is ready)
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!(error = %e, "Failed to create config directory");
                return;
            }
        }
        if let Err(e) = std::fs::write(&path, &content) {
            tracing::warn!(error = %e, "Failed to write snapshots file");
        } else {
            tracing::debug!(path = %path.display(), "Saved file snapshots");
        }
    }

    /// Get the snapshot for a file path.
    pub fn get_snapshot(&self, path: &str) -> Option<&FileSnapshot> {
        self.file_snapshots.snapshots.get(path)
    }

    /// Save a snapshot for a file path.
    /// Enforces the maximum snapshot limit by removing the oldest entries.
    pub fn save_snapshot(&mut self, path: &str, content: &str) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        self.file_snapshots.snapshots.insert(
            path.to_string(),
            FileSnapshot {
                content: content.to_string(),
                timestamp,
            },
        );

        // Enforce max snapshots limit by removing oldest entries
        while self.file_snapshots.snapshots.len() > MAX_SNAPSHOTS {
            // Find the oldest snapshot
            if let Some(oldest_path) = self
                .file_snapshots
                .snapshots
                .iter()
                .min_by_key(|(_, snap)| snap.timestamp)
                .map(|(path, _)| path.clone())
            {
                self.file_snapshots.snapshots.remove(&oldest_path);
            } else {
                break;
            }
        }

        self.save_snapshots();
    }

    /// Get the configured AI provider name.
    pub fn ai_provider(&self) -> Option<&str> {
        self.ai_provider.as_deref()
    }

    /// Get the configured AI model override.
    pub fn ai_model(&self) -> Option<&str> {
        self.ai_model.as_deref()
    }

    /// Get the persisted GitHub token.
    pub fn github_token(&self) -> Option<&str> {
        self.github_token.as_deref()
    }

    /// Get the persisted AI API key.
    pub fn ai_api_key(&self) -> Option<&str> {
        self.ai_api_key.as_deref()
    }

    /// Get the persisted Ollama URL override.
    pub fn ollama_url(&self) -> Option<&str> {
        self.ollama_url.as_deref()
    }

    /// Set the AI provider and model only (legacy, merge semantics).
    ///
    /// Preserves `github_token`, `ai_api_key`, `ollama_url`. Normalizes inputs.
    /// Clears AI summary cache if provider or model changed.
    /// Returns `true` if AI config changed.
    pub fn set_ai_config(&mut self, provider: Option<String>, model: Option<String>) -> bool {
        let provider = normalize_config_value(provider);
        let model = normalize_config_value(model);
        let changed = self.ai_provider != provider || self.ai_model != model;
        if changed {
            self.clear_ai_summaries();
        }
        self.ai_provider = provider;
        self.ai_model = model;
        self.save_config();
        changed
    }

    /// Set all user-configurable settings, persisting to config.
    ///
    /// Normalizes all inputs (trim + empty→None). Clears AI summary cache
    /// if any AI-related field changed. Returns `true` if AI config changed.
    pub fn set_app_config(
        &mut self,
        ai_provider: Option<String>,
        ai_model: Option<String>,
        ai_api_key: Option<String>,
        ollama_url: Option<String>,
        github_token: Option<String>,
    ) -> bool {
        let ai_provider = normalize_config_value(ai_provider);
        let ai_model = normalize_config_value(ai_model);
        let ai_api_key = normalize_config_value(ai_api_key);
        let ollama_url = normalize_config_value(ollama_url);
        let github_token = normalize_config_value(github_token);

        let ai_changed = self.ai_provider != ai_provider
            || self.ai_model != ai_model
            || self.ai_api_key != ai_api_key
            || self.ollama_url != ollama_url;

        if ai_changed {
            self.clear_ai_summaries();
        }

        self.ai_provider = ai_provider;
        self.ai_model = ai_model;
        self.ai_api_key = ai_api_key;
        self.ollama_url = ollama_url;
        self.github_token = github_token;
        self.save_config();
        ai_changed
    }

    /// Clear all cached AI summaries and persist.
    pub fn clear_ai_summaries(&mut self) {
        self.ai_summaries.summaries.clear();
        self.save_ai_summaries();
    }

    /// Get the config directory path.
    pub fn config_dir(&self) -> Option<&PathBuf> {
        self.config_dir.as_ref()
    }

    /// Get the AI summaries file path.
    fn ai_summaries_path(&self) -> Option<PathBuf> {
        self.config_dir
            .as_ref()
            .map(|dir| dir.join(AI_SUMMARIES_FILE))
    }

    /// Load AI summaries cache from disk.
    fn load_ai_summaries(&mut self) {
        let Some(path) = self.ai_summaries_path() else {
            return;
        };

        if !path.exists() {
            return;
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<AiSummaries>(&content) {
                Ok(summaries) => {
                    tracing::info!(
                        count = summaries.summaries.len(),
                        "Loaded AI summaries cache"
                    );
                    self.ai_summaries = summaries;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse AI summaries file");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read AI summaries file");
            }
        }
    }

    /// Save AI summaries cache to disk.
    fn save_ai_summaries(&self) {
        let Some(path) = self.ai_summaries_path() else {
            return;
        };

        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!(error = %e, "Failed to create config directory");
                return;
            }
        }

        match serde_json::to_string_pretty(&self.ai_summaries) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    tracing::warn!(error = %e, "Failed to write AI summaries file");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to serialize AI summaries");
            }
        }
    }

    /// Get a cached AI summary for a file path if the mtime matches.
    pub fn get_ai_summary(&self, path: &str, current_mtime: u64) -> Option<&str> {
        self.ai_summaries
            .summaries
            .get(path)
            .filter(|entry| entry.file_mtime == current_mtime)
            .map(|entry| entry.summary.as_str())
    }

    /// Store an AI summary for a file path and persist to disk.
    pub fn set_ai_summary(&mut self, path: String, summary: String, file_mtime: u64) {
        self.ai_summaries.summaries.insert(
            path,
            AiSummaryEntry {
                summary,
                file_mtime,
            },
        );
        self.save_ai_summaries();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_config_value() {
        assert_eq!(normalize_config_value(None), None);
        assert_eq!(normalize_config_value(Some(String::new())), None);
        assert_eq!(normalize_config_value(Some("   ".to_string())), None);
        assert_eq!(normalize_config_value(Some("\t\n".to_string())), None);
        assert_eq!(
            normalize_config_value(Some("  hello  ".to_string())),
            Some("hello".to_string())
        );
        assert_eq!(
            normalize_config_value(Some("value".to_string())),
            Some("value".to_string())
        );
    }

    #[test]
    fn test_set_app_config_normalizes_values() {
        let mut state = AppState::new(None);
        state.set_app_config(
            Some("  ollama  ".to_string()),
            Some("".to_string()),
            Some("   ".to_string()),
            Some("http://localhost:11434".to_string()),
            Some("\t".to_string()),
        );
        assert_eq!(state.ai_provider(), Some("ollama"));
        assert_eq!(state.ai_model(), None);
        assert_eq!(state.ai_api_key(), None);
        assert_eq!(state.ollama_url(), Some("http://localhost:11434"));
        assert_eq!(state.github_token(), None);
    }

    #[test]
    fn test_set_app_config_detects_ai_change() {
        let mut state = AppState::new(None);
        let changed = state.set_app_config(Some("ollama".to_string()), None, None, None, None);
        assert!(changed);

        // Same values again — no change
        let changed = state.set_app_config(Some("ollama".to_string()), None, None, None, None);
        assert!(!changed);

        // Change model — detects change
        let changed = state.set_app_config(
            Some("ollama".to_string()),
            Some("llama3".to_string()),
            None,
            None,
            None,
        );
        assert!(changed);
    }

    #[test]
    fn test_set_app_config_clears_summaries_on_ai_change() {
        let mut state = AppState::new(None);
        state.set_ai_summary("test.md".to_string(), "summary".to_string(), 100);
        assert!(state.get_ai_summary("test.md", 100).is_some());

        // Change AI config — summaries should be cleared
        state.set_app_config(
            Some("anthropic".to_string()),
            None,
            Some("key".to_string()),
            None,
            None,
        );
        assert!(state.get_ai_summary("test.md", 100).is_none());
    }

    #[test]
    fn test_set_ai_config_merge_semantics() {
        let mut state = AppState::new(None);
        state.set_app_config(
            Some("ollama".to_string()),
            None,
            Some("my-key".to_string()),
            Some("http://custom:1234".to_string()),
            Some("ghp_token".to_string()),
        );

        // Legacy set_ai_config only changes provider/model
        state.set_ai_config(Some("anthropic".to_string()), Some("claude-3".to_string()));

        assert_eq!(state.ai_provider(), Some("anthropic"));
        assert_eq!(state.ai_model(), Some("claude-3"));
        // Other fields preserved
        assert_eq!(state.ai_api_key(), Some("my-key"));
        assert_eq!(state.ollama_url(), Some("http://custom:1234"));
        assert_eq!(state.github_token(), Some("ghp_token"));
    }

    #[test]
    fn test_set_ai_config_clears_cache_on_change() {
        let mut state = AppState::new(None);
        state.set_app_config(Some("ollama".to_string()), None, None, None, None);
        state.set_ai_summary("file.md".to_string(), "old summary".to_string(), 42);
        assert!(state.get_ai_summary("file.md", 42).is_some());

        let changed = state.set_ai_config(Some("anthropic".to_string()), None);
        assert!(changed);
        assert!(state.get_ai_summary("file.md", 42).is_none());
    }

    #[test]
    fn test_set_ai_config_no_change_preserves_cache() {
        let mut state = AppState::new(None);
        state.set_app_config(Some("ollama".to_string()), None, None, None, None);
        state.set_ai_summary("file.md".to_string(), "cached".to_string(), 42);

        let changed = state.set_ai_config(Some("ollama".to_string()), None);
        assert!(!changed);
        assert_eq!(state.get_ai_summary("file.md", 42), Some("cached"));
    }

    #[test]
    fn test_persisted_config_backward_compat() {
        // Old config without new fields should deserialize fine
        let json = r#"{"recent_files": ["/tmp/test.md"]}"#;
        let config: PersistedConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.recent_files, vec!["/tmp/test.md"]);
        assert!(config.ai_provider.is_none());
        assert!(config.github_token.is_none());
        assert!(config.ai_api_key.is_none());
        assert!(config.ollama_url.is_none());
    }
}
