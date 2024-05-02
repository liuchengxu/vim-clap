mod language;
mod utf8_char_indices;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use tree_sitter_core::{Node, Point, TreeCursor};
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};

pub use self::language::Language;
pub use self::utf8_char_indices::{UncheckedUtf8CharIndices, Utf8CharIndices};
pub use tree_sitter_highlight::Error as HighlightError;

/// Parse .scm file for a list of node names.
pub fn parse_scopes(query: &str) -> Vec<&str> {
    let mut groups = query
        .split('\n')
        .filter_map(|line| {
            let line = line.trim();

            // Ignore the comment line.
            if line.starts_with(';') {
                None
            } else {
                Some(
                    // Each group confirms to the format @foo.
                    line.split_whitespace()
                        .filter_map(|i| {
                            i.strip_prefix('@')
                                .map(|i| i.trim_end_matches(|c: char| !c.is_ascii_alphanumeric()))
                        })
                        .collect::<Vec<_>>(),
                )
            }
        })
        .flatten()
        // Deduplication
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    groups.sort();

    groups
}

/// Represents a highlight element within a line.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HighlightItem {
    /// Column start, in bytes.
    pub start: Point,
    /// Column end, in bytes.
    pub end: Point,
    /// Highlight id.
    pub highlight: Highlight,
}

thread_local! {
    static HIGHLIGHTER: RefCell<Highlighter> = RefCell::new(Highlighter::new());
}

impl Language {
    /// Returns the syntax highlights of multiple lines.
    pub fn highlight(
        self,
        source: &[u8],
    ) -> Result<BTreeMap<usize, Vec<HighlightItem>>, tree_sitter_highlight::Error> {
        let config = language::get_highlight_config(self);
        HIGHLIGHTER.with_borrow_mut(|highlighter| highlight_inner(highlighter, &config, source))
    }

    /// Returns the syntax highlights of a single line.
    pub fn highlight_line(
        self,
        source: &[u8],
    ) -> Result<Vec<HighlightItem>, tree_sitter_highlight::Error> {
        let config = language::get_highlight_config(self);
        HIGHLIGHTER.with_borrow_mut(|highlighter| {
            highlight_inner(highlighter, &config, source)
                .map(|x| x.get(&0).cloned().unwrap_or_default())
        })
    }
}

fn highlight_inner(
    highlighter: &mut Highlighter,
    highlight_config: &HighlightConfiguration,
    source: &[u8],
) -> Result<BTreeMap<usize, Vec<HighlightItem>>, tree_sitter_highlight::Error> {
    let mut row = 0;
    let mut column = 0;
    let mut byte_offset = 0;
    let mut was_newline = false;
    let mut res = BTreeMap::new();
    let mut highlight_stack = Vec::new();
    // TODO: avoid allocation?
    let source = String::from_utf8_lossy(source);
    let mut char_indices = source.char_indices();
    for highlight_result in
        highlighter.highlight(highlight_config, source.as_bytes(), None, |_string| None)?
    {
        match highlight_result? {
            HighlightEvent::HighlightStart(h) => highlight_stack.push(h),
            HighlightEvent::HighlightEnd => {
                highlight_stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let mut start_position = Point::new(row, column);
                while byte_offset < end {
                    if byte_offset <= start {
                        start_position = Point::new(row, column);
                    }
                    if let Some((i, c)) = char_indices.next() {
                        if was_newline {
                            row += 1;
                            column = 0;
                        } else {
                            column += i - byte_offset;
                        }
                        was_newline = c == '\n';
                        byte_offset = i;
                    } else {
                        break;
                    }
                }
                if let Some(highlight) = highlight_stack.last() {
                    let line_number = start_position.row;
                    let info = HighlightItem {
                        start: start_position,
                        end: Point::new(row, column),
                        highlight: *highlight,
                    };
                    let items: &mut Vec<_> = res.entry(line_number).or_default();
                    items.push(info);
                }
            }
        }
    }
    Ok(res)
}

