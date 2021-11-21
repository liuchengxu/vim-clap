//! Various utility functions for caching and file management.

use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::fs::{self, read_dir, remove_dir_all, remove_file, File};
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{anyhow, Result};

use self::bytelines::ByteLines;

pub mod bytelines;
mod macros;

/// Removes all the file and directories under `target_dir`.
pub fn remove_dir_contents<P: AsRef<Path>>(target_dir: P) -> Result<()> {
    let entries = read_dir(target_dir)?;
    for entry in entries.into_iter().flatten() {
        let path = entry.path();

        if path.is_dir() {
            remove_dir_all(path)?;
        } else {
            remove_file(path)?;
        }
    }
    Ok(())
}

/// Returns true if the `dir` is a git repo, including git submodule.
pub fn is_git_repo(dir: &Path) -> bool {
    let mut gitdir = dir.to_owned();
    gitdir.push(".git");
    gitdir.exists()
}

/// Returns an Iterator to the Reader of the lines of the file.
///
/// The output is wrapped in a Result to allow matching on errors.
pub fn read_lines<P>(path: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(path)?;
    Ok(io::BufReader::new(file).lines())
}

/// Returns the first number lines given the file path.
pub fn read_first_lines<P: AsRef<Path>>(
    path: P,
    number: usize,
) -> io::Result<impl Iterator<Item = String>> {
    let file = File::open(path)?;
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
pub fn clap_cache_dir() -> Result<PathBuf> {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "vim", "Vim Clap") {
        let cache_dir = proj_dirs.cache_dir();
        std::fs::create_dir_all(cache_dir)?;

        Ok(cache_dir.to_path_buf())
    } else {
        Err(anyhow!("Couldn't create Vim Clap project directory"))
    }
}

/// Works for utf-8 lines only.
#[allow(unused)]
fn read_preview_lines_utf8<P: AsRef<Path>>(
    path: P,
    target_line: usize,
    size: usize,
) -> io::Result<(impl Iterator<Item = String>, usize)> {
    let file = File::open(path)?;
    let (start, end, hl_line) = if target_line > size {
        (target_line - size, target_line + size, size)
    } else {
        (0, 2 * size, target_line)
    };
    Ok((
        io::BufReader::new(file)
            .lines()
            .skip(start)
            .filter_map(|l| l.ok())
            .take(end - start),
        hl_line,
    ))
}

/// Returns the lines of (`target_line` - `size`, `target_line` - `size`) given the path.
pub fn read_preview_lines<P: AsRef<Path>>(
    path: P,
    target_line: usize,
    size: usize,
) -> io::Result<(Vec<String>, usize)> {
    read_preview_lines_impl(path, target_line, size)
}

// Copypasted from stdlib.
/// Indicates how large a buffer to pre-allocate before reading the entire file.
fn initial_buffer_size(file: &fs::File) -> usize {
    // Allocate one extra byte so the buffer doesn't need to grow before the
    // final `read` call at the end of the file.  Don't worry about `usize`
    // overflow because reading will fail regardless in that case.
    file.metadata().map(|m| m.len() as usize + 1).unwrap_or(0)
}

fn read_preview_lines_impl<P: AsRef<Path>>(
    path: P,
    target_line: usize,
    size: usize,
) -> io::Result<(Vec<String>, usize)> {
    let (start, end, hl_line) = if target_line > size {
        (target_line - size, target_line + size, size)
    } else {
        (0, 2 * size, target_line)
    };

    let mut filebuf: Vec<u8> = Vec::new();

    File::open(path)
        .and_then(|mut file| {
            //x XXX: is megabyte enough for any text file?
            const MEGABYTE: usize = 100 * 1_048_576;

            let filesize = initial_buffer_size(&file);
            if filesize > MEGABYTE {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "maximum preview file buffer size reached",
                ));
            }

            filebuf.reserve_exact(filesize);
            file.read_to_end(&mut filebuf)
        })
        .map(|_| {
            (
                ByteLines::new(&filebuf)
                    .into_iter()
                    .skip(start)
                    .take(end - start)
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>(),
                hl_line,
            )
        })
}

/// Returns an iterator of limited lines of `filename` from the line number `start_line`.
pub fn read_lines_from<P: AsRef<Path>>(
    path: P,
    start_line: usize,
    size: usize,
) -> io::Result<impl Iterator<Item = String>> {
    let file = File::open(path)?;
    Ok(io::BufReader::new(file)
        .lines()
        .skip(start_line)
        .filter_map(|i| i.ok())
        .take(size))
}

/// Converts `shell_cmd` to `Command` with optional working directory.
pub fn as_std_command<P: AsRef<Path>>(shell_cmd: impl AsRef<OsStr>, dir: Option<P>) -> Command {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(shell_cmd.as_ref());
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(shell_cmd.as_ref());
        cmd
    };

    if let Some(d) = dir {
        cmd.current_dir(d);
    }

    cmd
}

/// Executes the `shell_cmd` and returns the output.
pub fn execute_at<S, P>(shell_cmd: S, dir: Option<P>) -> Result<Output>
where
    S: AsRef<OsStr>,
    P: AsRef<Path>,
{
    let mut cmd = as_std_command(shell_cmd, dir);
    Ok(cmd.output()?)
}

/// Attempts to write an entire buffer into the file.
///
/// Creates one if the file doed not exist.
pub fn create_or_overwrite<P: AsRef<Path>>(path: P, buf: &[u8]) -> Result<()> {
    use std::io::Write;

    // Overwrite it.
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    f.write_all(buf)?;
    f.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_byte_reading() {
        let mut current_dir = std::env::current_dir().unwrap();
        current_dir.push("test_673.txt");
        let (lines, _hl_line) = read_preview_lines_impl(current_dir, 2, 5).unwrap();
        assert_eq!(
            lines,
            [
                "test_ddd",
                "test_ddd    //1����ˤ��ϡ�����1",
                "test_ddd    //2����ˤ��ϡ�����2",
                "test_ddd    //3����ˤ��ϡ�����3",
                "test_ddd    //hello"
            ]
        );
    }
}
