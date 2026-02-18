//! Document type detection and classification.
//!
//! This module provides the [`DocumentType`] enum for identifying and working
//! with different document formats. It serves as the single source of truth
//! for supported file extensions.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;

/// Supported document types for preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentType {
    /// Markdown documents (.md, .markdown, etc.)
    Markdown,
    /// PDF documents (.pdf)
    Pdf,
}

/// Cached list of all supported extensions (avoids repeated allocations).
static ALL_EXTENSIONS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    DocumentType::ALL
        .iter()
        .flat_map(|doc_type| doc_type.extensions().iter().copied())
        .collect()
});

impl DocumentType {
    /// All supported document types.
    pub const ALL: &'static [DocumentType] = &[Self::Markdown, Self::Pdf];

    /// Detect document type from file extension (case-insensitive).
    ///
    /// Returns `None` for unknown extensions or empty input.
    ///
    /// # Examples
    ///
    /// ```
    /// use markdown_preview_core::DocumentType;
    ///
    /// assert_eq!(DocumentType::from_extension("md"), Some(DocumentType::Markdown));
    /// assert_eq!(DocumentType::from_extension("MD"), Some(DocumentType::Markdown));
    /// assert_eq!(DocumentType::from_extension("pdf"), Some(DocumentType::Pdf));
    /// assert_eq!(DocumentType::from_extension("txt"), None);
    /// assert_eq!(DocumentType::from_extension(""), None);
    /// ```
    pub fn from_extension(ext: &str) -> Option<Self> {
        if ext.is_empty() {
            return None;
        }
        let ext_lower = ext.to_ascii_lowercase();
        for doc_type in Self::ALL {
            if doc_type.extensions().iter().any(|e| *e == ext_lower) {
                return Some(*doc_type);
            }
        }
        None
    }

    /// Detect document type from file path.
    ///
    /// Returns `None` if path has no extension or extension is unknown.
    ///
    /// # Examples
    ///
    /// ```
    /// use markdown_preview_core::DocumentType;
    /// use std::path::Path;
    ///
    /// assert_eq!(DocumentType::from_path(Path::new("README.md")), Some(DocumentType::Markdown));
    /// assert_eq!(DocumentType::from_path(Path::new("document.pdf")), Some(DocumentType::Pdf));
    /// assert_eq!(DocumentType::from_path(Path::new("file")), None);
    /// ```
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// Get lowercase file extensions for this document type.
    ///
    /// This is the single source of truth for extension matching.
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Markdown => &["md", "markdown", "mdown", "mkdn", "mkd"],
            Self::Pdf => &["pdf"],
        }
    }

    /// Get all supported extensions across all document types.
    ///
    /// Returns a cached slice (no allocation on repeated calls).
    pub fn all_extensions() -> &'static [&'static str] {
        ALL_EXTENSIONS.as_slice()
    }

    /// Check if this is a text-based format (UTF-8 content).
    pub fn is_text_based(&self) -> bool {
        match self {
            Self::Markdown => true,
            Self::Pdf => false,
        }
    }

    /// Get the name of this document type as a string.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Pdf => "pdf",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension_markdown() {
        assert_eq!(
            DocumentType::from_extension("md"),
            Some(DocumentType::Markdown)
        );
        assert_eq!(
            DocumentType::from_extension("markdown"),
            Some(DocumentType::Markdown)
        );
        assert_eq!(
            DocumentType::from_extension("mdown"),
            Some(DocumentType::Markdown)
        );
        assert_eq!(
            DocumentType::from_extension("mkdn"),
            Some(DocumentType::Markdown)
        );
        assert_eq!(
            DocumentType::from_extension("mkd"),
            Some(DocumentType::Markdown)
        );
    }

    #[test]
    fn test_from_extension_case_insensitive() {
        assert_eq!(
            DocumentType::from_extension("MD"),
            Some(DocumentType::Markdown)
        );
        assert_eq!(
            DocumentType::from_extension("Md"),
            Some(DocumentType::Markdown)
        );
        assert_eq!(DocumentType::from_extension("PDF"), Some(DocumentType::Pdf));
    }

    #[test]
    fn test_from_extension_pdf() {
        assert_eq!(DocumentType::from_extension("pdf"), Some(DocumentType::Pdf));
    }

    #[test]
    fn test_from_extension_unknown() {
        assert_eq!(DocumentType::from_extension("txt"), None);
        assert_eq!(DocumentType::from_extension("docx"), None);
        assert_eq!(DocumentType::from_extension(""), None);
    }

    #[test]
    fn test_from_path() {
        assert_eq!(
            DocumentType::from_path(Path::new("README.md")),
            Some(DocumentType::Markdown)
        );
        assert_eq!(
            DocumentType::from_path(Path::new("/path/to/doc.pdf")),
            Some(DocumentType::Pdf)
        );
        assert_eq!(DocumentType::from_path(Path::new("file")), None);
        assert_eq!(
            DocumentType::from_path(Path::new("/path/to/file.txt")),
            None
        );
    }

    #[test]
    fn test_all_extensions() {
        let all = DocumentType::all_extensions();
        assert!(all.contains(&"md"));
        assert!(all.contains(&"markdown"));
        assert!(all.contains(&"pdf"));
        assert!(!all.contains(&"txt"));
    }

    #[test]
    fn test_is_text_based() {
        assert!(DocumentType::Markdown.is_text_based());
        assert!(!DocumentType::Pdf.is_text_based());
    }

    #[test]
    fn test_name() {
        assert_eq!(DocumentType::Markdown.name(), "markdown");
        assert_eq!(DocumentType::Pdf.name(), "pdf");
    }
}
