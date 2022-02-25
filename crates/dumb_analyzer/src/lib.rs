//! Poor man's language analyzer.

use std::collections::HashMap;

use once_cell::sync::OnceCell;

/// Returns a list of comment prefix for a source file.
///
/// # Argument
///
/// - `ext`: the extension of a file, e.g., `rs`.
pub fn get_comment_syntax(ext: &str) -> &[&str] {
    static LANGUAGE_COMMENT_TABLE: OnceCell<HashMap<&str, Vec<&str>>> = OnceCell::new();

    let table = LANGUAGE_COMMENT_TABLE.get_or_init(|| {
        serde_json::from_str(include_str!("../../../scripts/dumb_jump/comments_map.json"))
            .expect("Wrong path for comments_map.json")
    });

    table
        .get(ext)
        .unwrap_or_else(|| table.get("*").expect("`*` entry exists; qed"))
}

/// Return `true` if the line is a comment.
pub fn is_comment(line: &str, file_ext: &str) -> bool {
    get_comment_syntax(file_ext)
        .iter()
        .any(|comment_syntax| line.trim_start().starts_with(comment_syntax))
}
