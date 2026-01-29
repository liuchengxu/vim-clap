//! Table of contents generation for markdown documents.
//!
//! This module provides functionality to:
//! - Parse markdown headings
//! - Generate table of contents with configurable formatting
//! - Find and update existing TOC markers
//! - Generate URL-safe slugs from heading text

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::path::Path;
use std::str::FromStr;

/// Read lines from a file, returning an iterator.
fn read_lines<P: AsRef<Path>>(path: P) -> std::io::Result<Lines<BufReader<File>>> {
    let file = File::open(path)?;
    Ok(BufReader::new(file).lines())
}

/// Converts heading text to a URL-safe slug following GitHub's convention.
///
/// GitHub's algorithm:
/// 1. Convert to lowercase
/// 2. Replace spaces with hyphens
/// 3. Remove all characters except alphanumeric, hyphens, and underscores
/// 4. Collapse multiple consecutive hyphens into one
///
/// # Example
///
/// ```
/// use markdown_preview_core::toc::slugify;
///
/// assert_eq!(slugify("Hello World"), "hello-world");
/// assert_eq!(slugify("API Reference (v2)"), "api-reference-v2");
/// ```
pub fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else if c == ' ' || c == '-' {
                '-'
            } else {
                // Remove other characters (punctuation, etc.)
                '\0'
            }
        })
        .filter(|&c| c != '\0')
        .collect::<String>()
        // Collapse multiple hyphens into one
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Configuration for table of contents generation.
#[derive(Debug, Clone)]
pub struct TocConfig {
    /// Bullet character for list items (default: "*")
    pub bullet: String,
    /// Number of spaces per indent level (default: 4)
    pub indent: usize,
    /// Maximum heading depth to include (default: None, include all)
    pub max_depth: Option<usize>,
    /// Minimum heading depth to include (default: 1)
    pub min_depth: usize,
    /// Optional header text for the TOC (default: "## Table of Contents")
    pub header: Option<String>,
    /// If true, generate plain text without links (default: false)
    pub no_link: bool,
}

impl Default for TocConfig {
    fn default() -> Self {
        Self {
            bullet: String::from("*"),
            indent: 4,
            max_depth: None,
            min_depth: 1,
            no_link: false,
            header: Some(String::from("## Table of Contents")),
        }
    }
}

/// Represents a parsed markdown heading.
#[derive(Debug, Clone)]
pub struct Heading {
    /// Heading depth (0-indexed: h1=0, h2=1, etc.)
    pub depth: usize,
    /// Heading text content
    pub title: String,
}

impl FromStr for Heading {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim_end();
        if trimmed.starts_with('#') {
            let mut depth = 0usize;
            let title = trimmed
                .chars()
                .skip_while(|c| {
                    if *c == '#' {
                        depth += 1;
                        true
                    } else {
                        false
                    }
                })
                .collect::<String>()
                .trim_start()
                .to_owned();
            Ok(Heading {
                depth: depth - 1,
                title,
            })
        } else {
            Err(())
        }
    }
}

static MARKDOWN_LINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[(.*)\](.*)").unwrap());

impl Heading {
    /// Format the heading as a TOC entry according to the given config.
    pub fn format(&self, config: &TocConfig) -> Option<String> {
        if self.depth >= config.min_depth
            && config.max_depth.map(|d| self.depth <= d).unwrap_or(true)
        {
            let Self { depth, title } = self;
            let title_link = strip_backticks(title);
            let indent_before_bullet = " "
                .repeat(config.indent)
                .repeat(depth.saturating_sub(config.min_depth));
            let bullet = &config.bullet;
            let indent_after_bullet = " ".repeat(config.indent.saturating_sub(1));

            if config.no_link {
                Some(format!(
                    "{indent_before_bullet}{bullet}{indent_after_bullet}{title}"
                ))
            } else if let Some(cap) = MARKDOWN_LINK.captures(title) {
                let title = cap.get(1).map(|x| x.as_str())?;
                Some(format!(
                    "{indent_before_bullet}{bullet}{indent_after_bullet}[{title}](#{})",
                    slugify(&title_link)
                ))
            } else {
                Some(format!(
                    "{indent_before_bullet}{bullet}{indent_after_bullet}[{title}](#{})",
                    slugify(&title_link)
                ))
            }
        } else {
            None
        }
    }
}

/// Indicates the type of code block fence.
enum CodeBlockStart {
    Backticks,
    Tildes,
}

