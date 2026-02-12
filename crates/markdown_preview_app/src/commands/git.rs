//! Git root detection command.

use crate::state::AppState;
use markdown_preview_core::find_git_root;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

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
