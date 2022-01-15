use std::path::PathBuf;

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;

use crate::utils::PROJECT_DIRS;

pub static GTAGS_EXISTS: Lazy<bool> = Lazy::new(|| gtags_executable_exists().unwrap_or(false));

/// Directory for `GTAGS`/`GRTAGS`.
pub static GTAGS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let mut gtags_dir = PROJECT_DIRS.data_dir().to_path_buf();
    gtags_dir.push("gtags");

    std::fs::create_dir_all(&gtags_dir).expect("Couldn't create gtags directory for vim-clap");

    gtags_dir
});

fn gtags_executable_exists() -> Result<bool> {
    let output = std::process::Command::new("gtags")
        .arg("--version")
        .stderr(std::process::Stdio::inherit())
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    if let Some(line) = stdout.split('\n').next() {
        Ok(line.starts_with("gtags"))
    } else {
        Err(anyhow!("ctags executable not found"))
    }
}
