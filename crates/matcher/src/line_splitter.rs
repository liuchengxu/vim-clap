use crate::{
    fzy, skim::fuzzy_indices as fuzzy_indices_skim, substring::substr_indices, MatcherResult,
};
use structopt::clap::arg_enum;

use crate::matchers::{FileNameMatcher, GrepMatcher, MatchItem, TagNameMatcher};

// Implement arg_enum so that we could control it from the command line.
arg_enum! {
  /// Sometimes we hope to filter on the part of line.
  #[derive(Debug, Clone)]
  pub enum LineSplitter {
      Full,
      TagNameOnly,
      FileNameOnly,
      GrepExcludeFilePath,
  }
}

impl From<&str> for LineSplitter {
    fn from(filtering: &str) -> Self {
        match filtering {
            "Full" => Self::Full,
            "TagNameOnly" => Self::TagNameOnly,
            "FileNameOnly" => Self::FileNameOnly,
            "GrepExcludeFilePath" => Self::GrepExcludeFilePath,
            _ => Self::Full,
        }
    }
}

impl From<String> for LineSplitter {
    fn from(filtering: String) -> Self {
        Self::from(filtering.as_str())
    }
}

fn do_match<'a, M: MatchItem<'a>>(
    matcher: M,
    query: &str,
    fuzzy_algo: impl FnOnce(&str, &str) -> MatcherResult,
) -> MatcherResult {
    matcher
        .match_text()
        .and_then(|match_info| match match_info {
            (text, 0) => fuzzy_algo(text, query),
            (text, offset) => fuzzy_algo(text, query)
                .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect())),
        })
}

#[inline]
pub(super) fn apply_on_grep_line_skim(line: &str, query: &str) -> MatcherResult {
    do_match(GrepMatcher::from(line), query, fuzzy_indices_skim)
}

#[inline]
pub(super) fn apply_on_grep_line_fzy(line: &str, query: &str) -> MatcherResult {
    do_match(GrepMatcher::from(line), query, fzy::fuzzy_indices)
}

#[inline]
pub(super) fn apply_on_grep_line_substr(line: &str, query: &str) -> MatcherResult {
    do_match(GrepMatcher::from(line), query, substr_indices)
}

#[inline]
pub(super) fn apply_on_file_line_skim(line: &str, query: &str) -> MatcherResult {
    do_match(FileNameMatcher::from(line), query, fuzzy_indices_skim)
}

#[inline]
pub(super) fn apply_on_file_line_fzy(line: &str, query: &str) -> MatcherResult {
    do_match(FileNameMatcher::from(line), query, fzy::fuzzy_indices)
}

#[inline]
pub(super) fn apply_on_file_line_substr(line: &str, query: &str) -> MatcherResult {
    do_match(FileNameMatcher::from(line), query, substr_indices)
}

#[inline]
pub(super) fn apply_on_tag_line_skim(line: &str, query: &str) -> MatcherResult {
    do_match(TagNameMatcher::from(line), query, fuzzy_indices_skim)
}

#[inline]
pub(super) fn apply_on_tag_line_fzy(line: &str, query: &str) -> MatcherResult {
    do_match(TagNameMatcher::from(line), query, fzy::fuzzy_indices)
}

#[inline]
pub(super) fn apply_on_tag_line_substr(line: &str, query: &str) -> MatcherResult {
    do_match(TagNameMatcher::from(line), query, substr_indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclude_grep_filepath() {
        let query = "rules";
        let line = "crates/maple_cli/src/lib.rs:2:1:macro_rules! println_json {";
        let (_, origin_indices) = fzy::fuzzy_indices(line, query).unwrap();
        let (_, indices) = apply_on_grep_line_fzy(line, query).unwrap();
        assert_eq!(origin_indices, indices);
    }

    #[test]
    fn test_file_name_only() {
        let query = "lib";
        let line = "crates/extracted_fzy/src/lib.rs";
        let (_, origin_indices) = fzy::fuzzy_indices(line, query).unwrap();
        let (_, indices) = apply_on_file_line_fzy(line, query).unwrap();
        assert_eq!(origin_indices, indices);
    }
}
