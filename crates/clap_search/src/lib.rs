//! Reusable streaming fuzzy search engine.
//!
//! Provides file search and grep with progressive result emission via the
//! [`SearchSink`] trait. Consumers (Tauri GUI, TUI, CLI) implement the trait
//! to receive results as they're found.

mod file_search;
mod grep_search;
mod types;

pub use file_search::run_file_search;
pub use grep_search::run_grep_search;
pub use types::*;

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use ::types::ClapItem;

/// Trait for receiving progressive search results.
///
/// Implement this for your UI framework (Tauri events, TUI redraw, stdout, etc.)
pub trait SearchSink: Send + Sync + 'static {
    fn emit(&self, payload: SearchPayload);
}

/// Configuration for a search operation.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub cwd: PathBuf,
    pub max_results: usize,
    pub hidden_files: bool,
    pub respect_gitignore: bool,
}

/// Cached file list for a specific directory.
pub struct FileCache {
    cwd: PathBuf,
    items: Vec<Arc<dyn ClapItem>>,
}

/// Thread-safe file cache.
#[derive(Default)]
pub struct FileCacheStore {
    inner: Mutex<Option<FileCache>>,
}

impl FileCacheStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub fn get(&self, cwd: &PathBuf) -> Option<Vec<Arc<dyn ClapItem>>> {
        let cache = self.inner.lock();
        cache
            .as_ref()
            .filter(|c| c.cwd == *cwd)
            .map(|c| c.items.clone())
    }

    pub fn set(&self, cwd: PathBuf, items: Vec<Arc<dyn ClapItem>>) {
        let mut cache = self.inner.lock();
        *cache = Some(FileCache { cwd, items });
    }

    pub fn invalidate(&self) {
        let mut cache = self.inner.lock();
        *cache = None;
    }
}
