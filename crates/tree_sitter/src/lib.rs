use std::collections::{BTreeMap, HashSet};
use tree_sitter::{Language, Node, Parser, Point, TreeCursor};
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};
use tree_sitter_tags::{TagsConfiguration, TagsContext};
use tree_sitter_traversal::{traverse, traverse_tree, Order};

/// Parse .scm file for a list of node kinds.
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

#[derive(Debug, Clone)]
pub struct HighlightItem {
    pub start: Point,
    pub end: Point,
    pub highlight: Highlight,
}

pub fn get_highlight_items(
    source: &[u8],
    highlight_names: &[&str],
) -> Result<BTreeMap<usize, Vec<HighlightItem>>, tree_sitter_highlight::Error> {
    let rust_language = tree_sitter_rust::language();

    let mut rust_config =
        HighlightConfiguration::new(rust_language, tree_sitter_rust::HIGHLIGHT_QUERY, "", "")
            .unwrap();

    rust_config.configure(highlight_names);

    let mut highlighter = Highlighter::new();

    get_highlight_items_inner(&mut highlighter, &rust_config, source)
}

fn get_highlight_items_inner(
    highlighter: &mut Highlighter,
    highlight_config: &HighlightConfiguration,
    source: &[u8],
) -> Result<BTreeMap<usize, Vec<HighlightItem>>, tree_sitter_highlight::Error> {
    let mut row = 0;
    let mut column = 0;
    let mut byte_offset = 0;
    let mut was_newline = false;
    let mut result = BTreeMap::new();
    let mut highlight_stack = Vec::new();
    let source = String::from_utf8_lossy(source);
    let mut char_indices = source.char_indices();
    for event in highlighter.highlight(highlight_config, source.as_bytes(), None, |_string| None)? {
        match event? {
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
                    let line_pos: &mut Vec<_> = result.entry(line_number).or_default();
                    line_pos.push(info);
                }
            }
        }
    }
    Ok(result)
}

fn highlight() {
    let rust_language = tree_sitter_rust::language();

    let mut rust_config =
        HighlightConfiguration::new(rust_language, tree_sitter_rust::HIGHLIGHT_QUERY, "", "")
            .unwrap();

    let highlight_names = [
        "attribute",
        "constant",
        "function.builtin",
        "function",
        "keyword",
        "operator",
        "property",
        "punctuation",
        "punctuation.bracket",
        "punctuation.delimiter",
        "string",
        "string.special",
        "tag",
        "type",
        "type.builtin",
        "variable",
        "variable.builtin",
        "variable.parameter",
    ];

    rust_config.configure(&highlight_names);

    let source_file = std::path::Path::new(
        "/home/xlc/.vim/plugged/vim-clap/crates/maple_core/src/stdio_server/plugin/system.rs",
    );
    let source_code = std::fs::read_to_string(source_file).unwrap();

    let mut highlighter = Highlighter::new();

    let highlights = highlighter
        .highlight(&rust_config, source_code.as_bytes(), None, |_| None)
        .unwrap();

    for highlight in highlights {
        println!("{highlight:?}");

        match highlight.unwrap() {
            HighlightEvent::Source { start, end } => {
                println!("source: {}-{}, {}", start, end, &source_code[start..end]);
            }
            HighlightEvent::HighlightStart(s) => {
                println!("highlight style started: {:?}", s);
            }
            HighlightEvent::HighlightEnd => {
                println!("highlight style ended");
            }
        }
    }

    let positions =
        get_highlight_items_inner(&mut highlighter, &rust_config, source_code.as_bytes()).unwrap();
    for pos in &positions {
        println!("{pos:?}");
    }

    println!("total highlights: {}", positions.len());
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

pub fn pretty_print_tree_full<W: std::fmt::Write>(
    fmt: &mut W,
    node: Node,
    source_code: &[u8],
) -> std::fmt::Result {
    if node.child_count() == 0 {
        if node.kind() == "field_declaration" {
            write!(fmt, "---------- node {:?}", node.parent());
        }

        if node_is_visible(&node) {
            write!(
                fmt,
                "({})[{}]",
                node.kind(),
                String::from_utf8_lossy(&source_code[node.byte_range()])
            )
        } else {
            write!(
                fmt,
                "\"{}\"[{}]",
                node.kind(),
                String::from_utf8_lossy(&source_code[node.byte_range()])
            )
        }
    } else {
        pretty_print_tree_impl_full(fmt, &mut node.walk(), 0, source_code)
    }
}

fn pretty_print_tree_impl_full<W: std::fmt::Write>(
    fmt: &mut W,
    cursor: &mut TreeCursor,
    depth: usize,
    source_code: &[u8],
) -> std::fmt::Result {
    let node = cursor.node();
    let visible = node_is_visible(&node);

    if node.kind() == "field_declaration" {
        let parent = node.parent().unwrap().parent().unwrap();
        let text = &source_code[parent.byte_range()];
        write!(fmt, "========== node text: {text:?}\n");
    }

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

            pretty_print_tree_impl_full(fmt, cursor, depth + 1, source_code)?;

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
        // write!(fmt, "[{}]", String::from_utf8_lossy(&source_code[node.byte_range()]));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let mut output = String::new();
        pretty_print_tree_full(&mut output, tree.root_node(), source_code.as_bytes());
        println!("{output}");

        // let rust_language = tree_sitter_rust::language();

        // for i in 0..rust_language.node_kind_count() {
        // println!("node kind for id: {:?} {:?}", rust_language.node_kind_for_id(i as u16), rust_language.field_name_for_id(i as u16));
        // }

        // for i in 0..rust_language.field_count() {
        // println!("field for id: {:?}", rust_language.field_name_for_id(i as u16));
        // }

        // println!("preorder: {preorder:?}");
        // println!("postorder: {postorder:?}");
    }

    #[test]
    fn test_parse_highlight_groups() {
        println!("{:?}", parse_nodes_table(tree_sitter_rust::HIGHLIGHT_QUERY));
    }
}
