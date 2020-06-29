use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

#[inline]
pub fn as_absolute_path<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::canonicalize(path.as_ref())?
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("{:?}, path:{}", e, path.as_ref().display()))
}

/// Build the absolute path using cwd and relative path.
pub fn build_abs_path(cwd: &str, curline: String) -> PathBuf {
    let mut path: PathBuf = cwd.into();
    path.push(&curline);
    path
}
