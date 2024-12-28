use anyhow::Result;
use clap::{Parser, Subcommand};
use maple_core::datastore::CACHE_INFO_IN_MEMORY;
use maple_core::dirs::Dirs;
use std::fs::read_dir;
use std::io::Write;
use std::path::{PathBuf, MAIN_SEPARATOR};
use utils::io::remove_dir_contents;

/// List and remove all the cached contents.
#[derive(Subcommand, Debug, Clone)]
pub enum Cache {
    List(List),
    Purge(Purge),
}

impl Cache {
    pub fn run(&self) -> Result<()> {
        match self {
            Self::List(list) => list.run(),
            Self::Purge(purge) => purge.run(),
        }
    }
}

#[derive(Parser, Debug, Clone)]
pub struct List {
    /// Display all the cached info, including the current cached entries.
    #[clap(long)]
    all: bool,
}

impl List {
    fn run(&self) -> Result<()> {
        let cache_dir = Dirs::clap_cache_dir()?;
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();

        let cache_dir_display = cache_dir.display();
        writeln!(lock, "Current cache directory:")?;
        writeln!(lock, "\t{cache_dir_display}\n")?;

        let cache_info = CACHE_INFO_IN_MEMORY.lock();
        let mut digests = cache_info.to_digests();
        digests.sort_unstable_by_key(|digest| digest.total);
        writeln!(lock, "{digests:#?}\n")?;

        if self.all {
            writeln!(lock, "Cached entries:")?;
            let mut entries = read_dir(&cache_dir)?
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
                writeln!(lock, "\t{cache_dir_display}{MAIN_SEPARATOR}{fname}")?;
            }
        }

        Ok(())
    }
}

#[derive(Parser, Debug, Clone)]
pub struct Purge {
    /// Purge all the cached contents.
    #[clap(long)]
    all: bool,
}

impl Purge {
    fn run(&self) -> Result<()> {
        let cache_dir = Dirs::clap_cache_dir()?;

        if let Ok(cache_size) = dir_size(&cache_dir) {
            let readable_size = if cache_size > 1024 * 1024 {
                format!("{}MB", cache_size / 1024 / 1024)
            } else if cache_size > 1024 {
                format!("{}KB", cache_size / 1024)
            } else {
                format!("{cache_size}B")
            };
            println!("Cache size: {readable_size:?}");
        }

        if let Some(f) = maple_core::datastore::cache_metadata_path() {
            match std::fs::remove_file(f) {
                Ok(()) => println!("Cache metadata {} has been deleted", f.display()),
                Err(e) => println!("Faild to delete {}: {e}", f.display()),
            }
        }

        remove_dir_contents(&cache_dir)?;

        println!(
            "Current cache directory {} has been purged sucessfully",
            cache_dir.display()
        );

        Ok(())
    }
}

// The cache directory is not huge and pretty deep, hence the recursive version is acceptable.
fn dir_size(path: impl Into<PathBuf>) -> std::io::Result<u64> {
    fn dir_size(mut dir: std::fs::ReadDir) -> std::io::Result<u64> {
        dir.try_fold(0, |acc, file| {
            let file = file?;
            let size = match file.metadata()? {
                data if data.is_dir() => dir_size(std::fs::read_dir(file.path())?)?,
                data => data.len(),
            };
            Ok(acc + size)
        })
    }

    dir_size(read_dir(path.into())?)
}
