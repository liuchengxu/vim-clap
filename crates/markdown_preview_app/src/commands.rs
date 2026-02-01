//! Tauri IPC commands for the markdown preview app.

use crate::state::AppState;
use markdown_preview_core::{
    calculate_document_stats, calculate_pdf_stats, find_git_root, to_html, DocumentStats,
    DocumentType, RenderOptions, RenderOutput,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::RwLock;

/// Result of rendering a document, sent to the frontend.
///
/// Maintains backward compatibility: `html` field always present for markdown.
/// New consumers should use the `output` field which provides type-safe access
/// to rendered content.
#[derive(Clone, serde::Serialize)]
pub struct RenderResponse {
    // === Legacy fields (always present for markdown) ===
    /// HTML content (for backward compatibility with existing frontend code).
    pub html: String,

    // === New fields ===
    /// Document type that was rendered. Always present.
    pub document_type: DocumentType,

    /// Generic render output (new consumers should use this).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<RenderOutput>,

    // === Existing metadata fields (unchanged) ===
    /// Document statistics
    pub stats: DocumentStats,
    /// Git repository root (if applicable)
    pub git_root: Option<String>,
    /// Path to the rendered file
    pub file_path: Option<String>,
    /// File modification time (Unix timestamp in milliseconds)
    pub modified_at: Option<u64>,
    /// Git branch name
    pub git_branch: Option<String>,
    /// GitHub URL for the branch
    pub git_branch_url: Option<String>,
    /// Last commit author for this file
    pub git_last_author: Option<String>,
}

impl RenderResponse {
    /// Create response for markdown from render result (preserves line_map for scroll sync).
    pub fn from_markdown(
        result: markdown_preview_core::RenderResult,
        stats: DocumentStats,
    ) -> Self {
        Self {
            html: result.html.clone(), // Legacy field for backward compatibility
            document_type: DocumentType::Markdown,
            output: Some(result.into_render_output()), // Preserves line_map
            stats,
            git_root: None,
            file_path: None,
            modified_at: None,
            git_branch: None,
            git_branch_url: None,
            git_last_author: None,
        }
    }

    /// Create response for PDF (file path for frontend).
    pub fn pdf(path: String, stats: DocumentStats) -> Self {
        Self {
            html: String::new(), // Empty for non-HTML
            document_type: DocumentType::Pdf,
            output: Some(RenderOutput::file_url(path, "application/pdf")),
            stats,
            git_root: None,
            file_path: None,
            modified_at: None,
            git_branch: None,
            git_branch_url: None,
            git_last_author: None,
        }
    }

    /// Set git metadata on the response.
    pub fn with_git_metadata(
        mut self,
        git_root: Option<String>,
        git_branch: Option<String>,
        git_branch_url: Option<String>,
        git_last_author: Option<String>,
    ) -> Self {
        self.git_root = git_root;
        self.git_branch = git_branch;
        self.git_branch_url = git_branch_url;
        self.git_last_author = git_last_author;
        self
    }

    /// Set file path and modification time on the response.
    pub fn with_file_info(mut self, file_path: Option<String>, modified_at: Option<u64>) -> Self {
        self.file_path = file_path;
        self.modified_at = modified_at;
        self
    }
}

/// Render markdown content to HTML.
#[tauri::command]
pub async fn render_markdown(content: String) -> Result<RenderResponse, String> {
    let result = to_html(&content, &RenderOptions::gui()).map_err(|e| e.to_string())?;

    let stats = calculate_document_stats(&content);

    Ok(RenderResponse::from_markdown(result, stats))
}

/// Expand ~ to home directory for open_file.
fn expand_tilde(path: &str) -> PathBuf {
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
fn get_git_branch(file_path: &str) -> Option<String> {
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
fn get_git_last_author(file_path: &str) -> Option<String> {
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
fn get_git_branch_url(file_path: &str, branch: &str) -> Option<String> {
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
pub async fn clear_recent_files(state: State<'_, Arc<RwLock<AppState>>>) -> Result<(), String> {
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

/// Check clipboard for a markdown file path and return it if valid.
#[tauri::command]
pub async fn check_clipboard_for_markdown(app: tauri::AppHandle) -> Result<Option<String>, String> {
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

    // Check if it looks like a file path and is a supported document
    if path.is_absolute() && path.exists() {
        if let Some(doc_type) = DocumentType::from_path(path) {
            tracing::debug!(doc_type = ?doc_type, "Found supported document type");
            tracing::info!(path = %text, "Found supported document in clipboard");
            return Ok(Some(text.to_string()));
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

/// Get the git root directory for the current file.
#[tauri::command]
pub async fn get_current_git_root(
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<Option<String>, String> {
    let state = state.read().await;
    if let Some(ref current_file) = state.current_file {
        let path_str = current_file.to_string_lossy().to_string();
        Ok(find_git_root(&path_str))
    } else {
        Ok(None)
    }
}

/// File metadata response (subset of RenderResponse for metadata-only updates).
#[derive(Clone, serde::Serialize)]
pub struct FileMetadata {
    /// File modification time (Unix timestamp in milliseconds)
    pub modified_at: Option<u64>,
    /// Document statistics
    pub stats: DocumentStats,
    /// Git branch name
    pub git_branch: Option<String>,
    /// GitHub URL for the branch
    pub git_branch_url: Option<String>,
    /// Last commit author for this file
    pub git_last_author: Option<String>,
}

/// Refresh metadata for the currently open file.
/// Returns updated modification time, git info, and document stats.
#[tauri::command]
pub async fn refresh_file_metadata(
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<Option<FileMetadata>, String> {
    let state = state.read().await;
    let Some(ref current_file) = state.current_file else {
        return Ok(None);
    };

    let path_str = current_file.to_string_lossy().to_string();

    // Get file modification time
    let modified_at = tokio::fs::metadata(current_file)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);

    // Read the file to get updated stats
    let content = tokio::fs::read_to_string(current_file)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let stats = calculate_document_stats(&content);
    let git_branch = get_git_branch(&path_str);
    let git_branch_url = git_branch
        .as_ref()
        .and_then(|b| get_git_branch_url(&path_str, b));
    let git_last_author = get_git_last_author(&path_str);

    Ok(Some(FileMetadata {
        modified_at,
        stats,
        git_branch,
        git_branch_url,
        git_last_author,
    }))
}

/// Parsed GitHub URL components.
struct GitHubUrl {
    owner: String,
    repo: String,
    git_ref: String,
    path: String,
}

/// Parse a GitHub blob URL into its components.
/// Example: https://github.com/owner/repo/blob/branch/path/to/file.md
fn parse_github_url(url: &str) -> Option<GitHubUrl> {
    let url = url.strip_prefix("https://github.com/")?;

    // Split by /blob/
    let (repo_part, path_part) = url.split_once("/blob/")?;

    // repo_part = "owner/repo"
    let (owner, repo) = repo_part.split_once('/')?;

    // path_part = "branch/path/to/file.md" or "commit_sha/path/to/file.md"
    let (git_ref, path) = path_part.split_once('/')?;

    Some(GitHubUrl {
        owner: owner.to_string(),
        repo: repo.to_string(),
        git_ref: git_ref.to_string(),
        path: path.to_string(),
    })
}

/// Convert a GitHub URL to raw content URL (for public repos).
fn convert_to_raw_github_url(url: &str) -> Option<String> {
    // Already a raw URL
    if url.starts_with("https://raw.githubusercontent.com/") {
        return Some(url.to_string());
    }

    // Convert blob URL to raw URL
    if url.starts_with("https://github.com/") && url.contains("/blob/") {
        let raw_url = url
            .replace("https://github.com/", "https://raw.githubusercontent.com/")
            .replace("/blob/", "/");
        return Some(raw_url);
    }

    None
}

/// Get GitHub token from environment.
fn get_github_token() -> Option<String> {
    std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .ok()
        .filter(|t| !t.is_empty())
}

/// Fetch content from GitHub API (works for private repos with token).
async fn fetch_github_api(github_url: &GitHubUrl, token: &str) -> Result<String, String> {
    // GitHub API endpoint for file contents
    let api_url = format!(
        "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
        github_url.owner, github_url.repo, github_url.path, github_url.git_ref
    );

    tracing::info!(api_url = %api_url, "Fetching from GitHub API");

    let client = reqwest::Client::new();
    let response = client
        .get(&api_url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github.raw+json")
        .header("User-Agent", "markdown-preview-app")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch from GitHub API: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("GitHub API error: HTTP {status} - {body}"));
    }

    response
        .text()
        .await
        .map_err(|e| format!("Failed to read GitHub API response: {e}"))
}

/// Fetch and render markdown from a URL.
#[tauri::command]
pub async fn open_url(url: String) -> Result<RenderResponse, String> {
    tracing::info!(url = %url, "Opening URL");

    let content = if let Some(github_url) = parse_github_url(&url) {
        // It's a GitHub URL - try API first if we have a token
        if let Some(token) = get_github_token() {
            tracing::info!("Using GitHub API with token");
            match fetch_github_api(&github_url, &token).await {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!(error = %e, "GitHub API failed, trying raw URL");
                    // Fall back to raw URL
                    fetch_raw_url(&url).await?
                }
            }
        } else {
            // No token, use raw URL (public repos only)
            fetch_raw_url(&url).await?
        }
    } else {
        // Not a GitHub URL, fetch directly
        fetch_raw_url(&url).await?
    };

    // Render the markdown
    let result = to_html(&content, &RenderOptions::gui()).map_err(|e| e.to_string())?;
    let stats = calculate_document_stats(&content);

    Ok(RenderResponse::from_markdown(result, stats).with_file_info(Some(url), None))
}

/// Fetch content from a URL (with GitHub raw URL conversion).
async fn fetch_raw_url(url: &str) -> Result<String, String> {
    let fetch_url = convert_to_raw_github_url(url).unwrap_or_else(|| url.to_string());

    tracing::info!(fetch_url = %fetch_url, "Fetching raw URL");

    let response = reqwest::get(&fetch_url)
        .await
        .map_err(|e| format!("Failed to fetch URL: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        if status.as_u16() == 404 && url.contains("github.com") {
            return Err(format!(
                "HTTP {status}: File not found. If this is a private repo, set GITHUB_TOKEN or GH_TOKEN environment variable."
            ));
        }
        return Err(format!("Failed to fetch URL: HTTP {status}"));
    }

    response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))
}

/// Supported file extensions response.
///
/// Contains both grouped (by document type) and flat list for convenience.
#[derive(Clone, serde::Serialize)]
pub struct SupportedExtensions {
    /// Extensions grouped by document type name.
    pub by_type: HashMap<String, Vec<String>>,
    /// All supported extensions as a flat list.
    pub all: Vec<String>,
}

/// Returns supported file extensions grouped by document type.
///
/// This is the single source of truth - frontend fetches this on init.
#[tauri::command]
pub fn get_supported_extensions() -> SupportedExtensions {
    let by_type: HashMap<String, Vec<String>> = DocumentType::ALL
        .iter()
        .map(|doc_type| {
            (
                doc_type.name().to_string(),
                doc_type
                    .extensions()
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            )
        })
        .collect();

    let all: Vec<String> = DocumentType::all_extensions()
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    SupportedExtensions { by_type, all }
}
