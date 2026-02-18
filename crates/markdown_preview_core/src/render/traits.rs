//! Renderer traits for different document types.
//!
//! This module defines the [`TextRenderer`] trait that provides a common
//! interface for rendering text-based document formats.

use crate::document::DocumentType;
use crate::render::output::RenderOutput;
use crate::stats::DocumentStats;

/// Error type for rendering operations.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// I/O error during file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid UTF-8 encoding in content.
    #[error("Invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// Unsupported document type for the renderer.
    #[error("Unsupported document type: {0:?}")]
    UnsupportedType(DocumentType),

    /// General rendering error.
    #[error("Render error: {0}")]
    Other(String),
}

/// Trait for text-based document renderers (markdown, etc.).
///
/// Text renderers process UTF-8 content and produce HTML output.
/// They support line mapping for scroll synchronization.
pub trait TextRenderer: Send + Sync {
    /// Render text content to output.
    ///
    /// # Arguments
    ///
    /// * `content` - The UTF-8 text content to render
    ///
    /// # Returns
    ///
    /// Returns [`RenderOutput::Html`] with the rendered content and optional line map.
    fn render_text(&self, content: &str) -> Result<RenderOutput, RenderError>;

    /// Calculate statistics from text content.
    fn calculate_text_stats(&self, content: &str) -> DocumentStats;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_error_display() {
        let io_err = RenderError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(io_err.to_string().contains("IO error"));

        let other_err = RenderError::Other("custom error".to_string());
        assert_eq!(other_err.to_string(), "Render error: custom error");

        let unsupported = RenderError::UnsupportedType(DocumentType::Pdf);
        assert!(unsupported.to_string().contains("Unsupported"));
    }
}
