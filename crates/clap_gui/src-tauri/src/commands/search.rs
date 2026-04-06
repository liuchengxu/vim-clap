//! Thin Tauri wrapper around the reusable `clap_search` crate.

use clap_search::{SearchConfig, SearchPayload, SearchSink};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

/// Tauri implementation of SearchSink — emits results as Tauri events.
struct TauriSink(AppHandle);

impl SearchSink for TauriSink {
    fn emit(&self, payload: SearchPayload) {
        let _ = self.0.emit("search-results", &payload);
    }
}

#[tauri::command]
pub async fn start_search(
    query: String,
    mode: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let stop_signal = state.new_search();

    if query.is_empty() {
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

    let config = SearchConfig {
        cwd: state.cwd.read().clone(),
        max_results: state.config.search.max_results,
        hidden_files: state.config.search.hidden_files,
        respect_gitignore: state.config.search.respect_gitignore,
    };

    let sink = Arc::new(TauriSink(app.clone()));
    let cache = state.file_cache();

    tokio::spawn(async move {
        match mode.as_str() {
            "files" => {
                clap_search::run_file_search(sink.as_ref(), query, config, stop_signal, &cache)
                    .await;
            }
            "grep" => {
                clap_search::run_grep_search(sink, query, config, stop_signal).await;
            }
            _ => {}
        }
    });

    Ok(())
}

#[tauri::command]
pub fn refresh_file_cache(state: State<'_, AppState>) {
    state.file_cache().invalidate();
}

#[tauri::command]
pub fn get_cwd(state: State<'_, AppState>) -> String {
    state.cwd.read().to_string_lossy().to_string()
}

#[tauri::command]
pub fn quit_app(window: tauri::WebviewWindow) {
    let _ = window.hide();
}

#[tauri::command]
pub fn force_quit() {
    std::process::exit(0);
}
