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
mod bonus;

use source_item::SourceItem;

pub use self::algo::*;
pub use self::bonus::language::Language;
pub use self::bonus::Bonus;
pub use source_item::MatchType;

/// Score of base matching algorithm(fzy, skim, etc).
pub type Score = i64;

/// A tuple of (score, matched_indices) for the line has a match given the query string.
pub type MatchResult = Option<(Score, Vec<usize>)>;

/// `Matcher` is composed of two components:
///
///   * `match_type`: represents the way of extracting the matching piece from the raw line.
///   * `algo`: algorithm used for matching the text.
///   * `bonus`: add a bonus to the result of base `algo`.
pub struct Matcher {
    match_type: MatchType,
    algo: Algo,
    bonuses: Vec<Bonus>,
}

impl Matcher {
    /// Constructs a `Matcher`.
    pub fn new(algo: Algo, match_type: MatchType, bonus: Bonus) -> Self {
        Self {
            algo,
            match_type,
            bonuses: vec![bonus],
        }
    }

    pub fn new_with_bonuses(algo: Algo, match_type: MatchType, bonuses: Vec<Bonus>) -> Self {
        Self {
            algo,
            match_type,
            bonuses,
        }
    }

    /// Match the item without considering the bonus.
    #[inline]
    pub fn base_match(&self, item: &SourceItem, query: &str) -> MatchResult {
        self.algo.apply_match(query, item, &self.match_type)
    }

    /// Actually performs the matching algorithm.
    pub fn do_match(&self, item: &SourceItem, query: &str) -> MatchResult {
        self.base_match(item, query).map(|(score, indices)| {
            let total_bonus_score: Score = self
                .bonuses
                .iter()
                .map(|b| b.bonus_for(item, score, &indices))
                .sum();
            (score + total_bonus_score, indices)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fzy;

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

    #[test]
    fn test_filename_bonus() {
        let lines = vec![
            "autoload/clap/filter.vim",
            "autoload/clap/provider/files.vim",
            "lua/fzy_filter.lua",
        ];
        let matcher = Matcher::new(Algo::Fzy, MatchType::Full, Bonus::FileName);
        let query = "fil";
        for line in lines {
            let (base_score, indices1) = matcher.base_match(&line.into(), query).unwrap();
            let (score_with_bonus, indices2) = matcher.do_match(&line.into(), query).unwrap();
            assert!(indices1 == indices2);
            assert!(score_with_bonus > base_score);
        }
    }

    #[test]
    fn test_filetype_bonus() {
        let lines = vec!["hellorsr foo", "function foo"];
        let matcher = Matcher::new(Algo::Fzy, MatchType::Full, Bonus::Language("vim".into()));
        let query = "fo";
        for line in lines {
            let (base_score, indices1) = matcher.base_match(&line.into(), query).unwrap();
            let (score_with_bonus, indices2) = matcher.do_match(&line.into(), query).unwrap();
            println!(
                "base_score: {}, score_with_bonus: {}",
                base_score, score_with_bonus
            );
            assert!(indices1 == indices2);
            // assert!(score_with_bonus > base_score);
        }
    }
}
