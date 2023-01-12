use crate::dirs::PROJECT_DIRS;
use anyhow::Result;
use chrono::prelude::*;
use icon::Icon;
use std::io::{BufRead, BufReader, Lines};
use std::path::{Path, PathBuf};
use subprocess::Exec;
use utility::{println_json, println_json_with_length, read_first_lines};

pub type UtcTime = DateTime<Utc>;

/// Returns a `PathBuf` using given file name under the project data directory.
pub fn generate_data_file_path(filename: &str) -> std::io::Result<PathBuf> {
    let data_dir = PROJECT_DIRS.data_dir();
    std::fs::create_dir_all(data_dir)?;

    let mut file = data_dir.to_path_buf();
    file.push(filename);

    Ok(file)
}

/// Returns a `PathBuf` using given file name under the project cache directory.
pub fn generate_cache_file_path(filename: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let cache_dir = PROJECT_DIRS.cache_dir();
    std::fs::create_dir_all(cache_dir)?;

    let mut file = cache_dir.to_path_buf();
    file.push(filename);

    Ok(file)
}

fn read_json_as<P: AsRef<Path>, T: serde::de::DeserializeOwned>(path: P) -> Result<T> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let deserializd = serde_json::from_reader(reader)?;

    Ok(deserializd)
}

pub fn load_json<T: serde::de::DeserializeOwned, P: AsRef<Path>>(path: Option<P>) -> Option<T> {
    path.and_then(|json_path| {
        if json_path.as_ref().exists() {
            read_json_as::<_, T>(json_path).ok()
        } else {
            None
        }
    })
}

pub fn write_json<T: serde::Serialize, P: AsRef<Path>>(
    obj: T,
    path: Option<P>,
) -> std::io::Result<()> {
    if let Some(json_path) = path.as_ref() {
        utility::create_or_overwrite(json_path, serde_json::to_string(&obj)?.as_bytes())?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub enum SendResponse {
    Json,
    JsonWithContentLength,
}

/// Reads the first lines from cache file and send back the cached info.
pub fn send_response_from_cache(
    tempfile: &Path,
    total: usize,
    response_ty: SendResponse,
    icon: Icon,
) {
    let using_cache = true;
    if let Ok(iter) = read_first_lines(&tempfile, 100) {
        let lines: Vec<String> = if let Some(icon_kind) = icon.icon_kind() {
            iter.map(|x| icon_kind.add_icon_to_text(x)).collect()
        } else {
            iter.collect()
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

/// Counts lines in the source `handle`.
///
/// # Examples
/// ```ignore
/// let lines: usize = count_lines(std::fs::File::open("Cargo.toml").unwrap()).unwrap();
/// ```
///
/// Credit: https://github.com/eclarke/linecount/blob/master/src/lib.rs
pub fn count_lines<R: std::io::Read>(handle: R) -> std::io::Result<usize> {
    let mut reader = std::io::BufReader::with_capacity(1024 * 32, handle);
    let mut count = 0;
    loop {
        let len = {
            let buf = reader.fill_buf()?;
            if buf.is_empty() {
                break;
            }
            count += bytecount::count(buf, b'\n');
            buf.len()
        };
        reader.consume(len);
    }

    Ok(count)
}

#[inline]
pub fn lines(cmd: Exec) -> Result<Lines<impl BufRead>> {
    // We usually have a decent amount of RAM nowdays.
    Ok(std::io::BufReader::with_capacity(8 * 1024 * 1024, cmd.stream_stdout()?).lines())
}

/// Returns the width of displaying `n` on the screen.
///
/// Same with `n.to_string().len()` but without allocation.
pub fn display_width(n: usize) -> usize {
    if n == 0 {
        return 1;
    }

    let mut n = n;
    let mut len = 0;
    while n > 0 {
        len += 1;
        n /= 10;
    }

    len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_lines() {
        let f: &[u8] = b"some text\nwith\nfour\nlines\n";
        assert_eq!(count_lines(f).unwrap(), 4);
    }
}
