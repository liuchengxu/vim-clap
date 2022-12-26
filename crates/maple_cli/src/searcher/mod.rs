use filter::MatchedItem;
use grep::searcher::{sinks, BinaryDetection, SearcherBuilder};
use ignore::{DirEntry, WalkBuilder, WalkState};
use matcher::Matcher;
use printer::DisplayLines;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use types::ProgressUpdate;

const UPDATE_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Default)]
struct MatchEverything;

impl grep_matcher::Matcher for MatchEverything {
    type Captures = grep_matcher::NoCaptures;
    type Error = String;

    fn find_at(
        &self,
        _haystack: &[u8],
        at: usize,
    ) -> Result<Option<grep_matcher::Match>, Self::Error> {
        // Signal there is a match and should be processed in the sink later.
        Ok(Some(grep_matcher::Match::zero(at)))
    }

    fn new_captures(&self) -> Result<Self::Captures, Self::Error> {
        Ok(grep_matcher::NoCaptures::new())
    }
}

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
enum SearcherMessage {
    Match(FileResult),
    ProcessedOne,
}

/// Represents a matched item in a file.
#[derive(Debug, Clone)]
pub struct FileResult {
    pub path: PathBuf,
    /// 0-based.
    pub line_number: usize,
    pub matched_item: MatchedItem,
}

fn run_searcher_worker(
    search_root: PathBuf,
    clap_matcher: Matcher,
    sender: UnboundedSender<SearcherMessage>,
    stop_signal: Arc<AtomicBool>,
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
            let sender = sender.clone();
            let stop_signal = stop_signal.clone();
            Box::new(move |entry: Result<DirEntry, ignore::Error>| -> WalkState {
                if stop_signal.load(Ordering::SeqCst) {
                    return WalkState::Quit;
                }

                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => return WalkState::Continue,
                };

                // TODO: Add search syntax for filtering path

                // Only search file and skip everything else.
                match entry.file_type() {
                    Some(entry) if entry.is_file() => {}
                    _ => return WalkState::Continue,
                };

                let result = searcher.search_path(
                    &MatchEverything,
                    entry.path(),
                    sinks::Lossy(|line_num, line| {
                        if line.is_empty() {
                            if let Err(err) = sender.send(SearcherMessage::ProcessedOne) {
                                tracing::debug!("SearcherMessage sender is dropped: {err:?}");
                                return Ok(false);
                            } else {
                                return Ok(true);
                            }
                        }

                        let maybe_file_result = clap_matcher
                            .match_file_result(entry.path(), line.trim())
                            .map(|matched_item| FileResult {
                                // TODO: May be cached somewhere so that the allcation won't be
                                // neccessary each time.
                                path: entry.path().to_path_buf(),
                                line_number: line_num as usize - 1,
                                matched_item,
                            });

                        let searcher_message = if let Some(file_result) = maybe_file_result {
                            SearcherMessage::Match(file_result)
                        } else {
                            SearcherMessage::ProcessedOne
                        };

                        if let Err(err) = sender.send(searcher_message) {
                            tracing::debug!("SearcherMessage sender is dropped: {err:?}");
                            Ok(false)
                        } else {
                            Ok(true)
                        }
                    }),
                );

                if let Err(err) = result {
                    tracing::error!("Global search error: {}, {}", entry.path().display(), err);
                }

                WalkState::Continue
            })
        });
}

#[derive(Debug)]
pub struct SearchResult {
    // TODO: bounded matched items.
    pub matches: Vec<FileResult>,
    pub total_matched: u64,
    pub total_processed: u64,
}

pub async fn search_with_progress(search_root: PathBuf, clap_matcher: Matcher) -> SearchResult {
    let (sender, mut receiver) = unbounded_channel();

    let stop_signal = Arc::new(AtomicBool::new(false));

    std::thread::Builder::new()
        .name("searcher-worker".into())
        .spawn(move || run_searcher_worker(search_root, clap_matcher, sender, stop_signal))
        .expect("Failed to spawn searcher worker thread");

    let mut matches = Vec::new();
    let mut total_matched = 0;
    let mut total_processed = 0;

    let mut past = Instant::now();

    while let Some(searcher_message) = receiver.recv().await {
        match searcher_message {
            SearcherMessage::Match(file_result) => {
                tracing::debug!(
                    "{}:{}:{}:{}, {:?}, {:?}",
                    file_result.path.display(),
                    file_result.line_number + 1,
                    1,
                    file_result.matched_item.display_text(),
                    file_result.matched_item.indices,
                    file_result.matched_item.score,
                );
                matches.push(file_result);
                total_matched += 1;
                total_processed += 1;
            }
            SearcherMessage::ProcessedOne => {
                total_processed += 1;
            }
        }

        if total_matched % 16 == 0 || total_processed % 16 == 0 {
            let now = Instant::now();
            if now > past + UPDATE_INTERVAL {
                println!("total_matched: {total_matched:?}, total_processed: {total_processed:?}");
                past = now;
            }
        }
    }

    SearchResult {
        matches,
        total_matched,
        total_processed,
    }
}

use icon::Icon;