fn node_is_visible(node: &Node) -> bool {
    node.is_missing() || (node.is_named() && node.language().node_kind_is_visible(node.kind_id()))
}

pub fn pretty_print_tree<W: std::fmt::Write>(fmt: &mut W, node: Node) -> std::fmt::Result {
    if node.child_count() == 0 {
        if node_is_visible(&node) {
            write!(fmt, "({})", node.kind())
        } else {
            write!(fmt, "\"{}\"", node.kind())
        }
    } else {
        pretty_print_tree_impl(fmt, &mut node.walk(), 0)
    }
}

fn pretty_print_tree_impl<W: std::fmt::Write>(
    fmt: &mut W,
    cursor: &mut TreeCursor,
    depth: usize,
) -> std::fmt::Result {
    let node = cursor.node();
    let visible = node_is_visible(&node);

    if visible {
        let indentation_columns = depth * 2;
        write!(fmt, "{:indentation_columns$}", "")?;

        if let Some(field_name) = cursor.field_name() {
            write!(fmt, "{}: ", field_name)?;
        }

        write!(fmt, "({}", node.kind())?;
    }

    // Handle children.
    if cursor.goto_first_child() {
        loop {
            if node_is_visible(&cursor.node()) {
                fmt.write_char('\n')?;
            }

            pretty_print_tree_impl(fmt, cursor, depth + 1)?;

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        let moved = cursor.goto_parent();
        // The parent of the first child must exist, and must be `node`.
        debug_assert!(moved);
        debug_assert!(cursor.node() == node);
    }

    if visible {
        fmt.write_char(')')?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn test_highlight_line() {
        // let line = b"let config = language::get_highlight_config(language);";

        // let line = b"pub fn parse_scopes(query: &str) -> Vec<&str> {";
        let line = b"// Cpp comment line";

        let highlight_items = crate::language::Language::Cpp.highlight_line(line).unwrap();
        println!("{highlight_items:?}");

        let syntax_tokens = highlight_items
            .into_iter()
            .map(|i| {
                (
                    crate::Language::Rust.highlight_name(i.highlight),
                    String::from_utf8_lossy(&line[i.start.column..i.end.column]),
                )
            })
            .collect::<Vec<_>>();

        println!("syntax_tokens: {syntax_tokens:?}");

        // assert_eq!(
        // syntax_tokens,
        // vec![
        // "keyword",
        // "punctuation.delimiter",
        // "function",
        // "punctuation.bracket",
        // "punctuation.bracket",
        // "punctuation.delimiter"
        // ]
        // )
    }

    #[test]
    fn test_parse_highlight_groups() {
        // use tree_sitter_core::{Query, QueryCursor, TextProvider};
        // use tree_sitter_tags::{TagsConfiguration, TagsContext};

        // let mut context = TagsContext::new();

        // let language = tree_sitter_rust::language();
        // let mut parser = tree_sitter_core::Parser::new();
        // parser
        // .set_language(language)
        // .expect("Error loading Rust grammar");

        // let tags_query = include_str!("../queries/rust/tags.scm");
        // let query = Query::new(language, tags_query).unwrap();

        // let source_code = include_bytes!("../../maple_core/src/stdio_server/service.rs");
        // let tree = parser.parse(source_code, None).unwrap();

        // for (i, name) in query.capture_names().iter().enumerate() {
        // println!("i: {i}, name: {name}");
        // }

        // let mut cursor = QueryCursor::new();
        // let matches = cursor.matches(&query, tree.root_node(), source_code.as_slice());

        // for mat in matches {
        // for cap in mat.captures {
        // let index = Some(cap.index);
        // let range = cap.node.byte_range();
        // if capture_names[cap.index as usize].starts_with("name.definition") {

        // println!(
        // "===== index: {index:?} {}, range: {:?}, text: {}",
        // &capture_names[cap.index as usize],
        // &range,
        // String::from_utf8_lossy(&source_code[range.clone()]),
        // );
        // }
        // }
        // }
    }
}
