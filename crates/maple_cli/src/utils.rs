use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chrono::prelude::*;
use once_cell::sync::Lazy;

use icon::Icon;
use types::{ExactTerm, InverseTerm};
use utility::{println_json, println_json_with_length, read_first_lines};

/// Yes or no terms.
#[derive(Debug, Clone)]
pub struct ExactOrInverseTerms {
    pub exact_terms: Vec<ExactTerm>,
    pub inverse_terms: Vec<InverseTerm>,
}

impl Default for ExactOrInverseTerms {
    fn default() -> Self {
        Self {
            exact_terms: Vec::new(),
            inverse_terms: Vec::new(),
        }
    }
}

impl ExactOrInverseTerms {
    /// Returns the match indices of exact terms if given `line` passes all the checks.
    fn check_terms(&self, line: &str) -> Option<Vec<usize>> {
        if let Some((_, indices)) = matcher::search_exact_terms(self.exact_terms.iter(), line) {
            let should_retain = !self
                .inverse_terms
                .iter()
                .any(|term| term.match_full_line(line));

            if should_retain {
                Some(indices)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn check_jump_line(
        &self,
        (jump_line, mut indices): (String, Vec<usize>),
    ) -> Option<(String, Vec<usize>)> {
        if let Some(exact_indices) = self.check_terms(&jump_line) {
            indices.extend_from_slice(&exact_indices);
            indices.sort_unstable();
            indices.dedup();
            Some((jump_line, indices))
        } else {
            None
        }
    }
}

pub type UtcTime = DateTime<Utc>;

pub fn generate_data_file_path(filename: &str) -> Result<PathBuf> {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "vim", "Vim Clap") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

        let mut file = data_dir.to_path_buf();
        file.push(filename);

        return Ok(file);
    }

    Err(anyhow!("Couldn't create the Vim Clap project directory"))
}

pub fn generate_cache_file_path(filename: &str) -> Result<PathBuf> {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "vim", "Vim Clap") {
        let cache_dir = proj_dirs.cache_dir();
        std::fs::create_dir_all(cache_dir)?;

        let mut file = cache_dir.to_path_buf();
        file.push(filename);

        return Ok(file);
    }

    Err(anyhow!("Couldn't create the Vim Clap project directory"))
}

pub fn read_json_as<P: AsRef<Path>, T: serde::de::DeserializeOwned>(path: P) -> Result<T> {
    use std::io::BufReader;

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let deserializd = serde_json::from_reader(reader)?;

    Ok(deserializd)
}

pub fn load_json<T: serde::de::DeserializeOwned, P: AsRef<Path>>(path: Option<P>) -> Option<T> {
    path.and_then(|json_path| {
        if json_path.as_ref().exists() {
            crate::utils::read_json_as::<_, T>(json_path).ok()
        } else {
            None
        }
    })
}

pub fn write_json<T: serde::Serialize, P: AsRef<Path>>(obj: T, path: Option<P>) -> Result<()> {
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
        let lines: Vec<String> = if let Some(painter) = icon.painter() {
            iter.map(|x| painter.paint(&x)).collect()
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

pub(crate) fn expand_tilde(path: impl AsRef<str>) -> Result<PathBuf> {
    static HOME_PREFIX: Lazy<String> = Lazy::new(|| format!("~{}", std::path::MAIN_SEPARATOR));

    let fpath = if let Some(stripped) = path.as_ref().strip_prefix(HOME_PREFIX.as_str()) {
        let mut home_dir = directories::BaseDirs::new()
            .ok_or_else(|| anyhow!("Failed to construct BaseDirs"))?
            .home_dir()
            .to_path_buf();
        home_dir.push(stripped);
        home_dir
    } else {
        path.as_ref().into()
    };

    Ok(fpath)
}

/// Build the absolute path using cwd and relative path.
pub fn build_abs_path<P: AsRef<Path>>(cwd: P, curline: impl AsRef<Path>) -> PathBuf {
    let mut path: PathBuf = cwd.as_ref().into();
    path.push(curline);
    path
}

/// Counts lines in the source `handle`.
///
/// # Examples
/// ```ignore
/// let lines: usize = count_lines(std::fs::File::open("Cargo.toml").unwrap()).unwrap();
/// ```
///
/// Credit: https://github.com/eclarke/linecount/blob/master/src/lib.rs
pub fn count_lines<R: std::io::Read>(handle: R) -> Result<usize, std::io::Error> {
    use std::io::BufRead;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_lines() {
        let f: &[u8] = b"some text\nwith\nfour\nlines\n";
        assert_eq!(count_lines(f).unwrap(), 4);
    }
}
