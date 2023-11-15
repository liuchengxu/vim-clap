mod language;
mod utf8_char_indices;

use std::collections::{BTreeMap, HashSet};
use tree_sitter_core::{Node, Point, TreeCursor};
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};

pub use self::language::Language;
pub use self::utf8_char_indices::{UncheckedUtf8CharIndices, Utf8CharIndices};
pub use tree_sitter_highlight::Error as HighlightError;

/// Parse .scm file for a list of node names.
pub fn parse_nodes_table(query: &str) -> Vec<&str> {
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
#[derive(Debug, Clone)]
pub struct HighlightItem {
    /// Column start, in bytes.
    pub start: Point,
    /// Column end, in bytes.
    pub end: Point,
    /// Highlight id.
    pub highlight: Highlight,
}

pub struct SyntaxHighlighter {
    highlighter: Highlighter,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self {
            highlighter: Highlighter::new(),
        }
    }

    /// Implements the syntax highlighting.
    pub fn highlight(
        &mut self,
        language: Language,
        source: &[u8],
    ) -> Result<BTreeMap<usize, Vec<HighlightItem>>, tree_sitter_highlight::Error> {
        let config = language::get_highlight_config(language);

        highlight_inner(&mut self.highlighter, &config, source)
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
    use super::*;

    /*
    fn tags() {
        let source_file = std::path::Path::new(
            "/home/xlc/.vim/plugged/vim-clap/crates/maple_core/src/stdio_server/plugin/system.rs",
        );
        let source_code = std::fs::read_to_string(source_file).unwrap();

        let mut context = TagsContext::new();
        let rust_config = TagsConfiguration::new(
            tree_sitter_rust::language(),
            tree_sitter_rust::TAGGING_QUERY,
            "",
        )
        .unwrap();

        let (tags, _) = context
            .generate_tags(&rust_config, source_code.as_bytes(), None)
            .unwrap();

        for tag in tags {
            let tag = tag.unwrap();
            let syntax_type = rust_config.syntax_type_name(tag.syntax_type_id);
            println!("text: {}", &source_code[tag.range]);
            println!(
                "name: {}, syntax type: {syntax_type}",
                &source_code[tag.name_range]
            );
        }
    }
    */

    #[test]
    fn it_works() {
        let mut parser = Parser::new();

        parser
            .set_language(tree_sitter_rust::language())
            .expect("Error loading Rust grammar");

        let source_file = std::path::Path::new(
            "/home/xlc/.vim/plugged/vim-clap/crates/maple_core/src/stdio_server/plugin/system.rs",
        );
        let source_code = std::fs::read_to_string(source_file).unwrap();
        let tree = parser.parse(&source_code, None).unwrap();

        let preorder: Vec<Node<'_>> = traverse(tree.walk(), Order::Pre).collect::<Vec<_>>();
        let postorder: Vec<Node<'_>> = traverse_tree(&tree, Order::Post).collect::<Vec<_>>();

        println!("");
        for node in preorder {
            println!("node: {:?}", node.kind());

            let text = &source_code[node.byte_range()];
            let node_kind = node.kind();
            match node_kind {
                "struct_item" => {
                    println!("struct: {text}");
                    let mut walk = node.walk();
                    for child in node.children(&mut walk) {
                        println!(
                            "level 0, child: {:?}, kind: {}",
                            &source_code[child.byte_range()],
                            child.kind()
                        );
                        let mut w = child.walk();
                        for c in child.children(&mut w) {
                            println!(
                                "level 1, child: {:?}, kind: {}",
                                &source_code[c.byte_range()],
                                c.kind()
                            );
                        }
                    }
                }
                "enum_item" => {
                    println!("enum: {text}");
                }
                "function_item" => {
                    println!("function: {text}");
                }
                "function_signature_item" => {
                    println!("function definition: {text}");
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_parse_highlight_groups() {
        println!("{:?}", parse_nodes_table(tree_sitter_rust::HIGHLIGHT_QUERY));
    }
}
