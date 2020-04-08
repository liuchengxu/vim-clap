use crate::utils::{calculate_hash, clap_cache_dir, remove_dir_contents};
use anyhow::{anyhow, Result};
use std::fs::{read_dir, DirEntry};
use std::path::PathBuf;
use std::time::SystemTime;
use structopt::StructOpt;

#[cfg(target_os = "windows")]
const PATH_SEPERATOR: &str = r"\";

#[cfg(not(target_os = "windows"))]
const PATH_SEPERATOR: &str = "/";

/// List and remove all the cached contents.
#[derive(StructOpt, Debug, Clone)]
pub struct Cache {
    /// List the current cached entries.
    #[structopt(short, long)]
    list: bool,

    /// Purge all the cached contents.
    #[structopt(short, long)]
    purge: bool,
}

impl Cache {
    pub fn run(&self) -> Result<()> {
        let cache_dir = clap_cache_dir();
        let cache_dir_str = cache_dir.clone().into_os_string().into_string().unwrap();
        if self.purge {
            remove_dir_contents(&cache_dir)?;
            println!("Current cache directory {} has been purged", cache_dir_str);
            return Ok(());
        }
        if self.list {
            self.list(cache_dir)?;
        }
        Ok(())
    }

    fn list(&self, cache_dir: PathBuf) -> Result<()> {
        let cache_dir_str = cache_dir.clone().into_os_string().into_string().unwrap();
        println!("Current cache directory:");
        println!("\t{}\n", cache_dir_str);
        if self.list {
            println!("Cached entries:");
            let mut entries = read_dir(cache_dir)?
                .map(|res| {
                    res.map(|e| {
                        e.path()
                            .file_name()
                            .and_then(std::ffi::OsStr::to_str)
                            .map(Into::into)
                            .expect("Couldn't get file name")
                    })
                })
                .collect::<Result<Vec<String>, std::io::Error>>()?;

            entries.sort();

            for fname in entries {
                println!("\t{}{}{}", cache_dir_str, PATH_SEPERATOR, fname);
            }
        }
        Ok(())
    }
}

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
