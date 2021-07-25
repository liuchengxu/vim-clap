//! This module provides the feature of persistent data store via file system.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::cache::{CacheInfo, MAX_DIGESTS};
use crate::recent_files::SortedRecentFiles;
use crate::utils::{generate_data_file_path, load_json};

// TODO: use mmap?

const CACHE_FILENAME: &str = "cache.json";

pub static CACHE_JSON_PATH: Lazy<Option<PathBuf>> =
    Lazy::new(|| generate_data_file_path(CACHE_FILENAME).ok());

pub static CACHE_INFO_IN_MEMORY: Lazy<Arc<Mutex<CacheInfo>>> = Lazy::new(|| {
    let maybe_persistent = load_json::<CacheInfo, _>(CACHE_JSON_PATH.as_deref())
        .unwrap_or_else(|| CacheInfo::with_capacity(MAX_DIGESTS));
    Arc::new(Mutex::new(maybe_persistent))
});

const RECENT_FILES_FILENAME: &str = "recent_files.json";

pub static RECENT_FILES_JSON_PATH: Lazy<Option<PathBuf>> =
    Lazy::new(|| generate_data_file_path(RECENT_FILES_FILENAME).ok());

pub static RECENT_FILES_IN_MEMORY: Lazy<Mutex<SortedRecentFiles>> = Lazy::new(|| {
    let maybe_persistent = load_json(RECENT_FILES_JSON_PATH.as_deref())
        .map(|f: SortedRecentFiles| f.remove_invalid_entries())
        .unwrap_or_default();
    Mutex::new(maybe_persistent)
});

pub fn store_cache_info(cache_info: &CacheInfo) -> Result<()> {
    crate::utils::write_json(cache_info, CACHE_JSON_PATH.as_ref())
}

pub fn store_recent_files(recent_files: &SortedRecentFiles) -> Result<()> {
    crate::utils::write_json(&recent_files, RECENT_FILES_JSON_PATH.as_ref())
}
