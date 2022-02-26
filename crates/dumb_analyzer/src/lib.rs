//! Poor man's language analyzer.

use std::collections::HashMap;

use once_cell::sync::OnceCell;

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

/// Calculates the bonus by checking the first two items.
fn calculate_weight(
    trimmed_line: &str,
    weight_fn: impl Fn(Option<&str>) -> Option<usize>,
) -> Option<Weight> {
    let mut iter = trimmed_line.split_whitespace();

    // Try the first two items because blines provider prepends the line number to the
    // original line and the language bonus is mostly used in the blines provider.
    let first_item = iter.next();

    match weight_fn(first_item) {
        Some(weight) => Some(weight.into()),
        None => {
            let second_item = iter.next();
            weight_fn(second_item).map(Into::into)
        }
    }
}

// TODO: language keyword lookup
//
// https://github.com/e3b0c442/keywords#rust-146-53-keywords
/// Calculates the weight of a specific pattern.
pub fn calculate_pattern_weight(pattern: impl AsRef<str>, file_ext: &str) -> Option<Weight> {
    let trimmed = pattern.as_ref().trim_start();

    // TODO: take care of the comment line universally.
    match file_ext {
        "vim" => {
            let weight_fn = |item: Option<&str>| {
                item.and_then(|s| {
                    // function[!]
                    if s.starts_with("function") {
                        Some(3)
                    } else if s == "let" {
                        Some(6)
                    } else {
                        None
                    }
                })
            };

            calculate_weight(trimmed, weight_fn)
        }

        "rs" => {
            const TYPE: &[&str] = &["type", "mod", "impl"];
            const FUNCTION: &[&str] = &["fn", "macro_rules"];
            const VARIABLE: &[&str] = &["let", "const", "static", "enum", "struct", "trait"];

            let weight_fn = |item: Option<&str>| {
                item.and_then(|s| {
                    if FUNCTION.contains(&s) {
                        Some(4)
                    } else if s.starts_with("pub") {
                        Some(6)
                    } else if TYPE.contains(&s) {
                        Some(7)
                    } else if VARIABLE.contains(&s) {
                        Some(8)
                    } else if s.starts_with("[cfg(feature") {
                        Some(10)
                    } else {
                        None
                    }
                })
            };

            calculate_weight(trimmed, weight_fn)
        }

        _ => None,
    }
}
