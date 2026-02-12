//! Tauri IPC commands for the markdown preview app.

pub mod clipboard;
pub mod diff;
pub mod file;
pub mod git;
pub mod metadata;
pub mod path;
pub mod recent;
pub mod render;
pub mod terminal;
pub mod url;

use markdown_preview_core::{DocumentStats, DocumentType, RenderOutput};

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
