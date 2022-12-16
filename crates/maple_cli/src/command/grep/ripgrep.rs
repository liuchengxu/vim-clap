use filter::MatchedItem;
use grep::searcher::{sinks, BinaryDetection, SearcherBuilder};
use ignore::{DirEntry, WalkBuilder, WalkState};
use matcher::ClapItem;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

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

pub fn search_parallel(
    search_root: PathBuf,
    clap_matcher: matcher::Matcher,
    total_processed: Arc<AtomicU64>,
    item_sender: UnboundedSender<FileResult>,
) {
    let file_picker_config = FilePickerConfig::default();

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
            let clap_matcher = clap_matcher.clone();
            let total_processed = total_processed.clone();
            let item_sender = item_sender.clone();
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

                let inverse_matcher = matcher::InverseMatcherWithRecord::default();

                let result = searcher.search_path(
                    &inverse_matcher,
                    entry.path(),
                    sinks::UTF8(|line_num, line| {
                        let item = Arc::new(line.to_string()) as Arc<dyn ClapItem>;
                        if let Some(matched_item) = clap_matcher.match_item(item) {
                            let file_result = FileResult {
                                path: entry.path().to_path_buf(),
                                line_number: line_num as usize - 1,
                                matched_item,
                            };
                            item_sender.send(file_result).unwrap();
                        }

                        Ok(true)
                    }),
                );

                let processed = inverse_matcher.processed();

                total_processed.fetch_add(processed, Ordering::SeqCst);

                if let Err(err) = result {
                    tracing::error!("Global search error: {}, {}", entry.path().display(), err);
                }
                WalkState::Continue
            })
        });
}

pub async fn run(search_root: impl AsRef<Path>, clap_matcher: matcher::Matcher) {
    let (sender, mut receiver) = unbounded_channel();

    let total_processed = Arc::new(AtomicU64::new(0));

    std::thread::spawn({
        let search_root = search_root.as_ref().to_path_buf();
        let total_processed = total_processed.clone();

        move || search_parallel(search_root, clap_matcher, total_processed, sender)
    });

    let mut total_matched = 0;
    while let Some(file_result) = receiver.recv().await {
        total_matched += 1;
        println!("=============== file_result: {file_result:?}");
    }

    println!(
        "============= total_processed: {:?}, total_matched: {total_matched:?}",
        total_processed.load(Ordering::SeqCst)
    );
}
