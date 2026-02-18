use etcetera::app_strategy::choose_native_strategy;
use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct Dirs;

pub struct DirsProject {
    home_dir: PathBuf,
    config_dir: PathBuf,
    cache_dir: PathBuf,
    data_dir: PathBuf,
}

impl DirsProject {
    fn new(app: impl AppStrategy) -> Self {
        DirsProject {
            home_dir: app.home_dir().to_path_buf(),
            config_dir: app.config_dir(),
            cache_dir: app.cache_dir(),
            data_dir: app.data_dir(),
        }
    }
}

impl Dirs {
    /// Project directory specifically for Vim Clap.
    ///
    /// All the files created by vim-clap are stored there.
    fn project() -> &'static DirsProject {
        static CELL: OnceLock<DirsProject> = OnceLock::new();

        CELL.get_or_init(|| {
            let app = AppStrategyArgs {
                top_level_domain: "org".to_string(),
                author: "vim".to_owned(),
                app_name: "Vim Clap".to_owned(),
            };

            if cfg!(target_os = "macos") {
                // SAFETY it does never fail
                let apple =
                    choose_native_strategy(app.clone()).expect("Cannot find the home directory");

                if apple.in_config_dir("config.toml").exists() {
                    return DirsProject::new(apple);
                }
            }
            let xdg = choose_app_strategy(app).expect("Cannot find the home directory");

            DirsProject::new(xdg)
        })
    }

    /// Get the home directory
    pub fn home_dir() -> &'static Path {
        Self::project().home_dir.as_path()
    }

    /// Get the config directory
    pub fn config_dir() -> &'static Path {
        Self::project().config_dir.as_path()
    }

    /// Get the cache directory
    pub fn cache_dir() -> &'static Path {
        Self::project().cache_dir.as_path()
    }

    /// Get the data directory
    pub fn data_dir() -> &'static Path {
        Self::project().data_dir.as_path()
    }

    /// Cache directory for Vim Clap project.
    pub fn clap_cache_dir() -> std::io::Result<&'static Path> {
        let cache_dir = Self::cache_dir();
        std::fs::create_dir_all(cache_dir)?;
        Ok(cache_dir)
    }
}
