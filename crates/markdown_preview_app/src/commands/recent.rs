//! Recent files commands.

use crate::state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

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

/// Remove a specific file from the recent files list.
#[tauri::command]
pub async fn remove_recent_file(
    path: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<(), String> {
    let mut state = state.write().await;
    state.remove_recent_file(std::path::Path::new(&path));
    Ok(())
}
