//! Git helper functions and commands.

use crate::state::AppState;
use markdown_preview_core::find_git_root;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

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

/// Get the git root directory for the current file (local or SSH).
#[tauri::command]
pub async fn get_current_git_root(
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<Option<String>, String> {
    let state_guard = state.read().await;

    // SSH path: query remote git root
    if let Some(ref ssh_path) = state_guard.current_ssh_path {
        if let Some(target) = crate::ssh::parse_ssh_path(ssh_path) {
            let parent = std::path::Path::new(&target.path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/".to_string());
            return Ok(crate::ssh::git_root(target.user.as_deref(), &target.host, &parent).await);
        }
    }

    // Local file
    if let Some(ref current_file) = state_guard.current_file {
        let path_str = current_file.to_string_lossy().to_string();
        Ok(find_git_root(&path_str))
    } else {
        Ok(None)
    }
}
