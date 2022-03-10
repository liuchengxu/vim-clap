//! This crate provides various matcher algorithms for line oriented search given the query string.
//!
//! The matcher result consists of the score and the indices of matched items.
//!
//! There two steps to match a line:
//!
//! //     RawSearchLine
//! //        |
//! //        |
//! //        |
//! //        ↓
//! //    Apply exact/inverse search term
//! //        |
//! //        |
//! //        |
//! //        ↓
//! //    Apply fuzzy search term
//! //        |
//! //        |  MatchingTextKind: extract the content to match.
//! //        |  FuzzyAlgorithm: run the match algorithm on MatchText.
//! //        |
//! //        ↓
//! //   MatchResult
//!

mod algo;
mod bonus;

pub use self::algo::{fzy, skim, substring, FuzzyAlgorithm};
pub use self::bonus::cwd::Cwd;
pub use self::bonus::language::Language;
pub use self::bonus::Bonus;
use types::FilteredItem;
// Re-export types
pub use types::{
    ExactTerm, ExactTermType, FuzzyTermType, MatchingText, MatchingTextKind, Query, SearchTerm,
    SourceItem, TermType,
};

/// Score of base matching algorithm(fzy, skim, etc).
pub type Score = i64;

/// A tuple of (score, matched_indices) for the line has a match given the query string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    pub score: Score,
    pub indices: Vec<usize>,
}

impl MatchResult {
    pub fn new(score: Score, indices: Vec<usize>) -> Self {
        Self { score, indices }
    }

    pub fn into_filtered_item<I: Into<SourceItem>>(self, item: I) -> FilteredItem {
        (item, self.score, self.indices).into()
    }
}

// TODO: the shorter search line has a higher score for the exact matching?
pub fn match_exact_terms<'a>(
    terms: impl Iterator<Item = &'a ExactTerm>,
    full_search_line: &str,
) -> Option<(Score, Vec<usize>)> {
    use ExactTermType::*;

    let mut indices = Vec::<usize>::new();
    let mut exact_score = Score::default();

    for term in terms {
        let sub_query = &term.word;

        match term.ty {
            Exact => {
                if let Some((score, sub_indices)) =
                    substring::substr_indices(full_search_line, sub_query)
                {
                    indices.extend_from_slice(&sub_indices);
                    exact_score += std::cmp::max(score, sub_query.len() as Score);
                } else {
                    return None;
                }
            }
            PrefixExact => {
                let trimmed = full_search_line.trim_start();
                let white_space_len = if full_search_line.len() > trimmed.len() {
                    full_search_line.len() - trimmed.len()
                } else {
                    0
                };
                if trimmed.starts_with(sub_query) {
                    let new_len = indices.len() + sub_query.len();
                    let mut start = -1i32 + white_space_len as i32;
                    indices.resize_with(new_len, || {
                        start += 1;
                        start as usize
                    });
                    exact_score += sub_query.len() as Score;
                } else {
                    return None;
                }
            }
            SuffixExact => {
                let trimmed = full_search_line.trim_end();

                let white_space_len = if full_search_line.len() > trimmed.len() {
                    full_search_line.len() - trimmed.len()
                } else {
                    0
                };

                if trimmed.ends_with(sub_query) {
                    let total_len = full_search_line.len();
                    // In case of underflow, we use i32 here.
                    let mut start =
                        total_len as i32 - sub_query.len() as i32 - 1i32 - white_space_len as i32;
                    let new_len = indices.len() + sub_query.len();
                    indices.resize_with(new_len, || {
                        start += 1;
                        start as usize
                    });
                    exact_score += sub_query.len() as Score;
                } else {
                    return None;
                }
            }
        }
    }

    Some((exact_score, indices))
}

/// `Matcher` is composed of two components:
///
///   * `matching_text_kind`: represents the way of extracting the matching piece from the raw line.
///   * `algo`: algorithm used for matching the text.
///   * `bonus`: add a bonus to the result of base `algo`.
#[derive(Debug, Clone)]
pub struct Matcher {
    bonuses: Vec<Bonus>,
    fuzzy_algo: FuzzyAlgorithm,
    matching_text_kind: MatchingTextKind,
}

