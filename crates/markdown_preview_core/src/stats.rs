//! Document statistics calculation for markdown content.
//!
//! Provides word count, character count, line count, and reading time estimation.

use serde::{Deserialize, Serialize};

/// Document statistics for display in the preview.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentStats {
    /// Total word count (0 for binary documents like PDFs)
    pub words: usize,
    /// Total character count (excluding whitespace)
    pub characters: usize,
    /// Total character count (including whitespace)
    pub characters_with_spaces: usize,
    /// Total line count (for text documents only)
    pub lines: usize,
    /// Estimated reading time in minutes (based on 200 words per minute)
    pub reading_minutes: usize,
    /// Page count (only for paginated documents like PDF)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<usize>,
}

/// Calculate document statistics from markdown content.
///
/// # Arguments
///
/// * `content` - The raw markdown text
///
/// # Returns
///
/// A `DocumentStats` struct containing various statistics about the document.
///
/// # Example
///
/// ```
/// use markdown_preview_core::stats::calculate_document_stats;
///
/// let stats = calculate_document_stats("Hello world!\n\nThis is a test.");
/// assert!(stats.words > 0);
/// assert!(stats.lines > 0);
/// ```
pub fn calculate_document_stats(content: &str) -> DocumentStats {
    let lines = content.lines().count();

    // Count words by splitting on whitespace
    let words: usize = content
        .lines()
        .map(|line| {
            line.split_whitespace()
                .filter(|word| {
                    // Filter out pure markdown syntax tokens
                    let trimmed = word.trim_matches(|c: char| {
                        c == '#'
                            || c == '*'
                            || c == '_'
                            || c == '`'
                            || c == '['
                            || c == ']'
                            || c == '('
                            || c == ')'
                            || c == '-'
                            || c == '>'
                            || c == '|'
                    });
                    !trimmed.is_empty()
                })
                .count()
        })
        .sum();

    // Count characters
    let characters_with_spaces = content.chars().count();
    let characters = content.chars().filter(|c| !c.is_whitespace()).count();

    // Reading time: average adult reads ~200-250 words per minute
    // Use 200 wpm for a conservative estimate
    let reading_minutes = words.div_ceil(200);

    DocumentStats {
        words,
        characters,
        characters_with_spaces,
        lines,
        reading_minutes,
        pages: None,
    }
}

/// Calculate statistics for PDF documents.
///
/// Since PDFs are binary documents, we can only meaningfully track the page count.
/// Reading time is estimated at approximately 2 minutes per page.
///
/// # Arguments
///
/// * `page_count` - The number of pages in the PDF document
///
/// # Example
///
/// ```
/// use markdown_preview_core::stats::calculate_pdf_stats;
///
/// let stats = calculate_pdf_stats(10);
/// assert_eq!(stats.pages, Some(10));
/// assert_eq!(stats.reading_minutes, 20);  // 2 min/page
/// ```
pub fn calculate_pdf_stats(page_count: usize) -> DocumentStats {
    DocumentStats {
        words: 0,
        characters: 0,
        characters_with_spaces: 0,
        lines: 0,
        reading_minutes: page_count.saturating_mul(2), // ~2 min/page estimate
        pages: Some(page_count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_stats() {
        let content = "Hello world!\n\nThis is a test.";
        let stats = calculate_document_stats(content);

        assert_eq!(stats.lines, 3);
        assert!(stats.words >= 5);
        assert!(stats.characters > 0);
        assert!(stats.characters_with_spaces > stats.characters);
    }

    #[test]
    fn test_empty_content() {
        let stats = calculate_document_stats("");
        assert_eq!(stats.words, 0);
        assert_eq!(stats.characters, 0);
        assert_eq!(stats.lines, 0);
    }

    #[test]
    fn test_markdown_syntax_filtering() {
        let content = "# Heading\n\n**Bold** and *italic*\n\n- List item";
        let stats = calculate_document_stats(content);

        // Should count real words, not just syntax
        assert!(stats.words >= 4);
    }

    #[test]
    fn test_reading_time() {
        // 200 words should be ~1 minute
        let words: Vec<&str> = std::iter::repeat("word").take(200).collect();
        let content = words.join(" ");
        let stats = calculate_document_stats(&content);

        assert_eq!(stats.reading_minutes, 1);
    }

    #[test]
    fn test_reading_time_longer() {
        // 450 words should be ~3 minutes (450/200 = 2.25, rounded up)
        let words: Vec<&str> = std::iter::repeat("word").take(450).collect();
        let content = words.join(" ");
        let stats = calculate_document_stats(&content);

        assert_eq!(stats.reading_minutes, 3);
    }

    #[test]
    fn test_pdf_stats() {
        let stats = calculate_pdf_stats(10);
        assert_eq!(stats.pages, Some(10));
        assert_eq!(stats.words, 0);
        assert_eq!(stats.characters, 0);
        assert_eq!(stats.lines, 0);
        assert_eq!(stats.reading_minutes, 20); // 2 min/page
    }

    #[test]
    fn test_pdf_stats_zero_pages() {
        let stats = calculate_pdf_stats(0);
        assert_eq!(stats.pages, Some(0));
        assert_eq!(stats.reading_minutes, 0);
    }

    #[test]
    fn test_markdown_stats_no_pages() {
        let stats = calculate_document_stats("Hello world!");
        assert_eq!(stats.pages, None);
    }
}
