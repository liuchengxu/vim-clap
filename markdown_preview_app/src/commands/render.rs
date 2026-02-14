//! Markdown rendering command.

use super::RenderResponse;
use markdown_preview_core::{calculate_document_stats, to_html, RenderOptions};

/// Render markdown content to HTML.
#[tauri::command]
pub async fn render_markdown(content: String) -> Result<RenderResponse, String> {
    let result = to_html(&content, &RenderOptions::gui()).map_err(|e| e.to_string())?;

    let stats = calculate_document_stats(&content);

    Ok(RenderResponse::from_markdown(result, stats))
}
