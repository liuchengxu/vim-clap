use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chrono::prelude::*;

use icon::IconPainter;
use utility::{println_json, println_json_with_length, read_first_lines};

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
    icon_painter: Option<IconPainter>,
) {
    let using_cache = true;
    if let Ok(iter) = read_first_lines(&tempfile, 100) {
        let lines: Vec<String> = if let Some(painter) = icon_painter {
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

/// Build the absolute path using cwd and relative path.
pub fn build_abs_path<P: AsRef<Path>>(cwd: P, curline: String) -> PathBuf {
    let mut path: PathBuf = cwd.as_ref().into();
    path.push(&curline);
    path
}
