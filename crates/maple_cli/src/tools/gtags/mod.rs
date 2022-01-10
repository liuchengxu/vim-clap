use std::path::PathBuf;

use once_cell::sync::{Lazy, OnceCell};

/// Directory for `GTAGS`/`GRTAGS`.
pub static GTAGS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let proj_dirs = directories::ProjectDirs::from("org", "vim", "Vim Clap")
        .expect("Couldn't create project directory for vim-clap");

    let mut gtags_dir = proj_dirs.data_dir().to_path_buf();
    gtags_dir.push("gtags");

    std::fs::create_dir_all(&gtags_dir).expect("Couldn't create data directory for vim-clap");

    gtags_dir
});

