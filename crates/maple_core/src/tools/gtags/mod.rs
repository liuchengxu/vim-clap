use dirs::Dirs;
use once_cell::sync::Lazy;
use std::path::PathBuf;

pub static GTAGS_EXISTS: Lazy<bool> = Lazy::new(|| gtags_executable_exists().unwrap_or(false));

/// Directory for `GTAGS`/`GRTAGS`.
pub static GTAGS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let gtags_dir = Dirs::project().data_dir().join("gtags");

    std::fs::create_dir_all(&gtags_dir).expect("Couldn't create gtags directory for vim-clap");

    gtags_dir
});

fn gtags_executable_exists() -> std::io::Result<bool> {
    let output = std::process::Command::new("gtags")
        .arg("--version")
        .stderr(std::process::Stdio::inherit())
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(line) = stdout.split('\n').next() {
        Ok(line.starts_with("gtags"))
    } else {
        Err(std::io::Error::other("gtags executable not found"))
    }
}
