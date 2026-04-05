use parking_lot::{Mutex, RwLock};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use types::ClapItem;

use crate::config::Config;

/// Cached file list for a specific directory.
pub struct FileCache {
    cwd: PathBuf,
    items: Vec<Arc<dyn ClapItem>>,
}

pub struct AppState {
    pub cwd: RwLock<PathBuf>,
    pub config: Config,
    stop_signal: Mutex<Arc<AtomicBool>>,
    file_cache: Mutex<Option<FileCache>>,
}

impl AppState {
    pub fn new(cwd: PathBuf, config: Config) -> Self {
        Self {
            cwd: RwLock::new(cwd),
            config,
            stop_signal: Mutex::new(Arc::new(AtomicBool::new(false))),
            file_cache: Mutex::new(None),
        }
    }

    /// Cancel any in-flight search and return a fresh stop signal for the new one.
    pub fn new_search(&self) -> Arc<AtomicBool> {
        let mut signal = self.stop_signal.lock();
        signal.store(true, Ordering::Relaxed);
        let new_signal = Arc::new(AtomicBool::new(false));
        *signal = new_signal.clone();
        new_signal
    }

    /// Get cached file items if cache is valid for the given cwd, otherwise None.
    pub fn get_cached_files(&self, cwd: &PathBuf) -> Option<Vec<Arc<dyn ClapItem>>> {
        let cache = self.file_cache.lock();
        cache
            .as_ref()
            .filter(|c| c.cwd == *cwd)
            .map(|c| c.items.clone())
    }

    /// Store file items in the cache for a given cwd.
    pub fn set_cached_files(&self, cwd: PathBuf, items: Vec<Arc<dyn ClapItem>>) {
        let mut cache = self.file_cache.lock();
        *cache = Some(FileCache { cwd, items });
    }

    /// Invalidate the file cache (e.g. when user wants a refresh).
    pub fn invalidate_file_cache(&self) {
        let mut cache = self.file_cache.lock();
        *cache = None;
    }
}
