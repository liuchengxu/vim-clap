//! Path completion and history commands.

use crate::state::AppState;
use markdown_preview_core::DocumentType;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

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
    if let Some(stripped) = partial.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(stripped)
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
    }
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
    let (dir_to_list, prefix, use_tilde) =
        if partial.ends_with('/') || partial.ends_with(std::path::MAIN_SEPARATOR) {
            // User typed a complete directory path, list its contents
            (expanded.clone(), String::new(), partial.starts_with('~'))
        } else if expanded.is_dir() {
            // Path is a directory without trailing slash, list its contents
            (expanded.clone(), String::new(), partial.starts_with('~'))
        } else {
            // Partial filename - get parent directory and filter prefix
            let parent = expanded.parent().unwrap_or(Path::new("/"));
            let file_name = expanded
                .file_name()
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

                // Include directories and supported document files
                if is_dir {
                    completions.push(PathCompletion {
                        path: display_path,
                        name: format!("{name}/"),
                        is_dir: true,
                    });
                } else if DocumentType::from_path(&entry_path).is_some() {
                    completions.push(PathCompletion {
                        path: display_path,
                        name,
                        is_dir: false,
                    });
                }
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "Failed to read directory");
            return Ok(Vec::new());
        }
    }

    // Sort: directories first, then alphabetically
    completions.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    // Limit results
    completions.truncate(20);

    Ok(completions)
}

/// Get path history sorted by frecency.
/// If a git_root is provided, paths under it get a boost.
#[tauri::command]
pub async fn get_path_history(
    git_root: Option<String>,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<Vec<String>, String> {
    let state = state.read().await;
    Ok(state.get_path_history(git_root.as_deref()))
}

/// Add a path to the history.
#[tauri::command]
pub async fn add_path_to_history(
    path: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<(), String> {
    let mut state = state.write().await;
    state.add_path_to_history(path);
    Ok(())
}
