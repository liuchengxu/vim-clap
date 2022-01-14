use std::path::PathBuf;

use once_cell::sync::Lazy;

use crate::utils::PROJECT_DIRS;

/// Directory for `GTAGS`/`GRTAGS`.
pub static GTAGS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let mut gtags_dir = PROJECT_DIRS.data_dir().to_path_buf();
    gtags_dir.push("gtags");

    std::fs::create_dir_all(&gtags_dir).expect("Couldn't create gtags directory for vim-clap");

    gtags_dir
});
