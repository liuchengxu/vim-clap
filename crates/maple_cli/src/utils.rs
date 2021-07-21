use std::path::{Path, PathBuf};

use chrono::prelude::*;
use anyhow::{anyhow, Result};

pub type UtcTime = DateTime<Utc>;

pub fn read_json_as<P: AsRef<Path>, T: serde::de::DeserializeOwned>(path: P) -> Result<T> {
    use std::io::BufReader;

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let deserializd = serde_json::from_reader(reader)?;

    Ok(deserializd)
}

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
