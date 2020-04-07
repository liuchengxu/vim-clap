use crate::utils::{calculate_hash, clap_cache_dir};
use anyhow::{anyhow, Result};
use std::fs::{read_dir, DirEntry};
use std::path::PathBuf;
use std::time::SystemTime;

#[cfg(target_os = "windows")]
const PATH_SEPERATOR: &str = r"\";

#[cfg(not(target_os = "windows"))]
const PATH_SEPERATOR: &str = "/";

pub struct CacheEntry;

impl CacheEntry {
    /// Construct the cache entry given command arguments and its working directory, the `total`
    /// info is cached in the file name.
    pub fn new(cmd_args: &[&str], cmd_dir: Option<PathBuf>, total: usize) -> Result<PathBuf> {
        let mut dir = clap_cache_dir();
        dir.push(cmd_args.join("_"));
        if let Some(mut cmd_dir) = cmd_dir {
            dir.push(format!("{}", calculate_hash(&mut cmd_dir)));
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

pub fn run(list_entries: bool) -> Result<()> {
    let cache_dir = clap_cache_dir();
    let cache_dir_str = cache_dir.clone().into_os_string().into_string().unwrap();
    println!("Current cache directory:");
    println!("\t{}", cache_dir_str);
    if list_entries {
        println!("Cached entries:");
        let mut entries = read_dir(cache_dir)?
            .map(|res| {
                res.map(|e| {
                    let fname: String = e
                        .path()
                        .file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .map(Into::into)
                        .expect("Couldn't get file name");
                    format!("{}{}{}", cache_dir_str, PATH_SEPERATOR, fname)
                })
            })
            .collect::<Result<Vec<String>, std::io::Error>>()?;

        entries.sort();

        for entry in entries {
            println!("\t{}", entry);
        }
    }
    Ok(())
}