impl Matcher {
    /// Constructs a new instance of [`Matcher`].
    pub fn new(
        bonus: Bonus,
        fuzzy_algo: FuzzyAlgorithm,
        matching_text_kind: MatchingTextKind,
    ) -> Self {
        Self {
            bonuses: vec![bonus],
            fuzzy_algo,
            matching_text_kind,
        }
    }

    /// Constructs a new instance of [`Matcher`] with multiple bonuses.
    pub fn with_bonuses(
        bonuses: Vec<Bonus>,
        fuzzy_algo: FuzzyAlgorithm,
        matching_text_kind: MatchingTextKind,
    ) -> Self {
        Self {
            bonuses,
            fuzzy_algo,
            matching_text_kind,
        }
    }

    /// Match the item without considering the bonus.
    #[inline]
    fn fuzzy_match<'a, T: MatchingText<'a>>(&self, item: &T, query: &str) -> Option<MatchResult> {
        self.fuzzy_algo
            .fuzzy_match(query, item, &self.matching_text_kind)
    }

    /// Returns the sum of bonus score.
    fn calc_bonus<'a, T: MatchingText<'a>>(
        &self,
        item: &T,
        base_score: Score,
        base_indices: &[usize],
    ) -> Score {
        self.bonuses
            .iter()
            .map(|b| b.bonus_score(item, base_score, base_indices))
            .sum()
    }

