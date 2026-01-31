//! Markdown to HTML rendering with GitHub-style features.
//!
//! This module provides markdown-to-HTML conversion with support for:
//! - GitHub Flavored Markdown (tables, strikethrough, task lists)
//! - GitHub-style alerts ([!NOTE], [!TIP], [!IMPORTANT], [!WARNING], [!CAUTION])
//! - Heading IDs for anchor links
//! - Source line mapping for scroll synchronization (vim-clap mode only)

mod github_alerts;
mod heading;

use crate::toc;
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};

pub use github_alerts::detect_github_alert;

/// Preview mode determines which features are enabled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PreviewMode {
    /// Standalone GUI app - no line tracking needed
    #[default]
    Gui,
    /// vim-clap integration via WebSocket - needs line tracking for scroll sync
    VimPlugin,
}

/// Options for rendering markdown to HTML.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Preview mode (Gui or VimPlugin)
    pub mode: PreviewMode,
    /// Enable GitHub Flavored Markdown tables
    pub enable_tables: bool,
    /// Enable strikethrough syntax (~~text~~)
    pub enable_strikethrough: bool,
    /// Enable task list items ([x] and [ ])
    pub enable_tasklists: bool,
    /// Enable heading attributes ({#id .class})
    pub enable_heading_attributes: bool,
}

impl RenderOptions {
    /// Create options for standalone GUI app (no line tracking).
    pub fn gui() -> Self {
        Self {
            mode: PreviewMode::Gui,
            enable_tables: true,
            enable_strikethrough: true,
            enable_tasklists: true,
            enable_heading_attributes: true,
        }
    }

    /// Create options for vim-clap plugin mode (with line tracking for scroll sync).
    pub fn vim_plugin() -> Self {
        Self {
            mode: PreviewMode::VimPlugin,
            enable_tables: true,
            enable_strikethrough: true,
            enable_tasklists: true,
            enable_heading_attributes: true,
        }
    }

    fn to_pulldown_options(&self) -> Options {
        let mut options = Options::empty();
        if self.enable_tables {
            options.insert(Options::ENABLE_TABLES);
        }
        if self.enable_strikethrough {
            options.insert(Options::ENABLE_STRIKETHROUGH);
        }
        if self.enable_tasklists {
            options.insert(Options::ENABLE_TASKLISTS);
        }
        if self.enable_heading_attributes {
            options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        }
        options
    }
}

/// Result of rendering markdown to HTML.
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// The rendered HTML content
    pub html: String,
    /// Mapping from rendered element index to source line number (1-indexed)
    pub line_map: Vec<usize>,
}

/// Convert byte offset to line number (1-indexed).
fn byte_offset_to_line(content: &str, byte_offset: usize) -> usize {
    let mut line = 1;
    for (i, byte) in content.bytes().enumerate() {
        if i >= byte_offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
        }
    }
    line
}

