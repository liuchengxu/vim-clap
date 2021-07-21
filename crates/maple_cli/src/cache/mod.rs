mod old;

pub use self::old::*;

use std::fs::{DirEntry, File};
use std::hash::Hash;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::{anyhow, Result};
use chrono::prelude::*;
use once_cell::sync::Lazy;

use icon::IconPainter;
use utility::{
    calculate_hash, clap_cache_dir, get_cached_entry, println_json, println_json_with_length,
    read_first_lines, remove_dir_contents,
};

type UtcTime = DateTime<Utc>;

/// Digest of cached info about a command.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheDigest {
    /// Raw shell command string.
    pub command: String,
    /// Working directory of command.
    ///
    /// The same command with different cwd normally has
    /// different results, thus we need to record the cwd too.
    pub cwd: PathBuf,
    /// Time of last execution.
    pub last_run: UtcTime,
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
            last_run: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheInfo(Vec<CacheDigest>);

impl Default for CacheInfo {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl CacheInfo {
    pub fn cache_digest(&self, command: &str, cwd: &PathBuf) -> Option<&CacheDigest> {
        self.0
            .iter()
            .find(|d| d.command == command && &d.cwd == cwd)
    }

    pub fn add(&mut self, cache_digest: CacheDigest) -> Result<()> {
        self.0.push(cache_digest);
        self.write_to_disk()?;
        Ok(())
    }

    fn write_to_disk(&self) -> Result<()> {
        if let Some(recent_files_json) = JSON_PATH.as_deref() {
            // Overwrite it.
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(recent_files_json)?;

            f.write_all(serde_json::to_string(self)?.as_bytes())?;
            f.flush()?;
        }
        Ok(())
    }
}

const CACHE_FILENAME: &str = "cache.json";

fn persistent_cache_info_path() -> Result<PathBuf> {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "vim", "Vim Clap") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

        let mut recent_files_json = data_dir.to_path_buf();
        recent_files_json.push(CACHE_FILENAME);

        return Ok(recent_files_json);
    }

    Err(anyhow!("Couldn't create the Vim Clap project directory"))
}

pub static JSON_PATH: Lazy<Option<PathBuf>> = Lazy::new(|| persistent_cache_info_path().ok());

fn read_cache_info_from_file<P: AsRef<Path>>(path: P) -> Result<CacheInfo> {
    use std::io::BufReader;

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let cache_infos = serde_json::from_reader(reader)?;
    Ok(cache_infos)
}

pub static CACHE_INFO_IN_MEMORY: Lazy<Mutex<CacheInfo>> =
    Lazy::new(|| Mutex::new(initialize_cache_info()));

fn initialize_cache_info() -> CacheInfo {
    JSON_PATH
        .as_deref()
        .and_then(|cache_json| {
            if cache_json.exists() {
                read_cache_info_from_file(cache_json).ok()
            } else {
                None
            }
        })
        .unwrap_or_default()
}

pub struct RawCommand(String);

impl RawCommand {
    pub fn cache_exists(&self, command: &str, cwd: &PathBuf) -> Option<PathBuf> {
        let cache_info = CACHE_INFO_IN_MEMORY.lock().unwrap();
        cache_info
            .cache_digest(command, cwd)
            .map(|d| d.cached_path.clone())
    }
}
