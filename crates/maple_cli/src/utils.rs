use anyhow::{anyhow, Result};
use std::collections::hash_map::DefaultHasher;
use std::fs::{read_dir, remove_dir_all, remove_file, DirEntry, File};
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

/// Removes all the file and directories under `target_dir`.
pub fn remove_dir_contents(target_dir: &PathBuf) -> Result<()> {
    let entries = read_dir(target_dir)?;
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();

            if path.is_dir() {
                remove_dir_all(path)?;
            } else {
                remove_file(path)?;
            }
        };
    }
    Ok(())
}

/// Returns true if the `dir` is a git repo, including git submodule.
pub fn is_git_repo(dir: &Path) -> bool {
    let mut gitdir = dir.to_owned();
    gitdir.push(".git");
    gitdir.exists()
}

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
pub fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

/// Returns the first number lines given the file path.
pub fn read_first_lines<P: AsRef<Path>>(
    filename: P,
    number: usize,
) -> io::Result<impl Iterator<Item = String>> {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file)
        .lines()
        .filter_map(|i| i.ok())
        .take(number))
}

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[inline]
pub(crate) fn clap_cache_dir() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push("vim.clap");
    dir
}

/// Returns the cache path for clap.
///
/// The reason for using hash(cmd_dir) instead of cmd_dir directory is to avoid the possible issue
/// of using a path as the directory name.
///
/// Formula: temp_dir + clap_cache + arg1_arg2_arg3 + hash(cmd_dir)
pub fn get_cache_dir(args: &[&str], cmd_dir: &PathBuf) -> PathBuf {
    let mut dir = clap_cache_dir();
    dir.push(args.join("_"));
    // TODO: use a readable cache cmd_dir name?
    dir.push(format!("{}", calculate_hash(&cmd_dir)));
    dir
}

/// Returns the cached entry given the cmd args and working dir.
pub fn get_cached_entry(args: &[&str], cmd_dir: &PathBuf) -> Result<DirEntry> {
    let cache_dir = get_cache_dir(args, &cmd_dir);
    if cache_dir.exists() {
        let mut entries = read_dir(cache_dir)?;

        // TODO: get latest modifed cache file?
        if let Some(Ok(first_entry)) = entries.next() {
            return Ok(first_entry);
        }
    }

    Err(anyhow!(
        "Couldn't get the cached entry for {:?} {:?}",
        args,
        cmd_dir
    ))
}
