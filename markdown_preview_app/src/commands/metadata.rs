//! File metadata, preview info, title extraction, and supported extensions commands.

use super::file::{get_git_branch, get_git_branch_url, get_git_last_author};
use crate::state::AppState;
use markdown_preview_core::{calculate_document_stats, DocumentStats, DocumentType};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

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

/// File preview info for tooltips.
#[derive(Clone, serde::Serialize)]
pub struct FilePreviewInfo {
    /// Document title (from frontmatter or H1 heading)
    pub title: Option<String>,
    /// First paragraph digest for preview
    pub digest: Option<String>,
    /// File modification time (Unix timestamp in milliseconds)
    pub modified_at: Option<u64>,
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

/// Extract a multi-line digest showing the document structure (headings + paragraphs).
///
/// Returns lines joined by `\n`. Heading lines are prefixed with `# ` so the
/// frontend can style them differently from paragraph text.
fn extract_digest(content: &str, max_lines: usize, max_chars: usize) -> Option<String> {
    let mut in_frontmatter = false;
    let mut in_code_block = false;
    let mut frontmatter_delimiter_count = 0;
    let mut lines: Vec<String> = Vec::new();
    let mut total_chars = 0;
    let mut found_first_heading = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Handle YAML frontmatter (---)
        if trimmed == "---" {
            frontmatter_delimiter_count += 1;
            if frontmatter_delimiter_count == 1 {
                in_frontmatter = true;
                continue;
            } else if frontmatter_delimiter_count == 2 {
                in_frontmatter = false;
                continue;
            }
        }

        if in_frontmatter {
            continue;
        }

        // Handle code blocks
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            continue;
        }

        // Skip empty lines, HTML comments
        if trimmed.is_empty() || trimmed.starts_with("<!--") {
            continue;
        }

        // Skip blockquotes (often used for alerts)
        if trimmed.starts_with('>') {
            continue;
        }

        // Sub-headings (## and deeper) — keep as structural markers
        // Skip the top-level `# Title` since it duplicates the title field
        if trimmed.starts_with('#') {
            let heading_text = trimmed.trim_start_matches('#').trim();
            if !found_first_heading {
                // Skip first heading (usually the document title shown separately)
                found_first_heading = true;
                continue;
            }
            if heading_text.is_empty() {
                continue;
            }
            let entry = format!("# {heading_text}");
            total_chars += entry.len();
            lines.push(entry);
            if lines.len() >= max_lines || total_chars >= max_chars {
                break;
            }
            continue;
        }

