mod old;

pub use self::old::{send_response_from_cache, SendResponse};

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Result;
use chrono::prelude::*;
use once_cell::sync::Lazy;

use crate::process::BaseCommand;
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
    /// Base command.
    #[serde(flatten)]
    pub base: BaseCommand,
    /// Time of last execution.
    pub execution_time: UtcTime,
    /// Time of last visit.
    pub last_visit: UtcTime,
    /// Number of results from last execution.
    pub total: usize,
    /// File saved for caching the results.
    pub cached_path: PathBuf,
}

impl Digest {
    pub fn new(base: BaseCommand, total: usize, cached_path: PathBuf) -> Self {
        let now = Utc::now();
        Self {
            base,
            total,
            cached_path,
            last_visit: now,
            execution_time: now,
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
    pub fn find_digest(&self, base_cmd: &BaseCommand) -> Option<&Digest> {
        self.0.iter().find(|d| &d.base == base_cmd)
    }

    pub fn add(&mut self, digest: Digest) -> Result<()> {
        self.0.push(digest);
        crate::utils::write_json(self, CACHE_JSON_PATH.as_ref())?;
        Ok(())
    }
}

pub fn add_new_cache_digest(digest: Digest) -> Result<()> {
    let mut cache_info = CACHE_INFO_IN_MEMORY.lock().unwrap();
    cache_info.add(digest)?;
    Ok(())
}

pub fn get_cached(base_cmd: &BaseCommand) -> Option<(usize, PathBuf)> {
    let cache_info = CACHE_INFO_IN_MEMORY.lock().unwrap();
    cache_info
        .find_digest(base_cmd)
        .map(|d| (d.total, d.cached_path.clone()))
}

/// Writes the whole stdout of `base_cmd` to a cache file.
fn write_stdout_to_disk(base_cmd: &BaseCommand, cmd_stdout: &[u8]) -> Result<PathBuf> {
    use std::io::Write;

    let cached_filename = utility::calculate_hash(base_cmd);
    let cached_path = crate::utils::generate_cache_file_path(&cached_filename.to_string())?;

    std::fs::File::create(&cached_path)?.write_all(cmd_stdout)?;

    Ok(cached_path)
}

/// Caches the output into a tempfile and also writes the cache digest to the disk.
pub fn create_cache(
    base_cmd: BaseCommand,
    total: usize,
    cmd_stdout: &[u8],
) -> Result<(String, PathBuf)> {
    let cache_file = write_stdout_to_disk(&base_cmd, cmd_stdout)?;

    let digest = Digest::new(base_cmd, total, cache_file.clone());

    add_new_cache_digest(digest)?;

    Ok((
        // lines used for displaying directly.
        // &cmd_output.stdout[..nth_newline_index]
        String::from_utf8_lossy(cmd_stdout).into(),
        cache_file,
    ))
}
