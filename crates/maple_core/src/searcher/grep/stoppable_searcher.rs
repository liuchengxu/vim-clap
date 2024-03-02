use crate::searcher::{walk_parallel, SearchContext, WalkConfig};
use crate::stdio_server::SearchProgressor;
use filter::MatchedItem;
use grep_searcher::{sinks, BinaryDetection, SearcherBuilder};
use icon::Icon;
use ignore::{DirEntry, WalkState};
use matcher::Matcher;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use types::{Rank, SearchProgressUpdate};

pub(super) const UPDATE_INTERVAL: Duration = Duration::from_millis(200);

pub(super) type SearcherMessage = crate::searcher::SearcherMessage<FileResult>;

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

/// Represents an matched item by searching a file.
#[derive(Debug, Clone)]
pub struct FileResult {
    pub path: PathBuf,
    pub line_number: u64,
    pub line: String,
    pub rank: Rank,
    pub indices_in_path: Vec<usize>,
    pub indices_in_line: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct StoppableSearchImpl {
    paths: Vec<PathBuf>,
    matcher: Matcher,
    sender: UnboundedSender<SearcherMessage>,
    stop_signal: Arc<AtomicBool>,
}

impl StoppableSearchImpl {
    pub(super) fn new(
        paths: Vec<PathBuf>,
        matcher: Matcher,
        sender: UnboundedSender<SearcherMessage>,
        stop_signal: Arc<AtomicBool>,
    ) -> Self {
        Self {
            paths,
            matcher,
            sender,
            stop_signal,
        }
    }

