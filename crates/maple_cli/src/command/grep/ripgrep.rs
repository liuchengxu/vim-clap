use filter::MatchedItem;
use grep::searcher::{sinks, BinaryDetection, SearcherBuilder};
use ignore::{DirEntry, WalkBuilder, WalkState};
use matcher::ClapItem;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

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

#[derive(Debug)]
pub struct FileResult {
    pub path: PathBuf,
    /// 0-based.
    pub line_number: usize,
    pub matched_item: MatchedItem,
}

pub fn run(search_root: impl AsRef<Path>, clap_matcher: matcher::Matcher) {
    let file_picker_config = FilePickerConfig::default();

    let searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();

    let total_processed = Arc::new(AtomicU64::new(0));

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
            let clap_matcher = clap_matcher.clone();
            let total_processed = total_processed.clone();
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

                let grep_matcher = matcher::SinglePathMatcher::default();

                let result = searcher.search_path(
                    &grep_matcher,
                    entry.path(),
                    sinks::UTF8(|line_num, line| {
                        // TODO
                        let item = Arc::new(line.to_string()) as Arc<dyn ClapItem>;
                        if let Some(matched_item) = clap_matcher.match_item(item) {
                            let file_result = FileResult {
                                path: entry.path().to_path_buf(),
                                line_number: line_num as usize - 1,
                                matched_item,
                            };
                            println!("{file_result:?}");
                        }

                        // TODO: record highlights.
                        // print!(
                        // "result: {}:{}:{line}",
                        // entry.path().display(),
                        // line_num as usize - 1
                        // );
                        // all_matches_sx
                        // .send(FileResult::new(entry.path(), line_num as usize - 1))
                        // .unwrap();

                        Ok(true)
                    }),
                );

                let processed = grep_matcher.processed();

                total_processed.fetch_add(processed, std::sync::atomic::Ordering::SeqCst);

                if let Err(err) = result {
                    tracing::error!("Global search error: {}, {}", entry.path().display(), err);
                }
                WalkState::Continue
            })
        });

    println!(
        "============= total_processed: {:?}",
        total_processed.load(std::sync::atomic::Ordering::SeqCst)
    );
}