    /// Actually performs the matching algorithm.
    pub fn match_query<'a, T: MatchingText<'a>>(
        &self,
        item: &T,
        query: &Query,
    ) -> Option<MatchResult> {
        // Try the inverse terms against the full search line.
        for inverse_term in query.inverse_terms.iter() {
            if inverse_term.match_full_line(item.full_text()) {
                return None;
            }
        }

        // Try the exact terms against the full search line.
        let (exact_score, mut indices) =
            match match_exact_terms(query.exact_terms.iter(), item.full_text()) {
                Some(ret) => ret,
                None => return None,
            };

        // Try the fuzzy terms against the matched text.
        let mut fuzzy_indices = Vec::with_capacity(query.fuzzy_len());
        let mut fuzzy_score = Score::default();

        for term in query.fuzzy_terms.iter() {
            let query = &term.word;
            if let Some(MatchResult { score, indices }) = self.fuzzy_match(item, query) {
                fuzzy_indices.extend_from_slice(&indices);
                fuzzy_score += score;
            } else {
                return None;
            }
        }

        if fuzzy_indices.is_empty() {
            let bonus_score = self.calc_bonus(item, exact_score, &indices);

            indices.sort_unstable();
            indices.dedup();

            Some(MatchResult::new(exact_score + bonus_score, indices))
        } else {
            fuzzy_indices.sort_unstable();
            fuzzy_indices.dedup();

            let bonus_score = self.calc_bonus(item, fuzzy_score, &fuzzy_indices);

            indices.extend_from_slice(fuzzy_indices.as_slice());
            indices.sort_unstable();
            indices.dedup();

            Some(MatchResult::new(
                exact_score + bonus_score + fuzzy_score,
                indices,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fzy;

    #[test]
    fn test_resize() {
        let total_len = 100;
        let sub_query = "hello";

        let new_indices1 = {
            let mut indices = [1, 2, 3].to_vec();
            let sub_indices = (total_len - sub_query.len()..total_len).collect::<Vec<_>>();
            indices.extend_from_slice(&sub_indices);
            indices
        };

        let new_indices2 = {
            let mut indices = [1, 2, 3].to_vec();
            let mut start = total_len - sub_query.len() - 1;
            let new_len = indices.len() + sub_query.len();
            indices.resize_with(new_len, || {
                start += 1;
                start
            });
            indices
        };

        assert_eq!(new_indices1, new_indices2);
    }

    #[test]
    fn test_matching_text_kind_ignore_file_path() {
        fn apply_on_grep_line_fzy(item: &SourceItem, query: &str) -> Option<MatchResult> {
            FuzzyAlgorithm::Fzy.fuzzy_match(query, item, &MatchingTextKind::IgnoreFilePath)
        }

        let query = "rules";
        let line = "crates/maple_cli/src/lib.rs:2:1:macro_rules! println_json {";
        let match_result1 = fzy::fuzzy_indices(line, query).unwrap();
        let match_result2 = apply_on_grep_line_fzy(&line.to_string().into(), query).unwrap();
        assert_eq!(match_result1.indices, match_result2.indices);
        assert!(match_result2.score > match_result1.score);
    }

    #[test]
    fn test_matching_text_kind_filename() {
        fn apply_on_file_line_fzy(item: &SourceItem, query: &str) -> Option<MatchResult> {
            FuzzyAlgorithm::Fzy.fuzzy_match(query, item, &MatchingTextKind::FileName)
        }

        let query = "lib";
        let line = "crates/extracted_fzy/src/lib.rs";
        let match_result1 = fzy::fuzzy_indices(line, query).unwrap();
        let match_result2 = apply_on_file_line_fzy(&line.to_string().into(), query).unwrap();
        assert_eq!(match_result1.indices, match_result2.indices);
        assert!(match_result2.score > match_result1.score);
    }

    #[test]
    fn test_filename_bonus() {
        let lines = vec![
            "autoload/clap/filter.vim",
            "autoload/clap/provider/files.vim",
            "lua/fzy_filter.lua",
        ];
        let matcher = Matcher::new(Bonus::FileName, FuzzyAlgorithm::Fzy, MatchingTextKind::Full);
        let query = "fil";
        for line in lines {
            let match_result_base = matcher.fuzzy_match(&SourceItem::from(line), query).unwrap();
            let match_result_with_bonus = matcher.match_query(&line, &query.into()).unwrap();
            assert!(match_result_base.indices == match_result_with_bonus.indices);
            assert!(match_result_with_bonus.score > match_result_base.score);
        }
    }

    #[test]
    fn test_language_keyword_bonus() {
        let lines = vec!["hellorsr foo", "function foo"];
        let matcher = Matcher::new(
            Bonus::Language("vim".into()),
            FuzzyAlgorithm::Fzy,
            MatchingTextKind::Full,
        );
        let query: Query = "fo".into();
        let match_result1 = matcher.match_query(&lines[0], &query).unwrap();
        let match_result2 = matcher.match_query(&lines[1], &query).unwrap();
        assert!(match_result1.indices == match_result2.indices);
        assert!(match_result1.score < match_result2.score);
    }

    #[test]
    fn test_search_syntax() {
        let items: Vec<SourceItem> = vec![
            "autoload/clap/provider/search_history.vim".into(),
            "autoload/clap/provider/files.vim".into(),
            "vim-clap/crates/matcher/src/algo.rs".into(),
            "pythonx/clap/scorer.py".into(),
        ];

        let matcher = Matcher::new(Bonus::FileName, FuzzyAlgorithm::Fzy, MatchingTextKind::Full);

        let query: Query = "clap .vim$ ^auto".into();
        let matched_results: Vec<_> = items
            .iter()
            .map(|item| matcher.match_query(item, &query))
            .collect();

        assert_eq!(
            vec![
                Some(MatchResult::new(
                    751,
                    [0, 1, 2, 3, 9, 10, 11, 12, 37, 38, 39, 40].to_vec()
                )),
                Some(MatchResult::new(
                    760,
                    [0, 1, 2, 3, 9, 10, 11, 12, 28, 29, 30, 31].to_vec()
                )),
                None,
                None
            ],
            matched_results
        );

        let query: Query = ".rs$".into();
        let matched_results: Vec<_> = items
            .iter()
            .map(|item| matcher.match_query(item, &query))
            .collect();

        assert_eq!(
            vec![
                None,
                None,
                Some(MatchResult::new(4, [32, 33, 34].to_vec())),
                None
            ],
            matched_results
        );

        let query: Query = "py".into();
        let matched_results: Vec<_> = items
            .iter()
            .map(|item| matcher.match_query(item, &query))
            .collect();

        assert_eq!(
            vec![
                Some(MatchResult::new(126, [14, 36].to_vec())),
                None,
                None,
                Some(MatchResult::new(360, [0, 1].to_vec()))
            ],
            matched_results
        );

        let query: Query = "'py".into();
        let matched_results: Vec<_> = items
            .iter()
            .map(|item| matcher.match_query(item, &query))
            .collect();

        assert_eq!(
            vec![None, None, None, Some(MatchResult::new(2, [0, 1].to_vec()))],
            matched_results
        );
    }
}
