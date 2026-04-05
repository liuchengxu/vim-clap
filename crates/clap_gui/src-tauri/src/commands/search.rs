//! Unified streaming search: emits progressive results via Tauri events.

use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use icon::file_icon;
use ignore::WalkBuilder;
use matcher::{MatchScope, MatcherBuilder};
use parking_lot::Mutex;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use types::{ClapItem, Query};

use crate::state::AppState;

const UPDATE_INTERVAL: Duration = Duration::from_millis(200);

// --- Result types ---

#[derive(Debug, Clone, Serialize)]
pub struct FileResult {
    pub path: String,
    pub display_path: String,
    pub icon: String,
    pub match_indices: Vec<usize>,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrepResultItem {
    pub path: String,
    pub line_number: u64,
    pub line_content: String,
    pub match_indices: Vec<usize>,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SearchPayload {
    #[serde(rename = "files")]
    Files {
        results: Vec<FileResult>,
        total_matched: usize,
        total_processed: usize,
        finished: bool,
    },
    #[serde(rename = "grep")]
    Grep {
        results: Vec<GrepResultItem>,
        total_matched: usize,
        total_processed: usize,
        finished: bool,
    },
}

// --- Commands ---

#[tauri::command]
pub async fn start_search(
    query: String,
    mode: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let stop_signal = state.new_search();

    if query.is_empty() {
        // Emit empty results immediately
        let payload = match mode.as_str() {
            "grep" => SearchPayload::Grep {
                results: vec![],
                total_matched: 0,
                total_processed: 0,
                finished: true,
            },
            _ => SearchPayload::Files {
                results: vec![],
                total_matched: 0,
                total_processed: 0,
                finished: true,
            },
        };
        let _ = app.emit("search-results", &payload);
        return Ok(());
    }

    // Extract all state we need before spawning
    let cwd = state.cwd.read().clone();
    let max_results = state.config.search.max_results;
    let hidden = state.config.search.hidden_files;
    let gitignore = state.config.search.respect_gitignore;

    // For file search, try to use the cache
    let cached_files = if mode == "files" {
        state.get_cached_files(&cwd)
    } else {
        None
    };

    tokio::spawn(async move {
        match mode.as_str() {
            "files" => {
                run_file_search(
                    &app, query, cwd, max_results, hidden, gitignore, stop_signal, cached_files,
                )
                .await;
            }
            "grep" => {
                run_grep_search(&app, query, cwd, max_results, hidden, gitignore, stop_signal)
                    .await;
            }
            _ => {}
        }
    });

    Ok(())
}

#[tauri::command]
pub fn refresh_file_cache(state: State<'_, AppState>) {
    state.invalidate_file_cache();
}

// --- File search (progressive) ---

async fn run_file_search(
    app: &AppHandle,
    query: String,
    cwd: PathBuf,
    max_results: usize,
    hidden: bool,
    gitignore: bool,
    stop: Arc<AtomicBool>,
    cached: Option<Vec<Arc<dyn ClapItem>>>,
) {
    // Phase 1: get file list (cached or walk)
    let items = match cached {
        Some(items) => items,
        None => {
            let mut builder = WalkBuilder::new(&cwd);
            builder.hidden(!hidden).git_ignore(gitignore);

            let items: Vec<Arc<dyn ClapItem>> = builder
                .build()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().map_or(false, |ft| ft.is_file()))
                .take_while(|_| !stop.load(Ordering::Relaxed))
                .filter_map(|entry| {
                    entry
                        .path()
                        .strip_prefix(&cwd)
                        .ok()
                        .map(|p| Arc::new(p.to_string_lossy().to_string()) as Arc<dyn ClapItem>)
                })
                .collect();

            if stop.load(Ordering::Relaxed) {
                return;
            }

            // Cache for subsequent searches
            if let Some(state) = app.try_state::<AppState>() {
                state.set_cached_files(cwd.clone(), items.clone());
            }

            items
        }
    };

    if stop.load(Ordering::Relaxed) {
        return;
    }

    // Phase 2: filter in parallel (this is fast, typically <50ms even for 100k files)
    let matcher = MatcherBuilder::new()
        .match_scope(MatchScope::FileName)
        .build(Query::from(query.as_str()));

    let matched = filter::par_filter(items.into_par_iter(), &matcher);

    if stop.load(Ordering::Relaxed) {
        return;
    }

    // Phase 3: convert and emit final results
    let total_matched = matched.len();
    let results: Vec<FileResult> = matched
        .into_iter()
        .take(max_results)
        .map(|item| {
            let path_str = item.display_text().to_string();
            let icon_char = file_icon(&path_str);
            FileResult {
                display_path: path_str.clone(),
                path: cwd.join(&path_str).to_string_lossy().to_string(),
                icon: icon_char.to_string(),
                match_indices: item.indices.clone(),
                score: item.rank[0],
            }
        })
        .collect();

    let _ = app.emit(
        "search-results",
        &SearchPayload::Files {
            results,
            total_matched,
            total_processed: total_matched,
            finished: true,
        },
    );
}

// --- Grep search (progressive with streaming) ---

#[derive(Debug)]
struct GrepLine {
    text: String,
    path: String,
    line_number: u64,
    /// Char offset where the line content starts in `text`
    content_char_offset: usize,
    /// Byte offset for slicing
    content_byte_offset: usize,
}

async fn run_grep_search(
    app: &AppHandle,
    query: String,
    cwd: PathBuf,
    max_results: usize,
    hidden: bool,
    gitignore: bool,
    stop: Arc<AtomicBool>,
) {
    let regex_matcher = match RegexMatcher::new(&query) {
        Ok(m) => m,
        Err(_) => return,
    };

    let grep_lines: Arc<Mutex<Vec<GrepLine>>> = Arc::new(Mutex::new(Vec::new()));
    let processed_files = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut walker = WalkBuilder::new(&cwd);
    walker.hidden(!hidden).git_ignore(gitignore);

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
    let app_progress = app.clone();
    let stop_progress = stop.clone();
    let lines_progress = grep_lines.clone();
    let processed_progress = processed_files.clone();
    let query_progress = query.clone();
    let cwd_progress = cwd.clone();

    let progress_handle = tokio::spawn(async move {
        let mut last_emit = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;

            if stop_progress.load(Ordering::Relaxed) {
                return;
            }

            let processed = processed_progress.load(Ordering::Relaxed);
            let elapsed = last_emit.elapsed();

            if elapsed >= UPDATE_INTERVAL {
                // Snapshot current lines, filter, and emit
                let snapshot: Vec<GrepLine> = {
                    let guard = lines_progress.lock();
                    // We can't clone GrepLine easily, so just read what we need
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
                    let results = filter_and_format_grep(
                        &snapshot,
                        &query_progress,
                        &cwd_progress,
                        max_results,
                    );
                    let _ = app_progress.emit(
                        "search-results",
                        &SearchPayload::Grep {
                            total_matched: results.len(),
                            results,
                            total_processed: processed,
                            finished: false,
                        },
                    );
                }

                last_emit = Instant::now();
            }

            // Done when all files processed
            if processed >= total_files {
                return;
            }
        }
    });

