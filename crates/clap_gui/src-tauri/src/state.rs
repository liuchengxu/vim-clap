use parking_lot::{Mutex, RwLock};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::Config;

pub struct AppState {
    pub cwd: RwLock<PathBuf>,
    pub config: Config,
    stop_signal: Mutex<Arc<AtomicBool>>,
}

impl AppState {
    pub fn new(cwd: PathBuf, config: Config) -> Self {
        Self {
            cwd: RwLock::new(cwd),
            config,
            stop_signal: Mutex::new(Arc::new(AtomicBool::new(false))),
        }
    }

    /// Cancel any in-flight search and return a fresh stop signal for the new one.
    pub fn new_search(&self) -> Arc<AtomicBool> {
        let mut signal = self.stop_signal.lock();
        // Tell any in-flight search to stop
        signal.store(true, Ordering::Relaxed);
        // Create a fresh signal for the new search
        let new_signal = Arc::new(AtomicBool::new(false));
        *signal = new_signal.clone();
        new_signal
    }
}
