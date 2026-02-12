//! File open and watch commands.

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

/// Get the current git branch name for a file path.
pub(super) fn get_git_branch(file_path: &str) -> Option<String> {
    let path = std::path::Path::new(file_path);
    let dir = if path.is_file() { path.parent()? } else { path };

    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Some(branch);
        }
    }
    None
}

/// Get the last commit author for a specific file.
pub(super) fn get_git_last_author(file_path: &str) -> Option<String> {
    let path = std::path::Path::new(file_path);
    let dir = if path.is_file() { path.parent()? } else { path };

    let output = std::process::Command::new("git")
        .args(["log", "-1", "--format=%an", "--", file_path])
        .current_dir(dir)
        .output()
        .ok()?;

    if output.status.success() {
        let author = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !author.is_empty() {
            return Some(author);
        }
    }
    None
}

/// Get the GitHub URL for the current branch.
pub(super) fn get_git_branch_url(file_path: &str, branch: &str) -> Option<String> {
    let path = std::path::Path::new(file_path);
    let dir = if path.is_file() { path.parent()? } else { path };

    // Get the remote URL
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if remote_url.is_empty() {
        return None;
    }

    // Convert git remote URL to GitHub HTTPS URL
    let github_base = if remote_url.starts_with("git@github.com:") {
        // git@github.com:user/repo.git -> https://github.com/user/repo
        let path = remote_url.trim_start_matches("git@github.com:");
        let path = path.trim_end_matches(".git");
        format!("https://github.com/{path}")
    } else if remote_url.starts_with("https://github.com/") {
        // https://github.com/user/repo.git -> https://github.com/user/repo
        remote_url.trim_end_matches(".git").to_string()
    } else {
        return None; // Not a GitHub repo
    };

    Some(format!("{github_base}/tree/{branch}"))
}

/// Open and render a document file (markdown or PDF).
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
    let path_buf = path_buf
        .canonicalize()
        .map_err(|e| format!("Failed to resolve path: {e}"))?;

    let absolute_path = path_buf.to_string_lossy().to_string();

    // Detect document type
    let doc_type = DocumentType::from_path(&path_buf)
        .ok_or_else(|| format!("Unsupported file type: {}", path_buf.display()))?;

    // Get file modification time
    let modified_at = tokio::fs::metadata(&path_buf)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);

    // Get git metadata (applies to all document types)
    let git_root = find_git_root(&absolute_path);
    let git_branch = get_git_branch(&absolute_path);
    let git_branch_url = git_branch
        .as_ref()
        .and_then(|b| get_git_branch_url(&absolute_path, b));
    let git_last_author = get_git_last_author(&absolute_path);

    // Update state
    {
        let mut state = state.write().await;
        state.current_file = Some(path_buf.clone());
        state.add_recent_file(path_buf);
    }

    // Handle based on document type
    match doc_type {
        DocumentType::Pdf => {
            // PDF: Return file URL for frontend PDF.js viewer
            // Stats will be computed by frontend after PDF loads
            let stats = calculate_pdf_stats(None); // No page count yet
            Ok(RenderResponse::pdf(absolute_path.clone(), stats)
                .with_file_info(Some(absolute_path), modified_at)
                .with_git_metadata(git_root, git_branch, git_branch_url, git_last_author))
        }
        DocumentType::Markdown => {
            // Markdown: Read, render, and return HTML
            let content = tokio::fs::read_to_string(&absolute_path)
                .await
                .map_err(|e| format!("Failed to read file: {e}"))?;

            let result = to_html(&content, &RenderOptions::gui()).map_err(|e| e.to_string())?;
            let stats = calculate_document_stats(&content);

            Ok(RenderResponse::from_markdown(result, stats)
                .with_file_info(Some(absolute_path), modified_at)
                .with_git_metadata(git_root, git_branch, git_branch_url, git_last_author))
        }
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

            // Read and render the file
            match tokio::fs::read_to_string(&path_clone).await {
                Ok(content) => {
                    if let Ok(result) = to_html(&content, &RenderOptions::gui()) {
                        let stats = calculate_document_stats(&content);
                        let git_root = find_git_root(&path_clone);
                        let git_branch = get_git_branch(&path_clone);
                        let git_branch_url = git_branch
                            .as_ref()
                            .and_then(|b| get_git_branch_url(&path_clone, b));
                        let git_last_author = get_git_last_author(&path_clone);
                        let modified_at = tokio::fs::metadata(&path_clone)
                            .await
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_millis() as u64);

                        let response = RenderResponse::from_markdown(result, stats)
                            .with_file_info(Some(path_clone.clone()), modified_at)
                            .with_git_metadata(
                                git_root,
                                git_branch,
                                git_branch_url,
                                git_last_author,
                            );

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
