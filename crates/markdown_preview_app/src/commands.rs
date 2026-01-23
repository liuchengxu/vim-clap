//! Tauri IPC commands for the markdown preview app.

use crate::state::AppState;
use markdown_preview_core::{calculate_document_stats, find_git_root, to_html, DocumentStats, RenderOptions};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::RwLock;

/// Result of rendering markdown, sent to the frontend.
#[derive(Clone, serde::Serialize)]
pub struct RenderResponse {
    /// Rendered HTML content
    pub html: String,
    /// Source line to rendered element mapping
    pub line_map: Vec<usize>,
    /// Document statistics
    pub stats: DocumentStats,
    /// Git repository root (if applicable)
    pub git_root: Option<String>,
    /// Path to the rendered file
    pub file_path: Option<String>,
}

/// Render markdown content to HTML.
#[tauri::command]
pub async fn render_markdown(content: String) -> Result<RenderResponse, String> {
    let result = to_html(&content, &RenderOptions::gfm()).map_err(|e| e.to_string())?;

    let stats = calculate_document_stats(&content);

    Ok(RenderResponse {
        html: result.html,
        line_map: result.line_map,
        stats,
        git_root: None,
        file_path: None,
    })
}

/// Open and render a markdown file.
#[tauri::command]
pub async fn open_file(
    path: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<RenderResponse, String> {
    let path_buf = PathBuf::from(&path);

    // Read the file
    let content = tokio::fs::read_to_string(&path_buf)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    // Render markdown
    let result = to_html(&content, &RenderOptions::gfm()).map_err(|e| e.to_string())?;

    let stats = calculate_document_stats(&content);
    let git_root = find_git_root(&path);

    // Update state
    {
        let mut state = state.write().await;
        state.current_file = Some(path_buf.clone());
        state.add_recent_file(path_buf);
    }

    Ok(RenderResponse {
        html: result.html,
        line_map: result.line_map,
        stats,
        git_root,
        file_path: Some(path),
    })
}

/// Get the list of recently opened files.
#[tauri::command]
pub async fn get_recent_files(
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<Vec<String>, String> {
    let state = state.read().await;
    Ok(state.get_recent_files())
}

/// Add a file to the recent files list.
#[tauri::command]
pub async fn add_recent_file(
    path: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<(), String> {
    let mut state = state.write().await;
    state.add_recent_file(PathBuf::from(path));
    Ok(())
}

/// Clear the recent files list.
#[tauri::command]
pub async fn clear_recent_files(
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<(), String> {
    let mut state = state.write().await;
    state.clear_recent_files();
    Ok(())
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

            // Read and render the file
            match tokio::fs::read_to_string(&path_clone).await {
                Ok(content) => {
                    if let Ok(result) = to_html(&content, &RenderOptions::gfm()) {
                        let stats = calculate_document_stats(&content);
                        let git_root = find_git_root(&path_clone);

                        let response = RenderResponse {
                            html: result.html,
                            line_map: result.line_map,
                            stats,
                            git_root,
                            file_path: Some(path_clone.clone()),
                        };

                        // Emit event to frontend
                        let _ = window_clone.emit("file-changed", response);
                    }
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

/// Check clipboard for a markdown file path and return it if valid.
#[tauri::command]
pub async fn check_clipboard_for_markdown(
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    tracing::debug!("Checking clipboard for markdown file path");

    let clipboard_text = match app.clipboard().read_text() {
        Ok(text) => {
            tracing::debug!(text = %text, "Read clipboard text");
            text
        }
        Err(e) => {
            tracing::debug!(error = %e, "Failed to read clipboard");
            return Ok(None);
        }
    };

    // Check if it's a valid markdown file path
    let text = clipboard_text.trim();
    if text.is_empty() {
        tracing::debug!("Clipboard is empty");
        return Ok(None);
    }

    let path = std::path::Path::new(text);
    tracing::debug!(path = %path.display(), is_absolute = path.is_absolute(), exists = path.exists(), "Checking path");

    // Check if it looks like a file path and is a markdown file
    if path.is_absolute() && path.exists() {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            tracing::debug!(extension = %ext_str, "Found extension");
            if matches!(ext_str.as_str(), "md" | "markdown" | "mdown" | "mkdn" | "mkd") {
                tracing::info!(path = %text, "Found markdown file in clipboard");
                return Ok(Some(text.to_string()));
            }
        }
    }

    tracing::debug!("No valid markdown file path in clipboard");
    Ok(None)
}
