use icon::file_icon;
use ignore::WalkBuilder;
use matcher::{MatchScope, MatcherBuilder};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use types::{ClapItem, Query};

use crate::{FileResult, FileCacheStore, SearchConfig, SearchPayload, SearchSink};

/// Run a file search with progressive result emission.
///
/// Uses `cache` to avoid re-walking the filesystem on every keystroke.
/// Results are emitted via `sink` when filtering completes.
pub async fn run_file_search(
    sink: &dyn SearchSink,
    query: String,
    config: SearchConfig,
    stop: Arc<AtomicBool>,
    cache: &FileCacheStore,
) {
    // Phase 1: get file list (cached or walk)
    let items = match cache.get(&config.cwd) {
        Some(items) => items,
        None => {
            let mut builder = WalkBuilder::new(&config.cwd);
            builder
                .hidden(!config.hidden_files)
                .git_ignore(config.respect_gitignore);

            let items: Vec<Arc<dyn ClapItem>> = builder
                .build()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().map_or(false, |ft| ft.is_file()))
                .take_while(|_| !stop.load(Ordering::Relaxed))
                .filter_map(|entry| {
                    entry
                        .path()
                        .strip_prefix(&config.cwd)
                        .ok()
                        .map(|p| Arc::new(p.to_string_lossy().to_string()) as Arc<dyn ClapItem>)
                })
                .collect();

            if stop.load(Ordering::Relaxed) {
                return;
            }

            cache.set(config.cwd.clone(), items.clone());
            items
        }
    };

    if stop.load(Ordering::Relaxed) {
        return;
    }

    // Phase 2: filter in parallel
    let matcher = MatcherBuilder::new()
        .match_scope(MatchScope::FileName)
        .build(Query::from(query.as_str()));

    let matched = filter::par_filter(items.into_par_iter(), &matcher);

    if stop.load(Ordering::Relaxed) {
        return;
    }

    // Phase 3: convert and emit
    let total_matched = matched.len();
    let results: Vec<FileResult> = matched
        .into_iter()
        .take(config.max_results)
        .map(|item| {
            let path_str = item.display_text().to_string();
            let icon_char = file_icon(&path_str);
            FileResult {
                display_path: path_str.clone(),
                path: config.cwd.join(&path_str).to_string_lossy().to_string(),
                icon: icon_char.to_string(),
                match_indices: item.indices.clone(),
                score: item.rank[0],
            }
        })
        .collect();

    sink.emit(SearchPayload::Files {
        results,
        total_matched,
        total_processed: total_matched,
        finished: true,
    });
}
