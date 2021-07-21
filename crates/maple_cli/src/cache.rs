use std::fs::{read_dir, DirEntry, File};
use std::hash::Hash;
use std::io::Write;
use std::sync::Mutex;
use std::path::{Path, PathBuf};
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
    pub cwd: PathBuf,
    /// Time of last execution.
    pub last_run: UtcTime,
    /// Number of results from last run.
    pub results_number: u64,
    /// File saved for caching the results.
    pub cached_path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheInfo(Vec<CacheDigest>);

impl Default for CacheInfo {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl CacheInfo {
    pub fn cache_digest(&self, command: &str) -> Option<&CacheDigest> {
        self.0.iter().find(|d| d.command == command)
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
    pub fn cache_exists(&self, command: &str) -> Option<PathBuf> {
        let cache_info = CACHE_INFO_IN_MEMORY.lock().unwrap();
        cache_info
            .cache_digest(command)
            .map(|d| d.cached_path.clone())
    }
}

pub struct CacheEntry;

impl CacheEntry {
    /// Construct the cache entry given command arguments and its working directory, the `total`
    /// info is cached in the file name.
    pub fn try_new<T: AsRef<Path> + Hash>(
        cmd_args: &[&str],
        cmd_dir: Option<T>,
        total: usize,
    ) -> Result<PathBuf> {
        let mut dir = clap_cache_dir();
        dir.push(cmd_args.join("_"));
        if let Some(cmd_dir) = cmd_dir {
            dir.push(format!("{}", calculate_hash(&cmd_dir.as_ref())));
        } else {
            dir.push("no_cmd_dir");
        }
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
        dir.push(format!(
            "{}_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
            total
        ));
        Ok(dir)
    }

    /// Write the `contents` to given cache entry.
    ///
    /// Remove all the existing old entries if there are any.
    pub fn write<T: AsRef<[u8]>>(entry: &Path, contents: T) -> Result<()> {
        // Remove the other outdated cache file if there are any.
        //
        // There should be only one cache file in parent_dir at this moment.
        if let Some(parent_dir) = entry.parent() {
            remove_dir_contents(&parent_dir.to_path_buf())?;
        }

        File::create(entry)?.write_all(contents.as_ref())?;

        Ok(())
    }

    /// Creates a new cache entry.
    pub fn create<T: AsRef<[u8]>, P: AsRef<Path> + Hash>(
        cmd_args: &[&str],
        cmd_dir: Option<P>,
        total: usize,
        contents: T,
    ) -> Result<PathBuf> {
        let entry = Self::try_new(cmd_args, cmd_dir, total)?;
        Self::write(&entry, contents)?;
        Ok(entry)
    }

    /// Get the total number of this cache entry from its file name.
    pub fn get_total(cached_entry: &DirEntry) -> Result<usize> {
        if let Some(path_str) = cached_entry.file_name().to_str() {
            let info = path_str.split('_').collect::<Vec<_>>();
            if info.len() == 2 {
                info[1].parse().map_err(Into::into)
            } else {
                Err(anyhow!("Invalid cache entry name: {:?}", info))
            }
        } else {
            Err(anyhow!("Couldn't get total from cached entry"))
        }
    }
}

#[derive(Debug, Clone)]
pub enum SendResponse {
    Json,
    JsonWithContentLength,
}

/// Reads the first lines from cache file and send back the cached info.
pub fn send_response_from_cache(
    tempfile: &Path,
    total: usize,
    response_ty: SendResponse,
    icon_painter: Option<IconPainter>,
) {
    let using_cache = true;
    if let Ok(lines_iter) = read_first_lines(&tempfile, 100) {
        let lines: Vec<String> = if let Some(painter) = icon_painter {
            lines_iter.map(|x| painter.paint(&x)).collect()
        } else {
            lines_iter.collect()
        };
        match response_ty {
            SendResponse::Json => println_json!(total, tempfile, using_cache, lines),
            SendResponse::JsonWithContentLength => {
                println_json_with_length!(total, tempfile, using_cache, lines)
            }
        }
    } else {
        match response_ty {
            SendResponse::Json => println_json!(total, tempfile, using_cache),
            SendResponse::JsonWithContentLength => {
                println_json_with_length!(total, tempfile, using_cache)
            }
        }
    }
}

/// Returns the cache file path and number of total cached items.
pub fn cache_exists(args: &[&str], cmd_dir: &Path) -> Result<(PathBuf, usize)> {
    if let Ok(cached_entry) = get_cached_entry(args, cmd_dir) {
        if let Ok(total) = CacheEntry::get_total(&cached_entry) {
            let tempfile = cached_entry.path();
            return Ok((tempfile, total));
        }
    }
    Err(anyhow!(
        "Cache does not exist for: {:?} in {:?}",
        args,
        cmd_dir
    ))
}