fn parse_toc(
    input_file: &Path,
    toc_config: &TocConfig,
    line_start: usize,
) -> std::io::Result<Vec<String>> {
    let mut code_fence = None;
    Ok(read_lines(input_file)?
        .skip(line_start)
        .filter_map(Result::ok)
        .filter(|line| match &code_fence {
            None => {
                if line.starts_with("```") {
                    code_fence.replace(CodeBlockStart::Backticks);
                    false
                } else if line.starts_with("~~~") {
                    code_fence.replace(CodeBlockStart::Tildes);
                    false
                } else {
                    true
                }
            }
            Some(code_block_start) => {
                match code_block_start {
                    CodeBlockStart::Backticks if line.starts_with("```") => {
                        code_fence.take();
                    }
                    CodeBlockStart::Tildes if line.starts_with("~~~") => {
                        code_fence.take();
                    }
                    _ => {}
                }
                false
            }
        })
        .filter_map(|line| {
            line.parse::<Heading>()
                .ok()
                .and_then(|heading| heading.format(toc_config))
        })
        .collect())
}

/// Generate a table of contents for a markdown file.
///
/// # Arguments
///
/// * `input_file` - Path to the markdown file
/// * `line_start` - Line number to start parsing from (0-indexed)
/// * `shiftwidth` - Number of spaces per indent level
///
/// # Returns
///
/// A vector of strings representing the TOC, wrapped in marker comments.
pub fn generate_toc(
    input_file: impl AsRef<Path>,
    line_start: usize,
    shiftwidth: usize,
) -> std::io::Result<VecDeque<String>> {
    let toc_config = TocConfig {
        indent: shiftwidth,
        ..Default::default()
    };
    let toc = parse_toc(input_file.as_ref(), &toc_config, line_start)?;

    let mut full_toc = Vec::with_capacity(toc.len() + 4);
    full_toc.push("<!-- clap-markdown-toc -->".to_string());
    full_toc.push(Default::default());
    full_toc.extend(toc);
    full_toc.push(Default::default());
    full_toc.push("<!-- /clap-markdown-toc -->".to_string());

    Ok(full_toc.into())
}

/// Find the line range of an existing TOC in a markdown file.
///
/// Looks for TOC marker comments:
/// - Start: `<!-- clap-markdown-toc -->`
/// - End: `<!-- /clap-markdown-toc -->`
///
/// # Returns
///
/// `Some((start, end))` with 0-indexed line numbers, or `None` if not found.
pub fn find_toc_range(input_file: impl AsRef<Path>) -> std::io::Result<Option<(usize, usize)>> {
    let mut start = 0;

    for (idx, line) in read_lines(input_file)?.map_while(Result::ok).enumerate() {
        let line = line.trim();
        if line == "<!-- clap-markdown-toc -->" {
            start = idx;
        } else if line == "<!-- /clap-markdown-toc -->" {
            return Ok(Some((start, idx)));
        } else {
            continue;
        }
    }

    Ok(None)
}

/// Strip backticks from text while preserving the inner content.
fn strip_backticks(input: &str) -> String {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"`([^`]*)`").unwrap());
    RE.replace_all(input, "$1").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("API Reference"), "api-reference");
        assert_eq!(slugify("Test-123"), "test-123");
        assert_eq!(slugify("foo_bar"), "foo_bar");
        assert_eq!(slugify("Multiple   Spaces"), "multiple-spaces");
    }

    #[test]
    fn test_heading_parsing() {
        let heading: Heading = "### run-`subcoin import-blocks`".parse().unwrap();
        assert_eq!(heading.title, "run-`subcoin import-blocks`");
        assert_eq!(heading.depth, 2);
    }

    #[test]
    fn test_heading_format() {
        let heading: Heading = "### run-`subcoin import-blocks`".parse().unwrap();
        let config = TocConfig {
            max_depth: Some(4),
            ..Default::default()
        };
        let formatted = heading.format(&config).unwrap();
        assert_eq!(
            formatted,
            "    *   [run-`subcoin import-blocks`](#run-subcoin-import-blocks)"
        );
    }

    #[test]
    fn test_strip_backticks() {
        assert_eq!(strip_backticks("hello `world`"), "hello world");
        assert_eq!(strip_backticks("`code` and `more`"), "code and more");
        assert_eq!(strip_backticks("no backticks"), "no backticks");
    }
}
