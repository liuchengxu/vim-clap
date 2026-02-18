//! SSH file open, watch, and completion commands.

use super::path::PathCompletion;
use super::RenderResponse;
use crate::ssh::{self, SshTarget};
use crate::state::AppState;
use markdown_preview_core::{calculate_document_stats, to_html, DocumentType, RenderOptions};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::RwLock;

/// Collect remote git metadata for an SSH target (all failures are non-fatal).
async fn collect_ssh_git_metadata(
    target: &SshTarget,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let parent = std::path::Path::new(&target.path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string());

    let git_root_val = ssh::git_root(target.user.as_deref(), &target.host, &parent).await;

    let git_branch_val = ssh::git_branch(target.user.as_deref(), &target.host, &parent).await;

    let git_branch_url_val =
        if let (Some(ref branch), Some(ref _root)) = (&git_branch_val, &git_root_val) {
            if let Some(remote_url) =
                ssh::git_remote_url(target.user.as_deref(), &target.host, &parent).await
            {
                ssh::git_branch_url_from_remote(&remote_url, branch)
            } else {
                None
            }
        } else {
            None
        };

    let git_last_author_val = if git_root_val.is_some() {
        ssh::git_last_author(target.user.as_deref(), &target.host, &parent, &target.path).await
    } else {
        None
    };

    (
        git_root_val,
        git_branch_val,
        git_branch_url_val,
        git_last_author_val,
    )
}

/// Open and render a markdown file from a remote SSH host.
#[tauri::command]
pub async fn open_ssh_file(
    path: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<RenderResponse, String> {
    let target = ssh::parse_ssh_path(&path).ok_or_else(|| format!("Invalid SSH path: {path}"))?;

    // Read remote file
    let content = ssh::read_file(target.user.as_deref(), &target.host, &target.path).await?;

    // Render markdown
    let result = to_html(&content, &RenderOptions::gui())
        .map_err(|e| format!("Failed to render markdown: {e}"))?;
    let stats = calculate_document_stats(&content);

    // Get remote mtime (non-fatal)
    let modified_at = ssh::stat_mtime(target.user.as_deref(), &target.host, &target.path)
        .await
        .ok();

    // Collect remote git metadata
    let (git_root, git_branch, git_branch_url, git_last_author) =
        collect_ssh_git_metadata(&target).await;

    // Update state
    {
        let mut state_guard = state.write().await;
        state_guard.current_file = None;
        state_guard.current_ssh_path = Some(path.clone());
        state_guard.add_recent_file(PathBuf::from(&path));
    }

    Ok(RenderResponse::from_markdown(result, stats)
        .with_file_info(Some(path), modified_at)
        .with_git_metadata(git_root, git_branch, git_branch_url, git_last_author))
}

/// Complete a partial SSH path by listing remote directory contents.
#[tauri::command]
pub async fn complete_ssh_path(partial: String) -> Result<Vec<PathCompletion>, String> {
    let partial = partial.trim();
    if partial.is_empty() {
        return Ok(Vec::new());
    }

    // We need at least a host and colon to complete
    let colon_pos = match partial.find(':') {
        Some(pos) => pos,
        None => return Ok(Vec::new()),
    };

    let user_host = &partial[..colon_pos];
    let remote_part = &partial[colon_pos + 1..];

    // Parse user@host
    let (user, host) = if let Some(at_pos) = user_host.find('@') {
        (Some(&user_host[..at_pos]), &user_host[at_pos + 1..])
    } else {
        (None, user_host)
    };

    // Remote path must start with / or ~/
    if !remote_part.starts_with('/') && !remote_part.starts_with("~/") {
        return Ok(Vec::new());
    }

    // Determine directory to list and prefix to filter
    let (dir_to_list, prefix) = if remote_part.ends_with('/') {
        (remote_part.to_string(), String::new())
    } else {
        let remote_path = std::path::Path::new(remote_part);
        let parent = remote_path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "~/".to_string());
        let file_name = remote_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        (
            if parent.ends_with('/') {
                parent
            } else {
                format!("{parent}/")
            },
            file_name,
        )
    };

    // List remote directory
    let entries = ssh::list_dir(user, host, &dir_to_list).await?;

    let prefix_lower = prefix.to_lowercase();
    let ssh_prefix = &partial[..=colon_pos]; // "user@host:" or "host:"

    let mut completions: Vec<PathCompletion> = entries
        .into_iter()
        .filter(|entry| {
            let name = entry.trim_end_matches('/');
            // Skip hidden files unless prefix starts with dot
            if name.starts_with('.') && !prefix.starts_with('.') {
                return false;
            }
            // Filter by prefix
            if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix_lower) {
                return false;
            }
            let is_dir = entry.ends_with('/');
            if is_dir {
                return true;
            }
            // Only include supported document types
            DocumentType::from_path(std::path::Path::new(name)).is_some()
        })
        .map(|entry| {
            let is_dir = entry.ends_with('/');
            let full_path = format!("{ssh_prefix}{dir_to_list}{entry}");
            PathCompletion {
                path: full_path,
                name: entry,
                is_dir,
            }
        })
        .collect();

    // Sort: directories first, then alphabetically
    completions.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    completions.truncate(20);

    Ok(completions)
}

