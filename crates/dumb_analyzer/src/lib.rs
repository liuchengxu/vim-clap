//! Poor man's language analyzer.

use std::collections::HashMap;

use once_cell::sync::OnceCell;

mod keywords;

/// General weight for fine-grained resolved result.
///
/// Lower is better, the better results will be displayed first.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Weight(usize);

impl Weight {
    pub fn as_i64(self) -> i64 {
        self.0 as i64
    }
}

impl Default for Weight {
    fn default() -> Self {
        Self(1000usize)
    }
}

impl From<usize> for Weight {
    fn from(weight: usize) -> Self {
        Self(weight)
    }
}

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

// TODO: language keyword lookup
//
// https://github.com/e3b0c442/keywords#rust-146-53-keywords
/// Calculates the weight of a specific pattern.
pub fn calculate_pattern_weight(pattern: impl AsRef<str>, file_ext: &str) -> Option<Weight> {
    let weight_fn = match file_ext {
        "vim" => keywords::viml::token_weight,
        "rs" => keywords::rust::token_weight,
        _ => return None,
    };

    // Try the first 3 items because:
    //
    // 1. blines provider prepends the line number to the original line and the language bonus
    //    is mostly used in the blines provider.
    // 2. Languages like Rust has the visibility before the commen keyword(fn, struct, ...).
    pattern
        .as_ref()
        .trim_start()
        .split_whitespace()
        .take(3)
        .find_map(weight_fn)
        .map(Into::into)
}