/// Render markdown content to HTML.
///
/// Returns the HTML output and a line map for scroll synchronization.
/// The line map maps rendered element indices to source line numbers.
///
/// # Arguments
///
/// * `markdown_content` - The raw markdown text
/// * `options` - Rendering options (use `RenderOptions::gui()` for GitHub Flavored Markdown)
///
/// # Example
///
/// ```
/// use markdown_preview_core::render::{to_html, RenderOptions};
///
/// let result = to_html("# Hello\n\nWorld", &RenderOptions::gui()).unwrap();
/// assert!(result.html.contains("<h1"));
/// ```
pub fn to_html(
    markdown_content: &str,
    options: &RenderOptions,
) -> Result<RenderResult, Box<dyn std::error::Error + Send + Sync>> {
    let pulldown_options = options.to_pulldown_options();
    let parser = Parser::new_ext(markdown_content, pulldown_options);

    let mut html_output = String::new();
    let mut heading_text = String::new();

    // Use into_offset_iter to get byte offsets for each event
    let events_with_offsets: Vec<(Event, std::ops::Range<usize>)> =
        parser.into_offset_iter().collect();
    let events: Vec<Event> = events_with_offsets.iter().map(|(e, _)| e.clone()).collect();
    let mut processed_events = Vec::new();
    let mut line_map = Vec::new();

    // Track nesting depth to avoid counting nested lists
    let mut list_depth: i32 = 0;
    let mut blockquote_depth: i32 = 0;

    let mut i = 0;
    while i < events.len() {
        // Update depth counters
        match &events[i] {
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth -= 1,
            Event::Start(Tag::BlockQuote) => blockquote_depth += 1,
            Event::End(TagEnd::BlockQuote) => blockquote_depth -= 1,
            _ => {}
        }

        // Only track line numbers in VimPlugin mode (for scroll sync)
        if options.mode == PreviewMode::VimPlugin {
            // Only track top-level elements (not nested inside lists or blockquotes)
            // Exception: We DO track the first level list/blockquote itself
            let should_track_line = match &events[i] {
                Event::Start(Tag::Paragraph) => list_depth == 0 && blockquote_depth == 0,
                Event::Start(Tag::Heading { .. }) => true, // Headings are always top-level
                Event::Start(Tag::BlockQuote) => blockquote_depth == 1, // First level only
                Event::Start(Tag::CodeBlock(_)) => list_depth == 0 && blockquote_depth == 0,
                Event::Start(Tag::List(_)) => list_depth == 1, // First level only
                Event::Start(Tag::Table(_)) => list_depth == 0 && blockquote_depth == 0,
                _ => false,
            };

            if should_track_line {
                let byte_offset = events_with_offsets[i].1.start;
                let line_number = byte_offset_to_line(markdown_content, byte_offset);
                tracing::debug!(
                    event = ?events[i],
                    byte_offset,
                    line_number,
                    list_depth,
                    blockquote_depth,
                    "Tracking line number for element"
                );
                line_map.push(line_number);
            }
        }

        match &events[i] {
            Event::Start(Tag::Heading {
                level,
                id: _,
                classes,
                attrs,
            }) => {
                heading_text.clear();

                // Collect heading text
                let mut j = i + 1;
                while j < events.len() {
                    match &events[j] {
                        Event::Text(text) | Event::Code(text) => {
                            heading_text.push_str(text);
                            j += 1;
                        }
                        Event::End(TagEnd::Heading(_)) => {
                            break;
                        }
                        _ => {
                            j += 1;
                        }
                    }
                }

                // Strip backticks and generate slug for heading (same as TOC does)
                let heading_text_without_backticks = heading_text.replace('`', "");
                let slug = toc::slugify(&heading_text_without_backticks);

                // Create heading with ID
                processed_events.push(Event::Start(Tag::Heading {
                    level: *level,
                    id: Some(slug.into()),
                    classes: classes.clone(),
                    attrs: attrs.clone(),
                }));

                i += 1;
            }
            Event::End(TagEnd::Heading(_)) => {
                processed_events.push(events[i].clone());
                i += 1;
            }
            Event::Start(Tag::BlockQuote) => {
                // Check if this is a GitHub alert by looking at the first text content
                let mut j = i + 1;
                let mut first_text = String::new();

                while j < events.len() {
                    match &events[j] {
                        Event::Text(text) => {
                            first_text.push_str(text);
                            break;
                        }
                        Event::Start(_) => {
                            j += 1;
                        }
                        Event::End(TagEnd::BlockQuote) => {
                            break;
                        }
                        _ => {
                            j += 1;
                        }
                    }
                }

                if let Some((alert_type, title, svg_icon)) = detect_github_alert(&first_text) {
                    // This is a GitHub alert - transform it to custom HTML
                    // Find the end of the blockquote
                    let mut end_idx = i + 1;
                    let mut depth = 1;
                    while end_idx < events.len() && depth > 0 {
                        match &events[end_idx] {
                            Event::Start(Tag::BlockQuote) => depth += 1,
                            Event::End(TagEnd::BlockQuote) => depth -= 1,
                            _ => {}
                        }
                        end_idx += 1;
                    }

                    // Emit custom HTML for GitHub alert
                    processed_events.push(Event::Html(CowStr::from(format!(
                        r#"<div class="markdown-alert markdown-alert-{alert_type}"><p class="markdown-alert-title">{svg_icon}{title}</p>"#
                    ))));

                    // Process inner content, skipping the alert marker text
                    let mut skip_first_text = true;
                    for event in events.iter().skip(i + 1).take(end_idx - i - 1) {
                        match event {
                            Event::Text(text) if skip_first_text => {
                                // Remove the [!TYPE] marker from the text
                                let cleaned = text.trim_start();
                                if let Some(content_start) = cleaned.find(']') {
                                    let remaining = &cleaned[content_start + 1..].trim_start();
                                    if !remaining.is_empty() {
                                        processed_events
                                            .push(Event::Text(CowStr::from(remaining.to_string())));
                                    }
                                }
                                skip_first_text = false;
                            }
                            Event::End(TagEnd::BlockQuote) => {
                                // Don't emit the blockquote end
                            }
                            Event::Start(Tag::BlockQuote) => {
                                // Don't emit nested blockquote start if it's the outer one
                            }
                            _ => {
                                processed_events.push(event.clone());
                            }
                        }
                    }

                    // Close the alert div
                    processed_events.push(Event::Html(CowStr::from("</div>")));

                    i = end_idx;
                } else {
                    // Regular blockquote
                    processed_events.push(events[i].clone());
                    i += 1;
                }
            }
            _ => {
                processed_events.push(events[i].clone());
                i += 1;
            }
        }
    }

    pulldown_cmark::html::push_html(&mut html_output, processed_events.into_iter());

    // Only log line map in VimPlugin mode where it's actually used
    if options.mode == PreviewMode::VimPlugin {
        tracing::debug!(
            line_map_length = line_map.len(),
            line_map = ?&line_map[..line_map.len().min(20)],
            "Generated line map"
        );
    }

    Ok(RenderResult {
        html: html_output,
        line_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_rendering() {
        let result = to_html("# Hello\n\nWorld", &RenderOptions::gui()).unwrap();
        assert!(result.html.contains("<h1"));
        assert!(result.html.contains("Hello"));
        assert!(result.html.contains("<p>World</p>"));
    }

    #[test]
    fn test_heading_ids() {
        let result = to_html("# Test Heading", &RenderOptions::gui()).unwrap();
        assert!(result.html.contains(r#"id="test-heading""#));
    }

    #[test]
    fn test_github_alert() {
        let result = to_html("> [!NOTE]\n> This is a note", &RenderOptions::gui()).unwrap();
        assert!(result.html.contains("markdown-alert-note"));
    }
}
