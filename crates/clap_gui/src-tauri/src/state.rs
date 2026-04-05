use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::Config;

pub struct AppState {
    pub cwd: RwLock<PathBuf>,
    pub config: Config,
    stop_signal: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(cwd: PathBuf, config: Config) -> Self {
        Self {
            cwd: RwLock::new(cwd),
            config,
            stop_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancel any in-flight search and return a new stop signal for the next one.
    pub fn new_search(&self) -> Arc<AtomicBool> {
        self.stop_signal.store(true, Ordering::Relaxed);
        Arc::new(AtomicBool::new(false))
    }

    pub fn stop_signal(&self) -> Arc<AtomicBool> {
        self.stop_signal.clone()
    }
}
