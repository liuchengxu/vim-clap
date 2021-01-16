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
use structopt::clap::arg_enum;

pub use algo::*;
pub use source_item::MatchType;

pub type Score = i64;

/// A tuple of (score, matched_indices) for the line has a match given the query string.
pub type MatchResult = Option<(Score, Vec<usize>)>;

/// Calculates the bonus score given the match result of base algorithm.
pub fn calculate_bonus(bonus: &Bonus, item: &SourceItem, score: Score, indices: &[usize]) -> Score {
    match bonus {
        Bonus::FileName => {
            if let Some((_, idx)) = pattern::file_name_only(&item.raw) {
                let hits = indices.iter().filter(|x| **x >= idx).collect::<Vec<_>>();
                if item.raw.len() > idx {
                    // bonus = base_score * len(matched elements in filename) / len(filename)
                    let bonus = score * hits.len() as i64 / (item.raw.len() - idx) as i64;
                    bonus
                } else {
                    0
                }
            } else {
                0
            }
        }
        Bonus::None => 0,
    }
}

arg_enum! {
  #[derive(Debug, Clone)]
  pub enum Bonus {
      // Give a bonus if the needle matches in the basename of the haystack.
      //
      // Ref https://github.com/liuchengxu/vim-clap/issues/561
      FileName,

      // No additional bonus.
      None,
  }
}

impl Default for Bonus {
    fn default() -> Self {
        Self::None
    }
}

impl From<String> for Bonus {
    fn from(b: String) -> Self {
        b.as_str().into()
    }
}

impl From<&str> for Bonus {
    fn from(b: &str) -> Self {
        match b.to_uppercase().as_str() {
            "NONE" => Self::None,
            "FILENAME" => Self::FileName,
            _ => Self::None,
        }
    }
}

/// `Matcher` is composed of two components:
///
///   * `match_type`: represents the way of extracting the matching piece from the raw line.
///   * `algo`: algorithm used for matching the text.
///   * `bonus`: add a bonus to the result of base `algo`.
pub struct Matcher {
    match_type: MatchType,
    algo: Algo,
    bonus: Bonus,
}

impl Matcher {
    /// Constructs a `Matcher`.
    pub fn new(algo: Algo, match_type: MatchType, bonus: Bonus) -> Self {
        Self {
            algo,
            match_type,
            bonus,
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
            let bonus_score = calculate_bonus(&self.bonus, item, score, &indices);
            (score + bonus_score, indices)
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
}
