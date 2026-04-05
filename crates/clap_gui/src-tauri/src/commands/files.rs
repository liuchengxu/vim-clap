use icon::file_icon;
use matcher::{MatchScope, MatcherBuilder};
use rayon::prelude::*;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::State;
use types::{ClapItem, Query};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub display_path: String,
    pub icon: String,
    pub match_indices: Vec<usize>,
    pub score: i32,
}

#[tauri::command]
pub async fn search_files(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let cwd = state.cwd.read().clone();
    let max_results = state.config.search.max_results;
    let hidden = state.config.search.hidden_files;
    let gitignore = state.config.search.respect_gitignore;
    let stop_signal = state.new_search();

    let mut builder = ignore::WalkBuilder::new(&cwd);
    builder.hidden(!hidden).git_ignore(gitignore);

    let items: Vec<Arc<dyn ClapItem>> = builder
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map_or(false, |ft| ft.is_file()))
        .take_while(|_| !stop_signal.load(Ordering::Relaxed))
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(&cwd)
                .ok()
                .map(|p| Arc::new(p.to_string_lossy().to_string()) as Arc<dyn ClapItem>)
        })
        .collect();

    if is_cancelled(&stop_signal) {
        return Ok(Vec::new());
    }

    let matcher = MatcherBuilder::new()
        .match_scope(MatchScope::FileName)
        .build(Query::from(query.as_str()));

    let matched = filter::par_filter(items.into_par_iter(), &matcher);

    let results = matched
        .into_iter()
        .take(max_results)
        .map(|item| {
            let path_str = item.display_text().to_string();
            let icon_char = file_icon(&path_str);
            SearchResult {
                display_path: path_str.clone(),
                path: cwd.join(&path_str).to_string_lossy().to_string(),
                icon: icon_char.to_string(),
                match_indices: item.indices.clone(),
                score: item.rank[0],
            }
        })
        .collect();

    Ok(results)
}

fn is_cancelled(signal: &Arc<AtomicBool>) -> bool {
    signal.load(Ordering::Relaxed)
}
