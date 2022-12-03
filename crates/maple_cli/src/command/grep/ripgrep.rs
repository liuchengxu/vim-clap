use grep_regex::RegexMatcherBuilder;
use grep_searcher::{sinks, BinaryDetection, SearcherBuilder};
use ignore::{DirEntry, WalkBuilder, WalkState};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilePickerConfig {
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

impl Default for FilePickerConfig {
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

pub fn run(regex: &str, search_root: impl AsRef<Path>) {
    let smart_case = true;
    let file_picker_config = FilePickerConfig::default();

    if let Ok(matcher) = RegexMatcherBuilder::new()
        .case_smart(smart_case)
        .build(regex)
    {
        let searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .build();

        WalkBuilder::new(search_root)
            .hidden(file_picker_config.hidden)
            .parents(file_picker_config.parents)
            .ignore(file_picker_config.ignore)
            .follow_links(file_picker_config.follow_symlinks)
            .git_ignore(file_picker_config.git_ignore)
            .git_global(file_picker_config.git_global)
            .git_exclude(file_picker_config.git_exclude)
            .max_depth(file_picker_config.max_depth)
            // We always want to ignore the .git directory, otherwise if
            // `ignore` is turned off above, we end up with a lot of noise
            // in our picker.
            .filter_entry(|entry| entry.file_name() != ".git")
            .build_parallel()
            .run(|| {
                let mut searcher = searcher.clone();
                let matcher = matcher.clone();
                // let all_matches_sx = all_matches_sx.clone();
                Box::new(move |entry: Result<DirEntry, ignore::Error>| -> WalkState {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(_) => return WalkState::Continue,
                    };

                    // Only search file and skip everything else.
                    match entry.file_type() {
                        Some(entry) if entry.is_file() => {}
                        _ => return WalkState::Continue,
                    };

                    let result = searcher.search_path(
                        &matcher,
                        entry.path(),
                        sinks::UTF8(|line_num, line| {
                            // TODO: record highlights.
                            println!(
                                "result: {:?}:{}:{line}",
                                entry.path().display(),
                                line_num as usize - 1
                            );
                            // all_matches_sx
                            // .send(FileResult::new(entry.path(), line_num as usize - 1))
                            // .unwrap();

                            Ok(true)
                        }),
                    );

                    if let Err(err) = result {
                        tracing::error!("Global search error: {}, {}", entry.path().display(), err);
                    }
                    WalkState::Continue
                })
            });
    }
}
