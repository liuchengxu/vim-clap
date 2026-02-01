//! Render output types for different document formats.
//!
//! This module provides the [`RenderOutput`] enum that represents the result
//! of rendering any document type. Different output types support different
//! rendering strategies (e.g., HTML for markdown, file URLs for PDFs).

use serde::{Deserialize, Serialize};

/// Rendered output from any document type.
///
/// This enum is serialized to JSON for transmission to the frontend.
/// The `type` field discriminator enables the frontend to handle
/// different output types appropriately.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RenderOutput {
    /// HTML content ready for innerHTML injection.
    Html {
        /// The rendered HTML content.
        content: String,
        /// Mapping from rendered element index to source line number (1-indexed).
        /// Used for scroll synchronization in vim-plugin mode.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        line_map: Vec<usize>,
    },
    /// File path for frontend to load directly (e.g., PDF.js).
    FileUrl {
        /// Filesystem path to the document.
        path: String,
        /// MIME type of the document.
        mime_type: String,
    },
}

impl RenderOutput {
    /// Create HTML output without line map.
    ///
    /// # Example
    ///
    /// ```
    /// use markdown_preview_core::RenderOutput;
    ///
    /// let output = RenderOutput::html("<p>Hello</p>".to_string());
    /// assert!(matches!(output, RenderOutput::Html { content, line_map } if line_map.is_empty()));
    /// ```
    pub fn html(content: String) -> Self {
        Self::Html {
            content,
            line_map: Vec::new(),
        }
    }

    /// Create HTML output with line map for scroll sync.
    ///
    /// The line map is used in vim-plugin mode to synchronize the preview
    /// scroll position with the editor cursor position.
    pub fn html_with_line_map(content: String, line_map: Vec<usize>) -> Self {
        Self::Html { content, line_map }
    }

    /// Create file URL output for frontend rendering.
    ///
    /// The frontend converts the filesystem path to a Tauri asset URL
    /// and handles the document rendering (e.g., using PDF.js for PDFs).
    pub fn file_url(path: String, mime_type: &str) -> Self {
        Self::FileUrl {
            path,
            mime_type: mime_type.to_string(),
        }
    }

    /// Get HTML content if this is HTML output.
    ///
    /// Returns `None` for non-HTML outputs like `FileUrl`.
    pub fn as_html(&self) -> Option<&str> {
        match self {
            Self::Html { content, .. } => Some(content),
            Self::FileUrl { .. } => None,
        }
    }

    /// Get the line map if this is HTML output with line tracking.
    pub fn line_map(&self) -> Option<&[usize]> {
        match self {
            Self::Html { line_map, .. } => Some(line_map),
            Self::FileUrl { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_output() {
        let output = RenderOutput::html("<p>test</p>".to_string());
        assert_eq!(output.as_html(), Some("<p>test</p>"));
        assert_eq!(output.line_map(), Some([].as_slice()));
    }

    #[test]
    fn test_html_with_line_map() {
        let output = RenderOutput::html_with_line_map("<p>test</p>".to_string(), vec![1, 5, 10]);
        assert_eq!(output.as_html(), Some("<p>test</p>"));
        assert_eq!(output.line_map(), Some([1, 5, 10].as_slice()));
    }

    #[test]
    fn test_file_url_output() {
        let output = RenderOutput::file_url("/path/to/doc.pdf".to_string(), "application/pdf");
        assert_eq!(output.as_html(), None);
        assert_eq!(output.line_map(), None);
        if let RenderOutput::FileUrl { path, mime_type } = output {
            assert_eq!(path, "/path/to/doc.pdf");
            assert_eq!(mime_type, "application/pdf");
        } else {
            panic!("Expected FileUrl variant");
        }
    }

    #[test]
    fn test_serde_html() {
        let output = RenderOutput::html("<p>test</p>".to_string());
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains(r#""type":"html""#));
        assert!(json.contains(r#""content":"<p>test</p>""#));
        // Empty line_map should be skipped
        assert!(!json.contains("line_map"));
    }

    #[test]
    fn test_serde_html_with_line_map() {
        let output = RenderOutput::html_with_line_map("<p>test</p>".to_string(), vec![1, 5]);
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains(r#""line_map":[1,5]"#));
    }

    #[test]
    fn test_serde_file_url() {
        let output = RenderOutput::file_url("/path/to/doc.pdf".to_string(), "application/pdf");
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains(r#""type":"file_url""#));
        assert!(json.contains(r#""path":"/path/to/doc.pdf""#));
        assert!(json.contains(r#""mime_type":"application/pdf""#));
    }
}