    // Run the actual grep in parallel (blocking rayon work)
    let stop_grep = stop.clone();
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
                    return Ok(false); // stop searching this file
                }
                let trimmed = line_content.trim_end().to_string();
                let text = format!("{rel_path}:{line_number}:1:{trimmed}");
                let byte_offset = rel_path.len() + 1 + line_number.to_string().len() + 1 + 1 + 1;
                let char_offset = rel_path.chars().count() + 1 + line_number.to_string().len() + 1 + 1 + 1;
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

    // Wait for progress emitter to finish
    let _ = progress_handle.await;

    if stop.load(Ordering::Relaxed) {
        return;
    }

    // Final emit with complete results
    let collected = Arc::try_unwrap(grep_lines)
        .expect("par_iter and progress task complete, sole Arc owner")
        .into_inner();

    let results = filter_and_format_grep(&collected, &query, &cwd, max_results);
    let total_matched = results.len();

    let _ = app.emit(
        "search-results",
        &SearchPayload::Grep {
            results,
            total_matched,
            total_processed: total_files,
            finished: true,
        },
    );
}

/// Filter grep lines with fuzzy matcher and format for the frontend.
fn filter_and_format_grep(
    lines: &[GrepLine],
    query: &str,
    cwd: &PathBuf,
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
                // Shift match indices using char offset (not byte offset)
                let match_indices: Vec<usize> = item
                    .indices
                    .iter()
                    .filter_map(|&i| i.checked_sub(gl.content_char_offset))
                    .collect();
                GrepResultItem {
                    path: cwd.join(&gl.path).to_string_lossy().to_string(),
                    line_number: gl.line_number,
                    line_content: line_content.to_string(),
                    match_indices,
                    score: item.rank[0],
                }
            })
        })
        .collect()
}
