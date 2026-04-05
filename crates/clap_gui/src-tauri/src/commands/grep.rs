use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use ignore::WalkBuilder;
use matcher::{MatchScope, MatcherBuilder};
use parking_lot::Mutex;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use types::{ClapItem, Query};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct GrepResult {
    pub path: String,
    pub line_number: u64,
    pub line_content: String,
    pub match_indices: Vec<usize>,
    pub score: i32,
}

#[derive(Debug)]
struct GrepLine {
    text: String,
    path: String,
    line_number: u64,
}

#[tauri::command]
pub async fn search_grep(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<GrepResult>, String> {
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let cwd = state.cwd.read().clone();
    let max_results = state.config.search.max_results;
    let hidden = state.config.search.hidden_files;
    let gitignore = state.config.search.respect_gitignore;

    let regex_matcher =
        RegexMatcher::new(&query).map_err(|e| format!("Invalid pattern: {e}"))?;

    let grep_lines: Arc<Mutex<Vec<GrepLine>>> = Arc::new(Mutex::new(Vec::new()));

    let mut walker = WalkBuilder::new(&cwd);
    walker.hidden(!hidden).git_ignore(gitignore);

    let entries: Vec<_> = walker
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
        .collect();

    entries.par_iter().for_each(|entry| {
        let mut searcher = Searcher::new();
        let path = entry.path();
        let rel_path = path
            .strip_prefix(&cwd)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let lines = grep_lines.clone();
        let matcher = regex_matcher.clone();
        let _ = searcher.search_path(
            matcher,
            path,
            UTF8(|line_number, line_content| {
                let trimmed = line_content.trim_end().to_string();
                lines.lock().push(GrepLine {
                    text: format!("{rel_path}:{line_number}:{trimmed}"),
                    path: rel_path.clone(),
                    line_number,
                });
                Ok(true)
            }),
        );
    });

    // After par_iter completes, we're the sole owner of the Arc
    let collected = Arc::try_unwrap(grep_lines)
        .expect("par_iter complete, sole Arc owner")
        .into_inner();

    // Build a HashMap for O(1) lookup from display text -> index
    let lookup: HashMap<&str, usize> = collected
        .iter()
        .enumerate()
        .map(|(i, gl)| (gl.text.as_str(), i))
        .collect();

    let items: Vec<Arc<dyn ClapItem>> = collected
        .iter()
        .map(|gl| Arc::new(gl.text.clone()) as Arc<dyn ClapItem>)
        .collect();

    let matcher = MatcherBuilder::new()
        .match_scope(MatchScope::GrepLine)
        .build(Query::from(query.as_str()));

    let matched: Vec<_> = filter::par_filter(items.into_par_iter(), &matcher)
        .into_iter()
        .take(max_results)
        .collect();

    let results = matched
        .into_iter()
        .filter_map(|item| {
            let text = item.display_text().to_string();
            lookup.get(text.as_str()).map(|&idx| {
                let gl = &collected[idx];
                let line_content = gl
                    .text
                    .splitn(3, ':')
                    .nth(2)
                    .unwrap_or("")
                    .to_string();
                GrepResult {
                    path: cwd.join(&gl.path).to_string_lossy().to_string(),
                    line_number: gl.line_number,
                    line_content,
                    match_indices: item.indices.clone(),
                    score: item.rank[0],
                }
            })
        })
        .collect();

    Ok(results)
}
