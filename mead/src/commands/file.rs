//! File open and watch commands.

use super::git::{get_git_branch, get_git_branch_url, get_git_last_author};
use super::RenderResponse;
use crate::state::AppState;
use markdown_preview_core::{
    calculate_document_stats, calculate_pdf_stats, find_git_root, to_html, DocumentType,
    RenderOptions,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::RwLock;

/// Expand ~ to home directory for open_file.
pub(super) fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

/// Read a local markdown file, render it, and collect git metadata.
pub(super) async fn render_local_markdown(path: &str) -> Result<RenderResponse, String> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let result = to_html(&content, &RenderOptions::gui())
        .map_err(|e| format!("Failed to render markdown: {e}"))?;
    let stats = calculate_document_stats(&content);

    let modified_at = super::get_file_mtime(std::path::Path::new(path)).await;
    let git_root = find_git_root(path);
    let git_branch = get_git_branch(path);
    let git_branch_url = git_branch
        .as_ref()
        .and_then(|b| get_git_branch_url(path, b));
    let git_last_author = get_git_last_author(path);

    Ok(RenderResponse::from_markdown(result, stats)
        .with_file_info(Some(path.to_string()), modified_at)
        .with_git_metadata(git_root, git_branch, git_branch_url, git_last_author))
}

/// Open and render a document file (markdown or PDF).
#[tauri::command]
pub async fn open_file(
    path: String,
    state: State<'_, Arc<RwLock<AppState>>>,
    app_handle: tauri::AppHandle,
) -> Result<RenderResponse, String> {
    // Expand ~ and resolve to absolute path
    let path_buf = expand_tilde(&path);
    let path_buf = if path_buf.is_relative() {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(&path_buf)
    } else {
        path_buf
    };

    // Canonicalize to get the real absolute path
    let path_buf = path_buf
        .canonicalize()
        .map_err(|e| format!("Failed to resolve path: {e}"))?;

    let absolute_path = path_buf.to_string_lossy().to_string();

    // Detect document type
    let doc_type = DocumentType::from_path(&path_buf)
        .ok_or_else(|| format!("Unsupported file type: {}", path_buf.display()))?;

    // Update state
    {
        let mut state_guard = state.write().await;
        state_guard.current_file = Some(path_buf.clone());
        state_guard.current_ssh_path = None;
        state_guard.add_recent_file(path_buf);
    }

    // Spawn background AI summary generation for the opened file
    crate::spawn_summary_for_file(state.inner().clone(), absolute_path.clone(), app_handle);

    // Handle based on document type
    match doc_type {
        DocumentType::Pdf => {
            let modified_at = super::get_file_mtime(std::path::Path::new(&absolute_path)).await;
            let git_root = find_git_root(&absolute_path);
            let git_branch = get_git_branch(&absolute_path);
            let git_branch_url = git_branch
                .as_ref()
                .and_then(|b| get_git_branch_url(&absolute_path, b));
            let git_last_author = get_git_last_author(&absolute_path);

            let stats = calculate_pdf_stats(None);
            Ok(RenderResponse::pdf(absolute_path.clone(), stats)
                .with_file_info(Some(absolute_path), modified_at)
                .with_git_metadata(git_root, git_branch, git_branch_url, git_last_author))
        }
        DocumentType::Markdown => render_local_markdown(&absolute_path).await,
    }
}

/// Start watching a file for changes.
#[tauri::command]
pub async fn watch_file(
    path: String,
    window: tauri::Window,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<(), String> {
    use markdown_preview_core::watcher::{FileWatcher, WatcherConfig};

    let path_buf = PathBuf::from(&path);

    // Stop any existing watcher
    {
        let mut state = state.write().await;
        if let Some(handle) = state.watcher_handle.take() {
            handle.abort();
        }
    }

    // Create a new file watcher
    let watcher = FileWatcher::new(&path_buf, WatcherConfig::default())
        .map_err(|e| format!("Failed to create watcher: {e}"))?;

    let mut rx = watcher.subscribe();
    let window_clone = window.clone();
    let path_clone = path.clone();

    // Spawn a task to handle file change events
    let handle = tokio::spawn(async move {
        // Keep the watcher alive
        let _watcher = watcher;

        loop {
            if rx.changed().await.is_err() {
                tracing::debug!("File watcher channel closed");
                break;
            }

            tracing::debug!(path = %path_clone, "File changed, reloading");

            // Read, render, and emit to frontend
            match render_local_markdown(&path_clone).await {
                Ok(response) => {
                    let _ = window_clone.emit("file-changed", response);
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to read file after change");
                }
            }
        }
    });

    // Store the handle
    {
        let mut state = state.write().await;
        state.watcher_handle = Some(handle);
    }

    Ok(())
}

/// Stop watching the current file.
#[tauri::command]
pub async fn unwatch_file(state: State<'_, Arc<RwLock<AppState>>>) -> Result<(), String> {
    let mut state = state.write().await;
    if let Some(handle) = state.watcher_handle.take() {
        handle.abort();
    }
    Ok(())
}
