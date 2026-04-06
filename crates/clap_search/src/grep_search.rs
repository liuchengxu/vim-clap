use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use icon::file_icon;
use ignore::WalkBuilder;
use matcher::{MatchScope, MatcherBuilder};
use parking_lot::Mutex;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use types::{ClapItem, Query, SearchTerm};

use crate::{GrepResultItem, SearchConfig, SearchPayload, SearchSink};

const UPDATE_INTERVAL: Duration = Duration::from_millis(200);

/// Extract a regex pattern for the initial grep pass from the query string.
///
/// Strips extended search syntax prefixes (`'`, `^`, `$`, `"`) and skips
/// inverse terms (`!`), then joins the remaining plain texts with `|` so
/// any matching term produces a candidate line for the matcher stage.
fn grep_pattern_from_query(query: &str) -> Option<String> {
    let parts: Vec<String> = query
        .split_whitespace()
        .map(SearchTerm::from)
        .filter(|t| !t.is_inverse_term())
        .map(|t| regex::escape(&t.text))
        .filter(|s| !s.is_empty())
        .collect();

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("|"))
    }
}

#[derive(Debug)]
struct GrepLine {
    text: String,
    path: String,
    line_number: u64,
    content_char_offset: usize,
    content_byte_offset: usize,
}

/// Run a grep search with progressive result emission.
///
/// Results are emitted via `sink` every 200ms as files are scanned,
/// with a final emission when the search completes.
pub async fn run_grep_search(
    sink: Arc<dyn SearchSink>,
    query: String,
    config: SearchConfig,
    stop: Arc<AtomicBool>,
) {
    let grep_pattern = match grep_pattern_from_query(&query) {
        Some(p) => p,
        None => return,
    };
    let regex_matcher = match RegexMatcher::new(&grep_pattern) {
        Ok(m) => m,
        Err(_) => return,
    };

    let grep_lines: Arc<Mutex<Vec<GrepLine>>> = Arc::new(Mutex::new(Vec::new()));
    let processed_files = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut walker = WalkBuilder::new(&config.cwd);
    walker
        .hidden(!config.hidden_files)
        .git_ignore(config.respect_gitignore);

    let entries: Vec<_> = walker
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
        .take_while(|_| !stop.load(Ordering::Relaxed))
        .collect();

    if stop.load(Ordering::Relaxed) {
        return;
    }

    let total_files = entries.len();

    // Spawn a progress emitter that sends intermediate results every 200ms
    let sink_progress = sink.clone();
    let stop_progress = stop.clone();
    let lines_progress = grep_lines.clone();
    let processed_progress = processed_files.clone();
    let query_progress = query.clone();
    let cwd_progress = config.cwd.clone();
    let max_results = config.max_results;

    let progress_handle = tokio::spawn(async move {
        let mut last_emit = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;

            if stop_progress.load(Ordering::Relaxed) {
                return;
            }

            let processed = processed_progress.load(Ordering::Relaxed);

            if last_emit.elapsed() >= UPDATE_INTERVAL {
                let snapshot: Vec<GrepLine> = {
                    let guard = lines_progress.lock();
                    guard
                        .iter()
                        .map(|gl| GrepLine {
                            text: gl.text.clone(),
                            path: gl.path.clone(),
                            line_number: gl.line_number,
                            content_char_offset: gl.content_char_offset,
                            content_byte_offset: gl.content_byte_offset,
                        })
                        .collect()
                };

                if !snapshot.is_empty() {
                    let results =
                        filter_and_format_grep(&snapshot, &query_progress, &cwd_progress, max_results);
                    sink_progress.emit(SearchPayload::Grep {
                        total_matched: results.len(),
                        results,
                        total_processed: processed,
                        finished: false,
                    });
                }

                last_emit = Instant::now();
            }

            if processed >= total_files {
                return;
            }
        }
    });

    // Run the actual grep in parallel
    let stop_grep = stop.clone();
    let cwd = config.cwd.clone();
    entries.par_iter().for_each(|entry| {
        if stop_grep.load(Ordering::Relaxed) {
            return;
        }

        let mut searcher = Searcher::new();
        let path = entry.path();
        let rel_path = path
            .strip_prefix(&cwd)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let lines = grep_lines.clone();
        let matcher = regex_matcher.clone();
        let stop_inner = stop_grep.clone();
        let _ = searcher.search_path(
            matcher,
            path,
            UTF8(|line_number, line_content| {
                if stop_inner.load(Ordering::Relaxed) {
                    return Ok(false);
                }
                let trimmed = line_content.trim_end().to_string();
                let text = format!("{rel_path}:{line_number}:1:{trimmed}");
                let byte_offset =
                    rel_path.len() + 1 + line_number.to_string().len() + 1 + 1 + 1;
                let char_offset =
                    rel_path.chars().count() + 1 + line_number.to_string().len() + 1 + 1 + 1;
                lines.lock().push(GrepLine {
                    text,
                    path: rel_path.clone(),
                    line_number,
                    content_char_offset: char_offset,
                    content_byte_offset: byte_offset,
                });
                Ok(true)
            }),
        );

        processed_files.fetch_add(1, Ordering::Relaxed);
    });

    let _ = progress_handle.await;

    if stop.load(Ordering::Relaxed) {
        return;
    }

    // Final emit with complete results
    let collected = Arc::try_unwrap(grep_lines)
        .expect("par_iter and progress task complete, sole Arc owner")
        .into_inner();

    let results = filter_and_format_grep(&collected, &query, &config.cwd, max_results);
    let total_matched = results.len();

    sink.emit(SearchPayload::Grep {
        results,
        total_matched,
        total_processed: total_files,
        finished: true,
    });
}

fn filter_and_format_grep(
    lines: &[GrepLine],
    query: &str,
    cwd: &std::path::PathBuf,
    max_results: usize,
) -> Vec<GrepResultItem> {
    let lookup: HashMap<&str, usize> = lines
        .iter()
        .enumerate()
        .map(|(i, gl)| (gl.text.as_str(), i))
        .collect();

    let items: Vec<Arc<dyn ClapItem>> = lines
        .iter()
        .map(|gl| Arc::new(gl.text.clone()) as Arc<dyn ClapItem>)
        .collect();

    let matcher = MatcherBuilder::new()
        .match_scope(MatchScope::GrepLine)
        .build(Query::from(query));

    filter::par_filter(items.into_par_iter(), &matcher)
        .into_iter()
        .take(max_results)
        .filter_map(|item| {
            let text = item.display_text().to_string();
            lookup.get(text.as_str()).map(|&idx| {
                let gl = &lines[idx];
                let line_content = &gl.text[gl.content_byte_offset..];
                let match_indices: Vec<usize> = item
                    .indices
                    .iter()
                    .filter_map(|&i| i.checked_sub(gl.content_char_offset))
                    .collect();
                GrepResultItem {
                    path: cwd.join(&gl.path).to_string_lossy().to_string(),
                    display_path: gl.path.clone(),
                    icon: file_icon(&gl.path).to_string(),
                    line_number: gl.line_number,
                    line_content: line_content.to_string(),
                    match_indices,
                    score: item.rank[0],
                }
            })
        })
        .collect()
}
