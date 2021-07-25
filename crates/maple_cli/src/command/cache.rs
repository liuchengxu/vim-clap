use std::fs::read_dir;
use std::io::Write;
use std::path::{self, Path};

use anyhow::Result;
use structopt::StructOpt;

use utility::{clap_cache_dir, remove_dir_contents};

use crate::datastore::CACHE_INFO_IN_MEMORY;

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
        let cache_dir = clap_cache_dir()?;
        if self.purge {
            if let Some(f) = crate::datastore::CACHE_JSON_PATH.as_deref() {
                std::fs::remove_file(f)?;
                println!("Cache metadata {} has been deleted", f.display());
            }
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

    fn list(&self, cache_dir: &Path) -> Result<()> {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();

        let cache_dir_str = cache_dir.display();
        writeln!(lock, "Current cache directory:")?;
        writeln!(lock, "\t{}\n", cache_dir_str)?;

        let cache_info = CACHE_INFO_IN_MEMORY.lock();
        writeln!(lock, "{:#?}\n", cache_info)?;

        if self.list {
            writeln!(lock, "Cached entries:")?;
            let mut entries = read_dir(cache_dir)?
                .map(|res| {
                    res.map(|e| {
                        e.path()
                            .file_name()
                            .and_then(std::ffi::OsStr::to_str)
                            .map(Into::into)
                            .unwrap_or_else(|| panic!("Couldn't get file name from {:?}", e.path()))
                    })
                })
                .collect::<Result<Vec<String>, std::io::Error>>()?;

            entries.sort();

            for fname in entries {
                writeln!(lock, "\t{}{}{}", cache_dir_str, path::MAIN_SEPARATOR, fname)?;
            }
        }
        Ok(())
    }
}
