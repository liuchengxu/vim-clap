use clap_search::FileCacheStore;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::Config;

pub struct AppState {
    pub cwd: parking_lot::RwLock<PathBuf>,
    pub config: Config,
    stop_signal: Mutex<Arc<AtomicBool>>,
    file_cache: Arc<FileCacheStore>,
}

impl AppState {
    pub fn new(cwd: PathBuf, config: Config) -> Self {
        Self {
            cwd: parking_lot::RwLock::new(cwd),
            config,
            stop_signal: Mutex::new(Arc::new(AtomicBool::new(false))),
            file_cache: Arc::new(FileCacheStore::new()),
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

    /// Get a clone of the file cache store (for passing into spawned tasks).
    pub fn file_cache(&self) -> Arc<FileCacheStore> {
        self.file_cache.clone()
    }
}
