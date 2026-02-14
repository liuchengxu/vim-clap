//! Clipboard checking command.

use markdown_preview_core::DocumentType;

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
