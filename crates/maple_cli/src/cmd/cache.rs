use anyhow::{anyhow, Result};
use icon::IconPainter;
use std::fs::{read_dir, DirEntry, File};
use std::io::Write;
use std::path::{self, PathBuf};
use std::time::SystemTime;
use structopt::StructOpt;
use utility::{
    calculate_hash, clap_cache_dir, get_cached_entry, read_first_lines, remove_dir_contents,
};

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
        if self.purge {
            remove_dir_contents(&cache_dir)?;
            println!(
                "Current cache directory {} has been purged",
                cache_dir.display()
            );
            return Ok(());
        }
        if self.list {
            self.list(&cache_dir)?;
        }
        Ok(())
    }

    fn list(&self, cache_dir: &PathBuf) -> Result<()> {
        let cache_dir_str = cache_dir.display();
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
                println!("\t{}{}{}", cache_dir_str, path::MAIN_SEPARATOR, fname);
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

    /// Write the `contents` to given cache entry.
    ///
    /// Remove all the existing old entries if there are any.
    pub fn write<T: AsRef<[u8]>>(entry: &PathBuf, contents: T) -> Result<()> {
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
    pub fn create<T: AsRef<[u8]>>(
        cmd_args: &[&str],
        cmd_dir: Option<PathBuf>,
        total: usize,
        contents: T,
    ) -> Result<PathBuf> {
        let entry = Self::new(cmd_args, cmd_dir, total)?;
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
    tempfile: &PathBuf,
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
                print_json_with_length!(total, tempfile, using_cache, lines)
            }
        }
    } else {
        match response_ty {
            SendResponse::Json => println_json!(total, tempfile, using_cache),
            SendResponse::JsonWithContentLength => {
                print_json_with_length!(total, tempfile, using_cache)
            }
        }
    }
}

/// Returns the cache file path and number of total cached items.
pub fn cache_exists(args: &[&str], cmd_dir: &PathBuf) -> Result<(PathBuf, usize)> {
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