/// Start polling a remote SSH file for changes.
#[tauri::command]
pub async fn watch_ssh_file(
    path: String,
    window: tauri::Window,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<(), String> {
    let target = ssh::parse_ssh_path(&path).ok_or_else(|| format!("Invalid SSH path: {path}"))?;

    // Stop any existing watcher
    {
        let mut state_guard = state.write().await;
        if let Some(handle) = state_guard.watcher_handle.take() {
            handle.abort();
        }
    }

    let window_clone = window.clone();
    let path_clone = path.clone();

    let handle = tokio::spawn(async move {
        let mut last_mtime: Option<u64> = None;
        let mut interval_secs: u64 = 3;
        let mut consecutive_failures: u32 = 0;
        let mut error_emitted = false;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;

            // Check mtime
            match ssh::stat_mtime(target.user.as_deref(), &target.host, &target.path).await {
                Ok(mtime) => {
                    // Recovery: reset backoff
                    if consecutive_failures > 0 {
                        tracing::info!(path = %path_clone, "SSH watcher recovered");
                        if error_emitted {
                            let _ = window_clone.emit("ssh-watch-recovered", ());
                            error_emitted = false;
                        }
                    }
                    consecutive_failures = 0;
                    interval_secs = 3;

                    let changed = last_mtime.is_some() && last_mtime != Some(mtime);
                    last_mtime = Some(mtime);

                    if changed {
                        tracing::debug!(path = %path_clone, "SSH file changed, reloading");

                        match ssh::read_file(target.user.as_deref(), &target.host, &target.path)
                            .await
                        {
                            Ok(content) => {
                                if let Ok(result) = to_html(&content, &RenderOptions::gui()) {
                                    let stats = calculate_document_stats(&content);
                                    let (git_root, git_branch, git_branch_url, git_last_author) =
                                        collect_ssh_git_metadata(&target).await;

                                    let response = RenderResponse::from_markdown(result, stats)
                                        .with_file_info(Some(path_clone.clone()), Some(mtime))
                                        .with_git_metadata(
                                            git_root,
                                            git_branch,
                                            git_branch_url,
                                            git_last_author,
                                        );

                                    let _ = window_clone.emit("file-changed", response);
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    "Failed to read SSH file after change"
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::warn!(
                        error = %e,
                        failures = consecutive_failures,
                        "SSH watcher stat failed"
                    );

                    // Exponential backoff: 3 -> 6 -> 12 -> 24 -> cap at 30
                    interval_secs = (interval_secs * 2).min(30);

                    // After 3 consecutive failures, emit a single error event
                    if consecutive_failures >= 3 && !error_emitted {
                        let _ = window_clone.emit("ssh-watch-error", e);
                        error_emitted = true;
                    }
                }
            }
        }
    });

    // Store the handle
    {
        let mut state_guard = state.write().await;
        state_guard.watcher_handle = Some(handle);
    }

    Ok(())
}
