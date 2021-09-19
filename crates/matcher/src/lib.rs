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
//! //        |  MatchType: extract the content to match.
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
// Re-export types
pub use types::{
    ExactTerm, ExactTermType, FuzzyTermType, MatchType, MatchingText, Query, SearchTerm,
    SourceItem, TermType,
};

/// Score of base matching algorithm(fzy, skim, etc).
pub type Score = i64;

/// A tuple of (score, matched_indices) for the line has a match given the query string.
pub type MatchResult = Option<(Score, Vec<usize>)>;

// TODO: the shorter search line has a higher score for the exact matching?
pub fn search_exact_terms<'a>(
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
///   * `match_type`: represents the way of extracting the matching piece from the raw line.
///   * `algo`: algorithm used for matching the text.
///   * `bonus`: add a bonus to the result of base `algo`.
#[derive(Debug, Clone)]
pub struct Matcher {
    fuzzy_algo: FuzzyAlgorithm,
    match_type: MatchType,
    bonuses: Vec<Bonus>,
}

impl Matcher {
    /// Constructs a new instance of [`Matcher`].
    pub fn new(fuzzy_algo: FuzzyAlgorithm, match_type: MatchType, bonus: Bonus) -> Self {
        Self {
            fuzzy_algo,
            match_type,
            bonuses: vec![bonus],
        }
    }

    /// Constructs a new instance of [`Matcher`] with multiple bonuses.
    pub fn with_bonuses(
        fuzzy_algo: FuzzyAlgorithm,
        match_type: MatchType,
        bonuses: Vec<Bonus>,
    ) -> Self {
        Self {
            fuzzy_algo,
            match_type,
            bonuses,
        }
    }

    /// Match the item without considering the bonus.
    #[inline]
    fn fuzzy_match<'a, T: MatchingText<'a>>(&self, item: &T, query: &str) -> MatchResult {
        self.fuzzy_algo.fuzzy_match(query, item, &self.match_type)
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
    pub fn match_query<'a, T: MatchingText<'a>>(&self, item: &T, query: &Query) -> MatchResult {
        // Try the inverse terms against the full search line.
        for inverse_term in query.inverse_terms.iter() {
            if inverse_term.match_full_line(item.full_text()) {
                return None;
            }
        }

        // Try the exact terms against the full search line.
        let (exact_score, mut indices) =
            match search_exact_terms(query.exact_terms.iter(), item.full_text()) {
                Some(ret) => ret,
                None => return None,
            };

        // Try the fuzzy terms against the matched text.
        let mut fuzzy_indices = Vec::<usize>::new();
        let mut fuzzy_score = Score::default();

        for term in query.fuzzy_terms.iter() {
            let query = &term.word;
            if let Some((score, sub_indices)) = self.fuzzy_match(item, query) {
                fuzzy_indices.extend_from_slice(&sub_indices);
                fuzzy_score += score;
            } else {
                return None;
            }
        }

        if fuzzy_indices.is_empty() {
            let bonus_score = self.calc_bonus(item, exact_score, &indices);

            indices.sort_unstable();
            indices.dedup();

            Some((exact_score + bonus_score, indices))
        } else {
            fuzzy_indices.sort_unstable();
            fuzzy_indices.dedup();

            let bonus_score = self.calc_bonus(item, fuzzy_score, &fuzzy_indices);

            indices.extend_from_slice(fuzzy_indices.as_slice());
            indices.sort_unstable();
            indices.dedup();

            Some((exact_score + bonus_score + fuzzy_score, indices))
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
    fn test_exclude_grep_filepath() {
        fn apply_on_grep_line_fzy(item: &SourceItem, query: &str) -> MatchResult {
            FuzzyAlgorithm::Fzy.fuzzy_match(query, item, &MatchType::IgnoreFilePath)
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
            FuzzyAlgorithm::Fzy.fuzzy_match(query, item, &MatchType::FileName)
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
        let matcher = Matcher::new(FuzzyAlgorithm::Fzy, MatchType::Full, Bonus::FileName);
        let query = "fil";
        for line in lines {
            let (base_score, indices1) =
                matcher.fuzzy_match(&SourceItem::from(line), query).unwrap();
            let (score_with_bonus, indices2) = matcher.match_query(&line, &query.into()).unwrap();
            assert!(indices1 == indices2);
            assert!(score_with_bonus > base_score);
        }
    }

    #[test]
    fn test_filetype_bonus() {
        let lines = vec!["hellorsr foo", "function foo"];
        let matcher = Matcher::new(
            FuzzyAlgorithm::Fzy,
            MatchType::Full,
            Bonus::Language("vim".into()),
        );
        let query: Query = "fo".into();
        let (score_1, indices1) = matcher.match_query(&lines[0], &query).unwrap();
        let (score_2, indices2) = matcher.match_query(&lines[1], &query).unwrap();
        assert!(indices1 == indices2);
        assert!(score_1 < score_2);
    }

    #[test]
    fn test_search_syntax() {
        let items: Vec<SourceItem> = vec![
            "autoload/clap/provider/search_history.vim".into(),
            "autoload/clap/provider/files.vim".into(),
            "vim-clap/crates/matcher/src/algo.rs".into(),
            "pythonx/clap/scorer.py".into(),
        ];

        let matcher = Matcher::new(FuzzyAlgorithm::Fzy, MatchType::Full, Bonus::FileName);

        let query: Query = "clap .vim$ ^auto".into();
        let matched_results: Vec<_> = items
            .iter()
            .map(|item| matcher.match_query(item, &query))
            .collect();

        assert_eq!(
            vec![
                Some((751, [0, 1, 2, 3, 9, 10, 11, 12, 37, 38, 39, 40].to_vec())),
                Some((760, [0, 1, 2, 3, 9, 10, 11, 12, 28, 29, 30, 31].to_vec())),
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
            vec![None, None, Some((4, [32, 33, 34].to_vec())), None],
            matched_results
        );

        let query: Query = "py".into();
        let matched_results: Vec<_> = items
            .iter()
            .map(|item| matcher.match_query(item, &query))
            .collect();

        assert_eq!(
            vec![
                Some((126, [14, 36].to_vec())),
                None,
                None,
                Some((360, [0, 1].to_vec()))
            ],
            matched_results
        );

        let query: Query = "'py".into();
        let matched_results: Vec<_> = items
            .iter()
            .map(|item| matcher.match_query(item, &query))
            .collect();

        assert_eq!(
            vec![None, None, None, Some((2, [0, 1].to_vec()))],
            matched_results
        );
    }
}
