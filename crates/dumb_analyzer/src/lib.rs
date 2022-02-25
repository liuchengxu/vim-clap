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

// TODO: More general precise reference resolution.
/// Returns a tuple of (ref_kind, kind_weight) given the pattern and source file extension.
pub fn resolve_reference_kind(pattern: impl AsRef<str>, file_ext: &str) -> (&'static str, usize) {
    let pattern = pattern.as_ref();

    let maybe_more_precise_kind = match file_ext {
        "rs" => {
            let pattern = pattern.trim_start();
            // use foo::bar;
            // pub(crate) use foo::bar;
            if pattern.starts_with("use ")
                || (pattern.starts_with("pub")
                    && pattern
                        .split_ascii_whitespace()
                        .take(2)
                        .last()
                        .map(|e| e == "use")
                        .unwrap_or(false))
            {
                Some(("use", 1))
            } else if pattern.starts_with("impl") {
                Some(("impl", 2))
            } else {
                None
            }
        }
        _ => None,
    };

    maybe_more_precise_kind.unwrap_or(("refs", 100))
}
