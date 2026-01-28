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
        stats,
        git_root: None,
        file_path: None,
    })
}

/// Expand ~ to home directory for open_file.
fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

/// Open and render a markdown file.
#[tauri::command]
pub async fn open_file(
    path: String,
    state: State<'_, Arc<RwLock<AppState>>>,
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
    let path_buf = path_buf.canonicalize()
        .map_err(|e| format!("Failed to resolve path: {e}"))?;

    let absolute_path = path_buf.to_string_lossy().to_string();

    // Read the file
    let content = tokio::fs::read_to_string(&path_buf)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    // Render markdown
    let result = to_html(&content, &RenderOptions::gfm()).map_err(|e| e.to_string())?;

    let stats = calculate_document_stats(&content);
    let git_root = find_git_root(&absolute_path);

    // Update state
    {
        let mut state = state.write().await;
        state.current_file = Some(path_buf.clone());
        state.add_recent_file(path_buf);
    }

    Ok(RenderResponse {
        html: result.html,
        stats,
        git_root,
        file_path: Some(absolute_path),
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

/// Path completion entry.
#[derive(Clone, serde::Serialize)]
pub struct PathCompletion {
    /// The full path
    pub path: String,
    /// Just the file/directory name
    pub name: String,
    /// Whether this is a directory
    pub is_dir: bool,
}

/// Expand ~ to home directory and resolve relative paths.
fn expand_path(partial: &str) -> PathBuf {
    // Expand ~ to home directory
    let expanded = if partial.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(&partial[2..])
        } else {
            PathBuf::from(partial)
        }
    } else if partial == "~" {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(partial))
    } else if partial.starts_with('/') || partial.starts_with('.') {
        // Absolute path or explicit relative path
        PathBuf::from(partial)
    } else {
        // Relative path - resolve from current directory
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(partial)
    };

    expanded
}

/// Format path for display, using ~ for home directory.
fn format_path_for_display(path: &std::path::Path, is_dir: bool) -> String {
    let path_str = path.to_string_lossy();
    let formatted = if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if path_str.starts_with(home_str.as_ref()) {
            format!("~{}", &path_str[home_str.len()..])
        } else {
            path_str.to_string()
        }
    } else {
        path_str.to_string()
    };

    if is_dir && !formatted.ends_with('/') {
        format!("{formatted}/")
    } else {
        formatted
    }
}

/// Complete a partial file path.
/// Returns matching directories and markdown files.
/// Supports:
/// - Absolute paths: /Users/foo/bar
/// - Home directory: ~/Documents
/// - Relative paths: ./foo, ../bar, or just foo
#[tauri::command]
pub async fn complete_path(partial: String) -> Result<Vec<PathCompletion>, String> {
    use std::path::Path;

    let partial = partial.trim();
    if partial.is_empty() {
        return Ok(Vec::new());
    }

    // Expand ~ and resolve relative paths
    let expanded = expand_path(partial);

    // Determine the directory to list and the prefix to filter by
    let (dir_to_list, prefix, use_tilde) = if partial.ends_with('/') || partial.ends_with(std::path::MAIN_SEPARATOR) {
        // User typed a complete directory path, list its contents
        (expanded.clone(), String::new(), partial.starts_with('~'))
    } else if expanded.is_dir() {
        // Path is a directory without trailing slash, list its contents
        (expanded.clone(), String::new(), partial.starts_with('~'))
    } else {
        // Partial filename - get parent directory and filter prefix
        let parent = expanded.parent().unwrap_or(Path::new("/"));
        let file_name = expanded.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        (parent.to_path_buf(), file_name, partial.starts_with('~'))
    };

    // Check if directory exists
    if !dir_to_list.is_dir() {
        return Ok(Vec::new());
    }

    // Read directory entries
    let mut completions = Vec::new();
    let prefix_lower = prefix.to_lowercase();

    match std::fs::read_dir(&dir_to_list) {
        Ok(entries) => {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip hidden files unless user is explicitly typing a dot
                if name.starts_with('.') && !prefix.starts_with('.') {
                    continue;
                }

                // Filter by prefix (case-insensitive)
                if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix_lower) {
                    continue;
                }

                let entry_path = entry.path();
                let is_dir = entry_path.is_dir();

                // Format path - use ~ notation if user started with ~
                let display_path = if use_tilde {
                    format_path_for_display(&entry_path, is_dir)
                } else if is_dir {
                    format!("{}/", entry_path.display())
                } else {
                    entry_path.to_string_lossy().to_string()
                };

                // Include directories and markdown files
                if is_dir {
                    completions.push(PathCompletion {
                        path: display_path,
                        name: format!("{name}/"),
                        is_dir: true,
                    });
                } else if let Some(ext) = entry_path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if matches!(ext_str.as_str(), "md" | "markdown" | "mdown" | "mkdn" | "mkd") {
                        completions.push(PathCompletion {
                            path: display_path,
                            name,
                            is_dir: false,
                        });
                    }
                }
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "Failed to read directory");
            return Ok(Vec::new());
        }
    }

    // Sort: directories first, then alphabetically
    completions.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    // Limit results
    completions.truncate(20);

    Ok(completions)
}
