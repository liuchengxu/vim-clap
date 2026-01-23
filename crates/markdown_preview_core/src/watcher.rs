//! File watching abstraction for detecting markdown file changes.
//!
//! Provides both inotify-based (when available) and polling-based file watching
//! with automatic fallback.

use notify::{Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tokio::sync::watch;

/// Events emitted by the file watcher.
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// File was modified
    Modified(PathBuf),
    /// File was created (for editors that use write-rename)
    Created(PathBuf),
    /// Watcher error occurred
    Error(String),
}

/// Configuration for the file watcher.
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Polling interval in milliseconds for fallback polling mode
    pub poll_interval_ms: u64,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1000,
        }
    }
}

/// A file watcher that monitors changes to a specific file.
///
/// Automatically falls back to polling if inotify is unavailable.
pub struct FileWatcher {
    /// Receiver for watch events
    event_rx: watch::Receiver<WatchEvent>,
    /// Channel to signal shutdown
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// The file path being watched
    file_path: PathBuf,
}

impl FileWatcher {
    /// Create a new file watcher for the given path.
    ///
    /// Attempts to use inotify-based watching first, falling back to polling
    /// if that fails.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to watch
    /// * `config` - Watcher configuration
    pub fn new(path: &Path, config: WatcherConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let file_path = path.to_path_buf();

        // Try inotify-based watcher first
        match Self::try_inotify_watcher(&file_path) {
            Ok((event_rx, shutdown_tx)) => {
                tracing::info!(path = ?file_path, "Started inotify-based file watcher");
                Ok(Self {
                    event_rx,
                    shutdown_tx: Some(shutdown_tx),
                    file_path,
                })
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    path = ?file_path,
                    "inotify file watcher failed, falling back to polling"
                );
                let (event_rx, shutdown_tx) = Self::spawn_polling_watcher(&file_path, config)?;
                Ok(Self {
                    event_rx,
                    shutdown_tx: Some(shutdown_tx),
                    file_path,
                })
            }
        }
    }

    /// Subscribe to watch events.
    ///
    /// Returns a receiver that can be used to receive events.
    pub fn subscribe(&self) -> watch::Receiver<WatchEvent> {
        self.event_rx.clone()
    }

    /// Get the path being watched.
    pub fn path(&self) -> &Path {
        &self.file_path
    }

    fn try_inotify_watcher(
        file_path: &Path,
    ) -> Result<(watch::Receiver<WatchEvent>, mpsc::Sender<()>), Box<dyn std::error::Error + Send + Sync>> {
        let (event_tx, event_rx) = watch::channel(WatchEvent::Modified(file_path.to_path_buf()));
        let (notify_tx, notify_rx) = mpsc::channel::<()>();
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();

        // Get the parent directory to watch
        let (watch_target, file_name) = if let (Some(parent), Some(name)) =
            (file_path.parent(), file_path.file_name())
        {
            (parent.to_path_buf(), name.to_os_string())
        } else {
            return Err("Invalid file path".into());
        };

        let file_name_for_filter = file_name.clone();
        let file_path_for_event = file_path.to_path_buf();

        // Spawn the watcher thread
        std::thread::spawn(move || {
            let mut watcher = match RecommendedWatcher::new(
                move |res: Result<NotifyEvent, notify::Error>| {
                    match res {
                        Ok(event) => {
                            // Filter events to only our target file
                            let is_target_file = event
                                .paths
                                .iter()
                                .any(|p| p.file_name() == Some(&file_name_for_filter));

                            if !is_target_file {
                                return;
                            }

                            if event.kind.is_modify()
                                || event.kind.is_create()
                                || event.kind.is_remove()
                            {
                                let _ = notify_tx.send(());
                            }
                        }
                        Err(e) => {
                            tracing::error!(?e, "File watcher error");
                        }
                    }
                },
                notify::Config::default(),
            ) {
                Ok(w) => w,
                Err(err) => {
                    let _ = started_tx.send(Err(err.to_string()));
                    return;
                }
            };

            if let Err(err) = watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
                let _ = started_tx.send(Err(err.to_string()));
                return;
            }

            let _ = started_tx.send(Ok(()));

            // Keep the watcher alive until shutdown
            loop {
                match shutdown_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        });

        // Bridge thread to send events
        let file_path_for_bridge = file_path_for_event.clone();
        std::thread::spawn(move || {
            while notify_rx.recv().is_ok() {
                let _ = event_tx.send(WatchEvent::Modified(file_path_for_bridge.clone()));
            }
        });

        // Wait for watcher to start
        match started_rx.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(Ok(())) => Ok((event_rx, shutdown_tx)),
            Ok(Err(err)) => Err(err.into()),
            Err(_) => Err("Watcher startup timeout".into()),
        }
    }

    fn spawn_polling_watcher(
        file_path: &Path,
        config: WatcherConfig,
    ) -> Result<(watch::Receiver<WatchEvent>, mpsc::Sender<()>), Box<dyn std::error::Error + Send + Sync>> {
        let (event_tx, event_rx) = watch::channel(WatchEvent::Modified(file_path.to_path_buf()));
        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        let file_path = file_path.to_path_buf();
        let poll_interval = std::time::Duration::from_millis(config.poll_interval_ms);

        tokio::spawn(async move {
            let mut last_mtime = std::fs::metadata(&file_path)
                .and_then(|m| m.modified())
                .ok();

            tracing::info!(
                path = ?file_path,
                poll_interval_ms = config.poll_interval_ms,
                "Started polling-based file watcher"
            );

            loop {
                // Check for shutdown
                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                tokio::time::sleep(poll_interval).await;

                if let Ok(metadata) = std::fs::metadata(&file_path) {
                    if let Ok(current_mtime) = metadata.modified() {
                        if let Some(last) = last_mtime {
                            if current_mtime > last {
                                let _ = event_tx.send(WatchEvent::Modified(file_path.clone()));
                                last_mtime = Some(current_mtime);
                            }
                        } else {
                            last_mtime = Some(current_mtime);
                        }
                    }
                }
            }
        });

        Ok((event_rx, shutdown_tx))
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        assert_eq!(config.poll_interval_ms, 1000);
    }
}
