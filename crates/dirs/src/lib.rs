use directories::{BaseDirs, ProjectDirs};
use std::path::PathBuf;
use std::sync::OnceLock;

pub struct Dirs;

impl Dirs {
    /// Project directory specifically for Vim Clap.
    ///
    /// All the files created by vim-clap are stored there.
    pub fn project() -> &'static ProjectDirs {
        static CELL: OnceLock<ProjectDirs> = OnceLock::new();

        CELL.get_or_init(|| {
            ProjectDirs::from("org", "vim", "Vim Clap")
                .expect("Couldn't create project directory for vim-clap")
        })
    }

    /// Provides access to the standard directories that the operating system uses.
    pub fn base() -> &'static BaseDirs {
        static CELL: OnceLock<BaseDirs> = OnceLock::new();

        CELL.get_or_init(|| BaseDirs::new().expect("Failed to construct BaseDirs"))
    }

    /// Cache directory for Vim Clap project.
    pub fn clap_cache_dir() -> std::io::Result<PathBuf> {
        let cache_dir = Self::project().cache_dir();
        std::fs::create_dir_all(cache_dir)?;
        Ok(cache_dir.to_path_buf())
    }
}
