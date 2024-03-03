use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::Sender;

const DEBOUNCE_DELAY: Duration = Duration::from_millis(10);

/// The fallback for `RecommendedWatcher` polling.
const FALLBACK_POLLING_TIMEOUT: Duration = Duration::from_secs(1);

pub fn watch(sender: Sender<()>) {
    let config_file = crate::config_file();

    // Exclude char devices like `/dev/null`, sockets, and so on, by checking that file type is a
    // regular file.
    //
    // Call `metadata` to resolve symbolic links.
    if !config_file
        .metadata()
        .map_or(false, |metadata| metadata.file_type().is_file())
    {
        return;
    }

    let path = match config_file.canonicalize() {
        Ok(canonical_path) => match config_file.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => canonical_path,
            _ => config_file.clone(),
        },
        _ => return,
    };

    // Canonicalize paths, keeping the base paths for symlinks.
    // for i in 0..paths.len() {
    // if let Ok(canonical_path) = paths[i].canonicalize() {
    // match paths[i].symlink_metadata() {
    // Ok(metadata) if metadata.file_type().is_symlink() => paths.push(canonical_path),
    // _ => paths[i] = canonical_path,
    // }
    // }
    // }

    // The Duration argument is a debouncing period.
    let (tx, rx) = mpsc::channel();
    let mut watcher = match RecommendedWatcher::new(
        tx,
        Config::default().with_poll_interval(FALLBACK_POLLING_TIMEOUT),
    ) {
        Ok(watcher) => watcher,
        Err(err) => {
            tracing::error!("Unable to watch config file: {err}");
            return;
        }
    };

    std::thread::Builder::new()
        .name("config-watcher".into())
        .spawn({
            move || {
                // Watch the configuration file.
                if let Err(err) = watcher.watch(&path, RecursiveMode::NonRecursive) {
                    tracing::debug!("Unable to watch config file {:?}: {err}", path);
                }

                // The current debouncing time.
                let mut debouncing_deadline: Option<Instant> = None;

                // The events accumulated during the debounce period.
                let mut received_events = Vec::new();

                loop {
                    // We use `recv_timeout` to debounce the events coming from the watcher and reduce
                    // the amount of config reloads.
                    let event = match debouncing_deadline.as_ref() {
                        Some(debouncing_deadline) => rx.recv_timeout(
                            debouncing_deadline.saturating_duration_since(Instant::now()),
                        ),
                        None => {
                            let event = rx.recv().map_err(Into::into);

                            // Set the debouncing deadline after receiving the event.
                            debouncing_deadline.replace(Instant::now() + DEBOUNCE_DELAY);

                            event
                        }
                    };

                    match event {
                        Ok(Ok(event)) => match event.kind {
                            EventKind::Any
                            | EventKind::Create(_)
                            | EventKind::Modify(_)
                            | EventKind::Other => {
                                received_events.push(event);
                            }
                            _ => (),
                        },
                        Err(RecvTimeoutError::Timeout) => {
                            // Go back to polling the events.
                            debouncing_deadline = None;

                            if received_events
                                .drain(..)
                                .flat_map(|event| event.paths.into_iter())
                                .any(|modified_path| modified_path.eq(&path))
                            {
                                crate::reload_config(path.clone());
                                // Always reload the primary configuration file.
                                let _ = sender.try_send(());
                            }
                        }
                        Ok(Err(err)) => {
                            tracing::debug!("Config watcher errors: {err:?}");
                        }
                        Err(err) => {
                            tracing::debug!("Config watcher channel dropped unexpectedly: {err}");
                            break;
                        }
                    };
                }
            }
        })
        .expect("Failed to spawn config-watcher thread");
}
