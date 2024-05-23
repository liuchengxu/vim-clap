//! This module provides the feature of persistent data store via file system.
//! Typically, the info will be persisted in the json format.

use crate::cache::CacheInfo;
use crate::recent_files::SortedRecentFiles;
use crate::stdio_server::InputHistory;
use dirs::Dirs;
use once_cell::sync::Lazy;
use parking_lot::{Mutex, RwLock};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Linux: ~/.local/share/vimclap/cache.json
static CACHE_METADATA_PATH: Lazy<Option<PathBuf>> =
    Lazy::new(|| generate_data_file_path("cache.json").ok());

pub static CACHE_INFO_IN_MEMORY: Lazy<Arc<Mutex<CacheInfo>>> = Lazy::new(|| {
    let mut maybe_persistent =
        load_json::<CacheInfo, _>(CACHE_METADATA_PATH.as_deref()).unwrap_or_else(CacheInfo::new);
    maybe_persistent.remove_invalid_and_old_entries();
    Arc::new(Mutex::new(maybe_persistent))
});

/// Linux: ~/.local/share/vimclap/recent_files.json
static RECENT_FILES_JSON_PATH: Lazy<Option<PathBuf>> =
    Lazy::new(|| generate_data_file_path("recent_files.json").ok());

/// Linux: ~/.local/share/vimclap/input_history.json
static INPUT_HISTORY_JSON_PATH: Lazy<Option<PathBuf>> =
    Lazy::new(|| generate_data_file_path("input_history.json").ok());

pub static RECENT_FILES_IN_MEMORY: Lazy<RwLock<SortedRecentFiles>> = Lazy::new(|| {
    let maybe_persistent = load_json(RECENT_FILES_JSON_PATH.as_deref())
        .map(|f: SortedRecentFiles| f.remove_invalid_entries())
        .unwrap_or_default();
    RwLock::new(maybe_persistent)
});

pub static INPUT_HISTORY_IN_MEMORY: Lazy<Arc<Mutex<InputHistory>>> = Lazy::new(|| {
    Arc::new(Mutex::new(
        load_json(INPUT_HISTORY_JSON_PATH.as_deref()).unwrap_or_else(InputHistory::new),
    ))
});

/// Synchronize the latest state of cache info to the disk.
pub fn store_cache_info(cache_info: &CacheInfo) -> std::io::Result<()> {
    write_json(cache_info, CACHE_METADATA_PATH.as_ref())
}

/// Synchronize the latest state of recent files to the disk.
pub fn store_recent_files(recent_files: &SortedRecentFiles) -> std::io::Result<()> {
    write_json(recent_files, RECENT_FILES_JSON_PATH.as_ref())
}

/// Synchronize the latest state of input history to the disk.
pub fn store_input_history(input_history: &InputHistory) -> std::io::Result<()> {
    write_json(input_history, INPUT_HISTORY_JSON_PATH.as_ref())
}

/// Returns the path of `cache.json`.
///
/// Used by maple_cli to inspect the local cache state.
pub fn cache_metadata_path() -> Option<&'static PathBuf> {
    CACHE_METADATA_PATH.as_ref()
}

/// Returns a `PathBuf` using given file name under the project data directory.
pub fn generate_data_file_path(filename: &str) -> std::io::Result<PathBuf> {
    let data_dir = Dirs::project().data_dir();
    std::fs::create_dir_all(data_dir)?;
    Ok(data_dir.join(filename))
}

/// Returns a `PathBuf` using given file name under the project cache directory.
pub fn generate_cache_file_path(filename: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let cache_dir = Dirs::project().cache_dir();
    std::fs::create_dir_all(cache_dir)?;
    Ok(cache_dir.join(filename))
}

fn read_json_as<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> std::io::Result<T> {
    let file = std::fs::File::open(&path)?;
    let reader = BufReader::new(&file);
    let deserializd = serde_json::from_reader(reader).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to write {} as json: {e:?}", path.as_ref().display()),
        )
    })?;

    Ok(deserializd)
}

fn load_json<T: DeserializeOwned, P: AsRef<Path>>(path: Option<P>) -> Option<T> {
    path.and_then(|json_path| {
        if json_path.as_ref().exists() {
            read_json_as::<_, T>(json_path).ok()
        } else {
            None
        }
    })
}

fn write_json<T: Serialize, P: AsRef<Path>>(obj: T, path: Option<P>) -> std::io::Result<()> {
    if let Some(json_path) = path.as_ref() {
        utils::create_or_overwrite(json_path, serde_json::to_string(&obj)?.as_bytes())?;
    }

    Ok(())
}