        // Skip list items
        if trimmed.starts_with('-')
            || trimmed.starts_with('*')
            || trimmed.starts_with('+')
            || trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            continue;
        }

        // Paragraph text — clean markdown formatting
        let cleaned = trimmed
            .replace(['[', ']'], "")
            .replace("**", "")
            .replace("__", "")
            .replace('*', "")
            .replace('_', " ");

        if cleaned.is_empty() {
            continue;
        }

        // Truncate long paragraphs at word boundary
        let remaining = max_chars.saturating_sub(total_chars);
        let entry = if cleaned.len() > remaining {
            let truncated: String = cleaned.chars().take(remaining).collect();
            if let Some(last_space) = truncated.rfind(' ') {
                format!("{}...", &truncated[..last_space])
            } else {
                format!("{truncated}...")
            }
        } else {
            cleaned
        };

        total_chars += entry.len();
        lines.push(entry);
        if lines.len() >= max_lines || total_chars >= max_chars {
            break;
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// Extract title from markdown content.
///
/// Looks for title in YAML frontmatter or first H1 heading.
fn extract_markdown_title(content: &str) -> Option<String> {
    // Limit to first 2000 chars for performance
    let content = if content.len() > 2000 {
        &content[..2000]
    } else {
        content
    };

    // Track content after frontmatter
    let content_after_frontmatter;

    // Try YAML frontmatter first
    if let Some(after_prefix) = content.strip_prefix("---") {
        if let Some(end_idx) = after_prefix.find("---") {
            let frontmatter = &after_prefix[..end_idx];
            for line in frontmatter.lines() {
                let line = line.trim();
                if let Some(title) = line.strip_prefix("title:") {
                    let title = title.trim();
                    // Remove quotes if present
                    let title = title
                        .strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                        .or_else(|| title.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                        .unwrap_or(title);
                    if !title.is_empty() {
                        return Some(title.to_string());
                    }
                }
            }
            // Skip past frontmatter for H1 search
            content_after_frontmatter = &after_prefix[end_idx + 3..];
        } else {
            content_after_frontmatter = content;
        }
    } else {
        content_after_frontmatter = content;
    }

    // Try first H1 heading (after frontmatter if present)
    for line in content_after_frontmatter.lines() {
        let line = line.trim();
        // Skip empty lines
        if line.is_empty() {
            continue;
        }
        // Check for H1 heading
        if let Some(title) = line.strip_prefix("# ") {
            let title = title.trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
        // Stop after first non-empty, non-heading line (title should be at the top)
        if !line.starts_with('#') {
            break;
        }
    }

    None
}

/// Extract title from PDF metadata.
fn get_pdf_title(path: &std::path::Path) -> Option<String> {
    use lopdf::Document;

    let doc = Document::load(path).ok()?;

    // Try to get the Info dictionary from the trailer
    let info_ref = doc.trailer.get(b"Info").ok()?;
    let info_obj = doc.get_object(info_ref.as_reference().ok()?).ok()?;
    let info_dict = info_obj.as_dict().ok()?;

    // Get the Title field
    let title_obj = info_dict.get(b"Title").ok()?;

    // Handle different string encodings in PDF
    match title_obj {
        lopdf::Object::String(bytes, _) => {
            // Try UTF-16 BE (starts with BOM 0xFE 0xFF)
            if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                let utf16: Vec<u16> = bytes[2..]
                    .chunks(2)
                    .filter_map(|chunk| {
                        if chunk.len() == 2 {
                            Some(u16::from_be_bytes([chunk[0], chunk[1]]))
                        } else {
                            None
                        }
                    })
                    .collect();
                String::from_utf16(&utf16).ok()
            } else {
                // Try as UTF-8 or Latin-1
                String::from_utf8(bytes.clone())
                    .ok()
                    .or_else(|| Some(bytes.iter().map(|&b| b as char).collect()))
            }
        }
        _ => None,
    }
    .filter(|s| !s.trim().is_empty())
}

/// Get file preview info (title, digest, and modification time) for tooltip display.
#[tauri::command]
pub async fn get_file_preview_info(
    path: String,
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<FilePreviewInfo, String> {
    let path_buf = std::path::Path::new(&path);

    // Get modification time
    let modified_at = tokio::fs::metadata(path_buf)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);

    // Check for cached AI summary (mtime must match)
    let ai_digest = modified_at.and_then(|mtime| {
        // We need a blocking read to check the cache synchronously.
        // Use try_read to avoid blocking the async runtime.
        let state_guard = state.try_read().ok()?;
        state_guard.get_ai_summary(&path, mtime).map(String::from)
    });

    // Get title and digest based on document type
    let (title, digest) = match DocumentType::from_path(path_buf) {
        Some(DocumentType::Markdown) => {
            let content = tokio::fs::read_to_string(path_buf).await.ok();
            let title = if let Some(ref content) = content {
                extract_markdown_title(content)
            } else {
                None
            };
            // Prefer AI summary over text-based digest
            let digest =
                ai_digest.or_else(|| content.as_ref().and_then(|c| extract_digest(c, 12, 1000)));
            (title, digest)
        }
        Some(DocumentType::Pdf) => (get_pdf_title(path_buf), ai_digest),
        None => (None, ai_digest),
    };

    Ok(FilePreviewInfo {
        title,
        digest,
        modified_at,
    })
}

/// Extract the title from a markdown file (legacy command, use get_file_preview_info instead).
#[tauri::command]
pub async fn get_markdown_title(path: String) -> Result<Option<String>, String> {
    let path_buf = std::path::Path::new(&path);
    if DocumentType::from_path(path_buf) != Some(DocumentType::Markdown) {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(path_buf)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;
    Ok(extract_markdown_title(&content))
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

/// AI configuration response for the frontend.
#[derive(Clone, serde::Serialize)]
pub struct AiConfigResponse {
    /// Provider name ("ollama", "anthropic", "openai") or null if disabled.
    pub provider: Option<String>,
    /// Model override or null for provider default.
    pub model: Option<String>,
}

/// Get the current AI summarization configuration.
#[tauri::command]
pub async fn get_ai_config(
    state: State<'_, Arc<RwLock<AppState>>>,
) -> Result<AiConfigResponse, String> {
    let state_guard = state.read().await;
    Ok(AiConfigResponse {
        provider: state_guard.ai_provider().map(String::from),
        model: state_guard.ai_model().map(String::from),
    })
}

/// Set the AI summarization configuration (legacy — use set_app_config instead).
///
/// Only updates provider and model. Preserves other config fields.
/// Clears AI summary cache and triggers re-summarization if changed.
#[tauri::command]
pub async fn set_ai_config(
    provider: Option<String>,
    model: Option<String>,
    state: State<'_, Arc<RwLock<AppState>>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let ai_changed = {
        let mut state_guard = state.write().await;
        state_guard.set_ai_config(provider, model)
    };

    if ai_changed {
        let state_arc = state.inner().clone();
        tauri::async_runtime::spawn(async move {
            crate::generate_recent_file_summaries(state_arc, app_handle).await;
        });
    }

    Ok(())
}

/// Unified application configuration for the Settings dialog.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    /// GitHub personal access token.
    pub github_token: Option<String>,
    /// AI provider name ("ollama", "anthropic", "openai") or null if disabled.
    pub ai_provider: Option<String>,
    /// AI model override or null for provider default.
    pub ai_model: Option<String>,
    /// AI API key for Anthropic/OpenAI.
    pub ai_api_key: Option<String>,
    /// Ollama URL override.
    pub ollama_url: Option<String>,
}

/// Get the full application configuration for the Settings dialog.
#[tauri::command]
pub async fn get_app_config(state: State<'_, Arc<RwLock<AppState>>>) -> Result<AppConfig, String> {
    let state_guard = state.read().await;
    Ok(AppConfig {
        github_token: state_guard.github_token().map(String::from),
        ai_provider: state_guard.ai_provider().map(String::from),
        ai_model: state_guard.ai_model().map(String::from),
        ai_api_key: state_guard.ai_api_key().map(String::from),
        ollama_url: state_guard.ollama_url().map(String::from),
    })
}

/// Set the full application configuration from the Settings dialog.
///
/// Normalizes all inputs, clears AI summary cache if AI config changed,
/// and triggers background re-summarization.
#[tauri::command]
pub async fn set_app_config(
    config: AppConfig,
    state: State<'_, Arc<RwLock<AppState>>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let ai_changed = {
        let mut state_guard = state.write().await;
        state_guard.set_app_config(
            config.ai_provider,
            config.ai_model,
            config.ai_api_key,
            config.ollama_url,
            config.github_token,
        )
    };

    if ai_changed {
        let state_arc = state.inner().clone();
        tauri::async_runtime::spawn(async move {
            crate::generate_recent_file_summaries(state_arc, app_handle).await;
        });
    }

    Ok(())
}