#[derive(Debug)]
pub struct BestFileResults {
    /// Time of last notification.
    pub past: Instant,
    pub results: Vec<FileResult>,
    pub last_lines: Vec<String>,
    pub last_visible_highlights: Vec<Vec<usize>>,
    pub max_capacity: usize,
}

impl BestFileResults {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            past: Instant::now(),
            results: Vec::with_capacity(max_capacity),
            last_lines: Vec::with_capacity(max_capacity),
            last_visible_highlights: Vec::with_capacity(max_capacity),
            max_capacity,
        }
    }
}

pub async fn search<P: ProgressUpdate<DisplayLines>>(
    search_root: PathBuf,
    clap_matcher: Matcher,
    stop_signal: Arc<AtomicBool>,
    number: usize,
    icon: Icon,
    winwidth: usize,
    progressor: P,
) {
    let mut best_results = BestFileResults::new(number);

    let (sender, mut receiver) = unbounded_channel();

    std::thread::Builder::new()
        .name("searcher-worker".into())
        .spawn({
            let stop_signal = stop_signal.clone();
            let search_root = search_root.clone();
            move || run_searcher_worker(search_root, clap_matcher, sender, stop_signal)
        })
        .expect("Failed to spawn searcher worker thread");

    let mut total_matched = 0u64;
    let mut total_processed = 0u64;

    let to_display_lines = |best_results: Vec<FileResult>, winwidth: usize, icon: Icon| {
        let items = best_results
            .into_iter()
            .filter_map(|file_result| {
                let mut file_result = file_result;
                if let Some(mut column) = file_result.matched_item.indices.first().copied() {
                    let line_number = file_result.line_number + 1;
                    column += 1;
                    let mut fmt_line =
                        if let Ok(relative_path) = file_result.path.strip_prefix(&search_root) {
                            format!("{}:{line_number}:{column}:", relative_path.display())
                        } else {
                            format!("{}:{line_number}:{column}:", file_result.path.display())
                        };
                    let offset = fmt_line.len();
                    fmt_line.push_str(file_result.matched_item.display_text().as_ref());
                    file_result.matched_item.output_text.replace(fmt_line);
                    file_result
                        .matched_item
                        .indices
                        .iter_mut()
                        .for_each(|x| *x += offset);
                    Some(file_result.matched_item)
                } else {
                    None
                }
            })
            .collect();
        printer::decorate_lines(items, winwidth, icon)
    };

    while let Some(searcher_message) = receiver.recv().await {
        if stop_signal.load(Ordering::SeqCst) {
            return;
        }

        match searcher_message {
            SearcherMessage::Match(file_result) => {
                total_matched += 1;
                total_processed += 1;

                if best_results.results.len() <= best_results.max_capacity {
                    best_results.results.push(file_result);
                    best_results
                        .results
                        .sort_unstable_by(|a, b| b.matched_item.score.cmp(&a.matched_item.score));

                    let now = Instant::now();
                    if now > best_results.past + UPDATE_INTERVAL {
                        let display_lines =
                            to_display_lines(best_results.results.clone(), winwidth, icon);
                        progressor.update_progress(
                            Some(&display_lines),
                            total_matched as usize,
                            total_processed as usize,
                        );
                        best_results.last_lines = display_lines.lines;
                        best_results.past = now;
                    }
                } else {
                    let last = best_results
                        .results
                        .last_mut()
                        .expect("Max capacity is non-zero; qed");

                    let new = file_result;
                    if new.matched_item.score > last.matched_item.score {
                        *last = new;
                        best_results.results.sort_unstable_by(|a, b| {
                            b.matched_item.score.cmp(&a.matched_item.score)
                        });
                    }

                    if total_matched % 16 == 0 || total_processed % 16 == 0 {
                        let now = Instant::now();
                        if now > best_results.past + UPDATE_INTERVAL {
                            let display_lines =
                                to_display_lines(best_results.results.clone(), winwidth, icon);

                            let visible_highlights = display_lines
                                .indices
                                .iter()
                                .map(|line_highlights| {
                                    line_highlights
                                        .iter()
                                        .copied()
                                        .filter(|&x| x <= winwidth)
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>();

                            // TODO: the lines are the same, but the highlights are not.
                            if best_results.last_lines != display_lines.lines.as_slice()
                                || best_results.last_visible_highlights != visible_highlights
                            {
                                progressor.update_progress(
                                    Some(&display_lines),
                                    total_matched as usize,
                                    total_processed as usize,
                                );
                                best_results.last_lines = display_lines.lines;
                                best_results.last_visible_highlights = visible_highlights;
                            } else {
                                progressor.update_progress(
                                    None,
                                    total_matched as usize,
                                    total_processed as usize,
                                )
                            }

                            best_results.past = now;
                        }
                    }
                }
            }
            SearcherMessage::ProcessedOne => {
                total_processed += 1;
            }
        }
    }

    let BestFileResults { results, .. } = best_results;

    let display_lines = to_display_lines(results, winwidth, icon);

    progressor.update_progress_on_finished(
        display_lines,
        total_matched as usize,
        total_processed as usize,
    );

    tracing::debug!(
        "Searching is done, total_matched: {total_matched:?}, total_processed: {total_processed}",
    );
}
