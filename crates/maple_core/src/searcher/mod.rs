pub mod file;
pub mod files;
pub mod grep;
pub mod tagfiles;

use crate::stdio_server::Vim;
use icon::Icon;
use ignore::{WalkBuilder, WalkParallel};
use paths::AbsPathBuf;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SearchContext {
    pub icon: Icon,
    pub line_width: usize,
    pub paths: Vec<PathBuf>,
    pub vim: Vim,
    pub stop_signal: Arc<AtomicBool>,
    pub item_pool_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkConfig {
    /// IgnoreOptions
    /// Enables ignoring hidden files.
    /// Whether to hide hidden files in file picker and global search results. Defaults to true.
    pub hidden: bool,
    /// Enables following symlinks.
    /// Whether to follow symbolic links in file picker and file or directory completions. Defaults to true.
    pub follow_symlinks: bool,
    /// Enables reading ignore files from parent directories. Defaults to true.
    pub parents: bool,
    /// Enables reading `.ignore` files.
    /// Whether to hide files listed in .ignore in file picker and global search results. Defaults to true.
    pub ignore: bool,
    /// Enables reading `.gitignore` files.
    /// Whether to hide files listed in .gitignore in file picker and global search results. Defaults to true.
    pub git_ignore: bool,
    /// Enables reading global .gitignore, whose path is specified in git's config: `core.excludefile` option.
    /// Whether to hide files listed in global .gitignore in file picker and global search results. Defaults to true.
    pub git_global: bool,
    /// Enables reading `.git/info/exclude` files.
    /// Whether to hide files listed in .git/info/exclude in file picker and global search results. Defaults to true.
    pub git_exclude: bool,
    /// WalkBuilder options
    /// Maximum Depth to recurse directories in file picker and global search. Defaults to `None`.
    pub max_depth: Option<usize>,
}

impl Default for WalkConfig {
    fn default() -> Self {
        Self {
            hidden: true,
            follow_symlinks: true,
            parents: true,
            ignore: true,
            git_ignore: true,
            git_global: true,
            git_exclude: true,
            max_depth: None,
        }
    }
}

fn walk_parallel(paths: Vec<PathBuf>, walk_config: WalkConfig, provider_id: &str) -> WalkParallel {
    // paths must be non-empty.
    let search_root = paths[0].clone();

    let maybe_ignore_config = AbsPathBuf::try_from(search_root)
        .map(|project_dir| maple_config::config().ignore_config(provider_id, &project_dir))
        .ok();

    // TODO: smarter paths to search the parent directory of path first?
    let mut builder = WalkBuilder::new(&paths[0]);
    for path in &paths[1..] {
        builder.add(path);
    }

    builder
        .hidden(walk_config.hidden)
        .parents(walk_config.parents)
        .ignore(walk_config.ignore)
        .follow_links(walk_config.follow_symlinks)
        .git_ignore(walk_config.git_ignore)
        .git_global(walk_config.git_global)
        .git_exclude(walk_config.git_exclude)
        .max_depth(walk_config.max_depth)
        .filter_entry(move |entry| {
            let file_name = entry.file_name();

            // We always want to ignore the .git directory, otherwise if
            // `ignore` is turned off above, we end up with a lot of noise
            // in our picker.
            if file_name == ".git" {
                return false;
            }

            if let Some(ignore_config) = maybe_ignore_config {
                if let Some(file_name) = file_name.to_str() {
                    if ignore_config
                        .ignore_file_name_pattern
                        .iter()
                        .any(|name| file_name.contains(name.as_str()))
                    {
                        return false;
                    }
                }

                if let Some(file_path) = entry.path().to_str() {
                    if ignore_config
                        .ignore_file_path_pattern
                        .iter()
                        .any(|p| file_path.contains(p))
                    {
                        return false;
                    }
                }
            }

            true
        })
        .build_parallel()
}
