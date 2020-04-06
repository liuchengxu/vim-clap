use crate::utils::clap_cache_dir;
use anyhow::Result;
use std::fs::read_dir;

pub fn run(list: bool) -> Result<()> {
    let cache_dir = clap_cache_dir();
    println!("Current cache directory: {:?}", cache_dir);
    if list {
        let mut entries = read_dir(cache_dir)?
            .map(|res| {
                res.map(|e| {
                    e.path()
                        .file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .map(Into::into)
                        .unwrap()
                })
            })
            .collect::<Result<Vec<String>, std::io::Error>>()?;

        entries.sort();

        println!("Cached entries: {:?}", entries);
    }
    Ok(())
}