    pub(super) fn run(self) {
        let Self {
            paths,
            matcher,
            sender,
            stop_signal,
        } = self;

        let searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .build();

        let search_root = paths[0].clone();

        walk_parallel(paths, WalkConfig::default(), "grep").run(|| {
            let mut searcher = searcher.clone();
            let matcher = matcher.clone();
            let sender = sender.clone();
            let stop_signal = stop_signal.clone();
            let search_root = search_root.clone();
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
                    sinks::Lossy(|line_number, line| {
                        if line.is_empty() {
                            // Discontinue if the sender has been dropped.
                            return Ok(sender.send(SearcherMessage::ProcessedOne).is_ok());
                        }

                        let path = entry
                            .path()
                            .strip_prefix(&search_root)
                            .unwrap_or_else(|_| entry.path());
                        let line = line.trim();
                        let maybe_file_result =
                            matcher
                                .match_file_result(path, line)
                                .map(|matched| FileResult {
                                    path: entry.path().to_path_buf(),
                                    line_number,
                                    line: line.to_string(),
                                    rank: matched.rank,
                                    indices_in_path: matched.exact_indices,
                                    indices_in_line: matched.fuzzy_indices,
                                });

                        let searcher_message = if let Some(file_result) = maybe_file_result {
                            SearcherMessage::Match(file_result)
                        } else {
                            SearcherMessage::ProcessedOne
                        };

                        // Discontinue if the sender has been dropped.
                        Ok(sender.send(searcher_message).is_ok())
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

#[derive(Debug)]
struct BestFileResults {
    /// Time of last notification.
    past: Instant,
    results: Vec<FileResult>,
    last_lines: Vec<String>,
    last_visible_highlights: Vec<Vec<usize>>,
    max_capacity: usize,
}

impl BestFileResults {
    fn new(max_capacity: usize) -> Self {
        Self {
            past: Instant::now(),
            results: Vec::with_capacity(max_capacity),
            last_lines: Vec::with_capacity(max_capacity),
            last_visible_highlights: Vec::with_capacity(max_capacity),
            max_capacity,
        }
    }

    fn sort(&mut self) {
        self.results.sort_unstable_by(|a, b| b.rank.cmp(&a.rank));
    }
}

pub async fn search(query: String, matcher: Matcher, search_context: SearchContext) {
    let SearchContext {
        icon,
        line_width,
        vim,
        paths,
        stop_signal,
        item_pool_size,
    } = search_context;

    let progressor = SearchProgressor::new(vim, stop_signal.clone());
    let number = item_pool_size;
    let search_root = paths[0].clone();

    let mut best_results = BestFileResults::new(number);

    let (sender, mut receiver) = unbounded_channel();

    std::thread::Builder::new()
        .name("grep-worker".into())
        .spawn({
            let stop_signal = stop_signal.clone();
            move || StoppableSearchImpl::new(paths, matcher, sender, stop_signal).run()
        })
        .expect("Failed to spawn grep-worker thread");

    let mut total_matched = 0usize;
    let mut total_processed = 0usize;

    let to_display_lines = |best_results: &[FileResult], icon: Icon| {
        let grep_results = best_results
            .iter()
            .filter_map(|file_result| {
                let FileResult {
                    path,
                    line_number,
                    line,
                    rank,
                    indices_in_path,
                    indices_in_line,
                } = file_result;

                let maybe_column = indices_in_path.first().or_else(|| indices_in_line.first());

                if let Some(mut column) = maybe_column.copied() {
                    column += 1;
                    let mut fmt_line = if let Ok(relative_path) = path.strip_prefix(&search_root) {
                        format!("{}:{line_number}:{column}:", relative_path.display())
                    } else {
                        format!("{}:{line_number}:{column}:", path.display())
                    };
                    let offset = fmt_line.len();
                    fmt_line.push_str(line);

                    let mut indices = indices_in_path.clone();
                    indices.extend(indices_in_line.iter().map(|x| *x + offset));

                    let matched_item = MatchedItem::new(Arc::new(fmt_line), *rank, indices);

                    let line_number = *line_number as usize;
                    Some(printer::GrepResult {
                        matched_item,
                        path: path
                            .strip_prefix(&search_root)
                            .unwrap_or(path)
                            .to_path_buf(),
                        line_number,
                        column,
                        column_end: offset,
                    })
                } else {
                    None
                }
            })
            .collect();
        printer::grep_results_to_display_lines(grep_results, line_width, icon)
    };

    let now = std::time::Instant::now();
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
                    best_results.sort();

                    let now = Instant::now();
                    if now > best_results.past + UPDATE_INTERVAL {
                        let display_lines = to_display_lines(&best_results.results, icon);
                        progressor.update_all(&display_lines, total_matched, total_processed);
                        best_results.last_lines = display_lines.lines;
                        best_results.past = now;
                    }
                } else {
                    let last = best_results
                        .results
                        .last_mut()
                        .expect("Max capacity is non-zero; qed");

                    let new = file_result;
                    if let std::cmp::Ordering::Greater = new.rank.cmp(&last.rank) {
                        *last = new;
                        best_results.sort();
                    }

                    if total_matched % 16 == 0 || total_processed % 16 == 0 {
                        let now = Instant::now();
                        if now > best_results.past + UPDATE_INTERVAL {
                            let display_lines = to_display_lines(&best_results.results, icon);

                            let visible_highlights = display_lines
                                .indices
                                .iter()
                                .map(|line_highlights| {
                                    line_highlights
                                        .iter()
                                        .copied()
                                        .filter(|&x| x <= line_width)
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>();

                            if best_results.last_lines != display_lines.lines.as_slice()
                                || best_results.last_visible_highlights != visible_highlights
                            {
                                progressor.update_all(
                                    &display_lines,
                                    total_matched,
                                    total_processed,
                                );
                                best_results.last_lines = display_lines.lines;
                                best_results.last_visible_highlights = visible_highlights;
                            } else {
                                progressor.quick_update(total_matched, total_processed)
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

    let elapsed = now.elapsed().as_millis();

    let display_lines = to_display_lines(&best_results.results, icon);

    progressor.on_finished(display_lines, total_matched, total_processed);

    tracing::debug!(
        total_processed,
        total_matched,
        ?query,
        "Searching is complete in {elapsed:?}ms"
    );
}

pub async fn search_all(
    query: String,
    matcher: Matcher,
    paths: Vec<PathBuf>,
    stop_signal: Arc<AtomicBool>,
) {
    let (sender, mut receiver) = unbounded_channel();

    std::thread::Builder::new()
        .name("grep-worker".into())
        .spawn({
            let stop_signal = stop_signal.clone();
            move || StoppableSearchImpl::new(paths, matcher, sender, stop_signal).run()
        })
        .expect("Failed to spawn grep-worker thread");

    let mut matches = Vec::with_capacity(1000);

    let mut total_matched = 0usize;
    let mut total_processed = 0usize;

    let now = std::time::Instant::now();
    while let Some(searcher_message) = receiver.recv().await {
        if stop_signal.load(Ordering::SeqCst) {
            return;
        }

        match searcher_message {
            SearcherMessage::Match(file_result) => {
                total_matched += 1;
                total_processed += 1;

                matches.push(file_result);
            }
            SearcherMessage::ProcessedOne => {
                total_processed += 1;
            }
        }
    }

    let elapsed = now.elapsed().as_millis();

    tracing::debug!(
        total_processed,
        total_matched,
        ?query,
        "Searching is complete in {elapsed:?}ms"
    );
}
