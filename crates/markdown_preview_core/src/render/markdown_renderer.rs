//! Markdown document renderer implementation.
//!
//! This module provides the [`MarkdownRenderer`] which implements the
//! [`TextRenderer`] trait for converting markdown to HTML.

use super::output::RenderOutput;
use super::traits::{RenderError, TextRenderer};
use super::{to_html, RenderOptions};
use crate::stats::{calculate_document_stats, DocumentStats};

/// Markdown document renderer.
///
/// Converts markdown content to HTML with support for GitHub Flavored Markdown,
/// syntax highlighting, and optional line mapping for scroll synchronization.
pub struct MarkdownRenderer {
    options: RenderOptions,
}

impl MarkdownRenderer {
    /// Create a new markdown renderer with the given options.
    pub fn new(options: RenderOptions) -> Self {
        Self { options }
    }

    /// Create a renderer configured for GUI mode (no line tracking).
    pub fn gui() -> Self {
        Self::new(RenderOptions::gui())
    }

    /// Create a renderer configured for vim-plugin mode (with line tracking).
    pub fn vim_plugin() -> Self {
        Self::new(RenderOptions::vim_plugin())
    }
}

impl TextRenderer for MarkdownRenderer {
    fn render_text(&self, content: &str) -> Result<RenderOutput, RenderError> {
        let result =
            to_html(content, &self.options).map_err(|e| RenderError::Other(e.to_string()))?;
        Ok(result.into_render_output())
    }

    fn calculate_text_stats(&self, content: &str) -> DocumentStats {
        calculate_document_stats(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_renderer() {
        let renderer = MarkdownRenderer::gui();
        let output = renderer.render_text("# Hello\n\nWorld").unwrap();

        if let RenderOutput::Html { content, line_map } = output {
            assert!(content.contains("<h1"));
            assert!(content.contains("Hello"));
            assert!(line_map.is_empty()); // GUI mode doesn't track lines
        } else {
            panic!("Expected HTML output");
        }
    }

    #[test]
    fn test_vim_plugin_renderer() {
        let renderer = MarkdownRenderer::vim_plugin();
        let output = renderer.render_text("# Hello\n\nWorld").unwrap();

        if let RenderOutput::Html { content, line_map } = output {
            assert!(content.contains("<h1"));
            // VimPlugin mode tracks lines
            assert!(!line_map.is_empty());
        } else {
            panic!("Expected HTML output");
        }
    }

    #[test]
    fn test_calculate_stats() {
        let renderer = MarkdownRenderer::gui();
        let stats = renderer.calculate_text_stats("Hello world!\n\nThis is a test.");

        assert!(stats.words > 0);
        assert!(stats.lines > 0);
    }
}
