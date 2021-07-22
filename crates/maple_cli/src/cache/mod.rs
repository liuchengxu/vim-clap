mod old;

pub use self::old::*;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use chrono::prelude::*;
use once_cell::sync::Lazy;

use crate::utils::{generate_data_file_path, load_json, UtcTime};

const CACHE_FILENAME: &str = "cache.json";

pub static CACHE_JSON_PATH: Lazy<Option<PathBuf>> =
    Lazy::new(|| generate_data_file_path(CACHE_FILENAME).ok());

pub static CACHE_INFO_IN_MEMORY: Lazy<Mutex<CacheInfo>> = Lazy::new(|| {
    let maybe_persistent =
        load_json::<CacheInfo, _>(CACHE_JSON_PATH.as_deref()).unwrap_or_default();
    Mutex::new(maybe_persistent)
});

/// Digest of cached info about a command.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Digest {
    /// Raw shell command string.
    pub command: String,
    /// Working directory of command.
    ///
    /// The same command with different cwd normally has
    /// different results, thus we need to record the cwd too.
    pub cwd: PathBuf,
    /// Time of last execution.
    pub execution_time: UtcTime,
    /// Number of results from last run.
    pub results_number: u64,
    /// File saved for caching the results.
    pub cached_path: PathBuf,
}

impl CacheDigest {
    pub fn new(command: String, cwd: PathBuf, results_number: u64, cached_path: PathBuf) -> Self {
        Self {
            command,
            cwd,
            results_number,
            cached_path,
            execution_time: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheInfo(Vec<Digest>);

impl Default for CacheInfo {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl CacheInfo {
    pub fn cache_digest(&self, command: &str, cwd: &PathBuf) -> Option<&Digest> {
        self.0
            .iter()
            .find(|d| d.command == command && &d.cwd == cwd)
    }

    pub fn add(&mut self, cache_digest: Digest) -> Result<()> {
        self.0.push(cache_digest);
        self.write_to_disk()?;
        Ok(())
    }

    fn write_to_disk(&self) -> Result<()> {
        crate::utils::write_json(self, CACHE_JSON_PATH.as_ref())
    }
}

pub fn add_new_cache_digest(digest: Digest) -> Result<()> {
    let mut cache_info = CACHE_INFO_IN_MEMORY.lock().unwrap();
    cache_info.add(digest)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct BaseCommand {
    pub command: String,
    pub cwd: PathBuf,
}

impl BaseCommand {
    pub fn new(command: String, cwd: PathBuf) -> Self {
        Self { command, cwd }
    }

    pub fn cache_exists(&self) -> Option<Digest> {
        let cache_info = CACHE_INFO_IN_MEMORY.lock().unwrap();
        cache_info.cache_digest(&self.command, &self.cwd).cloned()
    }
}
