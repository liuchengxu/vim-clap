use crate::utils::clap_cache_dir;
use anyhow::Result;
use std::fs::read_dir;

#[cfg(target_os = "windows")]
const PATH_SEPERATOR: &str = r"\";

#[cfg(not(target_os = "windows"))]
const PATH_SEPERATOR: &str = "/";

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
