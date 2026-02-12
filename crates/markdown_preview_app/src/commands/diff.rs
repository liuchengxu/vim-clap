//! File diff commands.

use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// The kind of change in a diff.
#[derive(Clone, serde::Serialize)]
pub enum ChangeKind {
    /// Line was added
    Add,
    /// Line was removed
    Remove,
    /// Line is unchanged (context)
    Equal,
}

/// A single change in the diff.
#[derive(Clone, serde::Serialize)]
pub struct DiffChange {
    /// The kind of change
    pub kind: ChangeKind,
    /// The content of the line
    pub content: String,
}

/// Result of computing a file diff.
#[derive(Clone, serde::Serialize)]
pub struct FileDiff {
    /// Whether there are any changes
    pub has_changes: bool,
    /// When the previous snapshot was taken (ms since epoch)
    pub snapshot_time: u64,
    /// Current file modification time (ms since epoch)
    pub current_time: u64,
    /// Line-by-line changes
    pub changes: Vec<DiffChange>,
}

/// Get the diff between the current file content and the last snapshot.
///
/// This command:
/// 1. Reads the current file content
/// 2. Gets the previous snapshot from state (if any)
/// 3. Computes the diff if a snapshot exists
/// 4. Saves the current content as the new snapshot
/// 5. Returns the diff (or None if no previous snapshot)
#[tauri::command]
pub async fn get_file_diff(
    path: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<Option<FileDiff>, String> {
    use similar::{ChangeTag, TextDiff};

    // Read current file content
    let current_content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    // Get current file modification time
    let current_time = tokio::fs::metadata(&path)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Get previous snapshot and compute diff
    let diff_result = {
        let state_read = state.read().await;
        if let Some(snapshot) = state_read.get_snapshot(&path) {
            // Compute diff using similar crate
            let text_diff = TextDiff::from_lines(&snapshot.content, &current_content);

            let changes: Vec<DiffChange> = text_diff
                .iter_all_changes()
                .map(|change| {
                    let kind = match change.tag() {
                        ChangeTag::Delete => ChangeKind::Remove,
                        ChangeTag::Insert => ChangeKind::Add,
                        ChangeTag::Equal => ChangeKind::Equal,
                    };
                    DiffChange {
                        kind,
                        content: change.value().to_string(),
                    }
                })
                .collect();

            let has_changes = changes
                .iter()
                .any(|c| !matches!(c.kind, ChangeKind::Equal));

            Some(FileDiff {
                has_changes,
                snapshot_time: snapshot.timestamp,
                current_time,
                changes,
            })
        } else {
            None
        }
    };

    // Save current content as new snapshot
    {
        let mut state_write = state.write().await;
        state_write.save_snapshot(&path, &current_content);
    }

    Ok(diff_result)
}
