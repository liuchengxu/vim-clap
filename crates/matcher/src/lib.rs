//! This crate provides various matcher algorithms for line oriented search given the query string.
//!
//! The matcher result consists of the score and the indices of matched items.
//!
//! There two steps to match a line:
//!
//! //     RawLine
//! //        |
//! //        |  MatchType: extract the content to match.
//! //        |
//! //        ↓
//! //    MatchText
//! //        |
//! //        |      Algo: run the match algorithm on MatchText.
//! //        |
//! //        ↓
//! //   MatchResult
//!

mod algo;

use source_item::SourceItem;

pub use algo::*;
pub use source_item::MatchType;

/// A tuple of (score, matched_indices) for the line has a match given the query string.
pub type MatchResult = Option<(i64, Vec<usize>)>;

/// `Matcher` is composed of two components:
///
///   * `algo`: algorithm used for matching the text.
///   * `match_type`: represents the way of extracting the matching piece from the raw line.
pub struct Matcher {
    algo: Algo,
    match_type: MatchType,
}

impl Matcher {
    /// Constructs a `Matcher`.
    pub fn new(algo: Algo, match_type: MatchType) -> Self {
        Self { algo, match_type }
    }

    /// Actually performs the matching algorithm.
    pub fn do_match(&self, item: &SourceItem, query: &str) -> MatchResult {
        self.algo.apply_match(query, item, &self.match_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fzy, skim::fuzzy_indices as fuzzy_indices_skim, substring::substr_indices};
    use pattern::{file_name_only, strip_grep_filepath, tag_name_only};

    #[test]
    fn test_exclude_grep_filepath() {
        fn apply_on_grep_line_fzy(item: &SourceItem, query: &str) -> MatchResult {
            Algo::Fzy.apply_match(query, item, &MatchType::IgnoreFilePath)
        }

        let query = "rules";
        let line = "crates/maple_cli/src/lib.rs:2:1:macro_rules! println_json {";
        let (_, origin_indices) = fzy::fuzzy_indices(line, query).unwrap();
        let (_, indices) = apply_on_grep_line_fzy(&line.to_string().into(), query).unwrap();
        assert_eq!(origin_indices, indices);
    }

    #[test]
    fn test_file_name_only() {
        fn apply_on_file_line_fzy(item: &SourceItem, query: &str) -> MatchResult {
            Algo::Fzy.apply_match(query, item, &MatchType::FileName)
        }

        let query = "lib";
        let line = "crates/extracted_fzy/src/lib.rs";
        let (_, origin_indices) = fzy::fuzzy_indices(line, query).unwrap();
        let (_, indices) = apply_on_file_line_fzy(&line.to_string().into(), query).unwrap();
        assert_eq!(origin_indices, indices);
    }
}
