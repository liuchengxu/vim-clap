//! Application state management.

use std::collections::VecDeque;
use std::path::PathBuf;

/// Maximum number of recent files to keep
const MAX_RECENT_FILES: usize = 10;

/// Application state shared across commands.
#[derive(Debug, Default)]
pub struct AppState {
    /// Currently open file path
    pub current_file: Option<PathBuf>,
    /// Recently opened files
    pub recent_files: VecDeque<PathBuf>,
    /// Active file watcher handle
    pub watcher_handle: Option<tokio::task::JoinHandle<()>>,
}

impl AppState {
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
    }
}
