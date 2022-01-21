//! Essentially, all the engines here are based on the regexp approach.
//! The difference is that `regex` engine is the poor man's way where we
//! use our own regex pattern rule with the ripgrep executable together,
//! while `ctags` and `gtags` maintain theirs which are well polished.

mod ctags;
mod gtags;
mod regex;

pub use self::ctags::CtagsSearcher;
pub use self::gtags::GtagsSearcher;
pub use self::regex::RegexSearcher;

use std::collections::HashMap;

use once_cell::sync::OnceCell;

/// When spawning the ctags/gtags request, we can specify the searching strategy.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(unused)]
pub enum SearchType {
    /// Prefix match.
    StartWith,
    /// Exact match.
    Exact,
    /// Substring match.
    Contain,
    ///
    Inherit,
}

impl Default for SearchType {
    fn default() -> Self {
        Self::Exact
    }
}

fn rs_kind_alias() -> HashMap<&'static str, &'static str> {
    vec![
        ("module", "mod"),
        ("function", "fn"),
        ("interface", "trait"),
        ("enumerator", "enum"),
        ("implementation", "impl"),
    ]
    .into_iter()
    .collect()
}

// Make the kind field shorter to save more spaces for the other fields.
fn get_kind_alias<'a>(extension: &'a str, kind: &'a str) -> Option<&'a &'static str> {
    static KIND_MAP: OnceCell<HashMap<&str, HashMap<&str, &str>>> = OnceCell::new();

    let map = KIND_MAP.get_or_init(|| {
        vec![("rs", rs_kind_alias())]
            .into_iter()
            .collect::<HashMap<_, _>>()
    });

    map.get(extension).and_then(|m| m.get(kind))
}

fn compact_kind(maybe_extension: Option<&str>, kind: &str) -> String {
    maybe_extension
        .and_then(|extension| get_kind_alias(extension, kind))
        .unwrap_or(&kind)
        .to_string()
}

/// Parsed from `ctags` and `gtags` output.
#[derive(Default, Debug)]
pub struct TagInfo {
    /// None for `gtags`.
    pub name: Option<String>,
    pub path: String,
    pub pattern: String,
    pub line: usize,
    /// ctags only.
    pub kind: Option<String>,
    /// ctags only.
    pub scope: Option<String>,
}

impl TagInfo {
    /// Parse from the output of `readtags`.
    pub fn from_ctags(s: &str) -> Option<Self> {
        let mut items = s.split('\t');

        let mut l = Self {
            name: Some(items.next()?.into()),
            path: items.next()?.into(),
            ..Default::default()
        };

        // https://docs.ctags.io/en/latest/man/ctags-client-tools.7.html#parse-readtags-output
        if let Some(p) = items
            .clone()
            .peekable()
            .peek()
            .and_then(|p| p.strip_suffix(";\""))
        {
            let search_pattern_used = (p.starts_with('/') && p.ends_with('/'))
                || (p.len() > 1 && p.starts_with('$') && p.ends_with('$'));
            if search_pattern_used {
                let pattern = items.next()?;
                let pattern_len = pattern.len();
                // forward search: `/^foo$/`
                // backward search: `?^foo$?`
                if p.starts_with("/^") || p.starts_with("?^") {
                    if p.ends_with("$/") || p.ends_with("$?") {
                        l.pattern = String::from(&pattern[2..pattern_len - 4]);
                    } else {
                        l.pattern = String::from(&pattern[2..pattern_len - 2]);
                    }
                } else {
                    l.pattern = String::from(&pattern[2..pattern_len]);
                }
            } else {
                return None;
            }
        } else {
            return None;
        }

        let maybe_extension = l.path.rsplit_once('.').map(|(_, extension)| extension);

        for item in items {
            if let Some((k, v)) = item.split_once(':') {
                if v.is_empty() {
                    continue;
                }
                match k {
                    "kind" => l.kind = Some(compact_kind(maybe_extension, v)),
                    "scope" => l.scope = Some(v.into()),
                    "line" => l.line = v.parse().expect("line is an integer"),
                    // Unused for now.
                    "language" | "roles" | "access" | "signature" => {}
                    unknown => {
                        tracing::debug!(line = %s, "Unknown field: {}", unknown);
                    }
                }
            }
        }

        Some(l)
    }

    pub fn from_gtags(s: &str) -> Option<Self> {
        pattern::parse_gtags(s).map(|(line, path, pattern)| Self {
            path: path.into(),
            pattern: pattern.into(),
            line,
            ..Default::default()
        })
    }

    /// Constructs a grep like line with highlighting indices.
    fn grep_format_inner(
        &self,
        kind: &str,
        query: &str,
        ignorecase: bool,
    ) -> (String, Option<Vec<usize>>) {
        let mut formatted = format!("[{}]{}:{}:1:", kind, self.path, self.line);

        let found = if ignorecase {
            self.pattern.to_lowercase().find(&query.to_lowercase())
        } else {
            self.pattern.find(query)
        };

        let indices = if let Some(idx) = found {
            let start = formatted.len() + idx;
            let end = start + query.len();
            Some((start..end).into_iter().collect())
        } else {
            None
        };

        formatted.push_str(&self.pattern);

        (formatted, indices)
    }

    pub fn grep_format_ctags(&self, query: &str, ignorecase: bool) -> (String, Option<Vec<usize>>) {
        let kind = self.kind.as_ref().map(|s| s.as_ref()).unwrap_or("tags");
        self.grep_format_inner(kind, query, ignorecase)
    }

    pub fn grep_format_gtags(
        &self,
        kind: &str,
        query: &str,
        ignorecase: bool,
    ) -> (String, Option<Vec<usize>>) {
        self.grep_format_inner(kind, query, ignorecase)
    }
}
