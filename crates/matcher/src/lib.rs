//! This crate provides various matcher algorithms for line oriented search given the query string.
//!
//! The matcher result consists of the score and the indices of matched items.
//!
//! There two steps to match a line:
//!
//! //     arc<dyn ClapItem>
//! //        |
//! //        |
//! //        |
//! //        ↓
//! //    Apply InverseSearchTerms
//! //        |
//! //        |
//! //        |
//! //        ↓
//! //    Apply ExactSearchTerms
//! //        |
//! //        |
//! //        |
//! //        ↓
//! //    Apply FuzzyTerms
//! //        |
//! //        |  MatchScope: extract the content to match.
//! //        |  FuzzyAlgorithm: run the match algorithm on FuzzyText.
//! //        |
//! //        ↓
//! //   MatchResult
//!

mod algo;
mod bonus;

use std::sync::Arc;

pub use self::algo::{fzy, skim, substring, FuzzyAlgorithm};
pub use self::bonus::cwd::Cwd;
pub use self::bonus::language::Language;
pub use self::bonus::Bonus;
use types::{CaseMatching, MatchedItem};
// Re-export types
pub use types::{
    ClapItem, ExactTerm, ExactTermType, FuzzyTermType, MatchResult, MatchScope, MultiItem, Query,
    Score, SearchTerm, TermType,
};

/// Returns an optional tuple of (score, indices) if all the exact searching terms are satisfied.
pub fn match_exact_terms<'a>(
    terms: impl Iterator<Item = &'a ExactTerm>,
    full_search_line: &str,
    case_matching: CaseMatching,
) -> Option<(Score, Vec<usize>)> {
    use ExactTermType::*;

    let mut indices = Vec::<usize>::new();
    let mut exact_score = Score::default();

    for term in terms {
        let sub_query = &term.word;

        match term.ty {
            Exact => {
                if let Some((score, sub_indices)) =
                    substring::substr_indices(full_search_line, sub_query, case_matching)
                {
                    indices.extend_from_slice(&sub_indices);
                    exact_score += score.max(sub_query.len() as Score);
                } else {
                    return None;
                }
            }
            PrefixExact => {
                let trimmed = full_search_line.trim_start();
                let white_space_len = full_search_line.len().saturating_sub(trimmed.len());
                if trimmed.starts_with(sub_query) {
                    let mut match_start = -1i32 + white_space_len as i32;
                    let new_len = indices.len() + sub_query.len();
                    indices.resize_with(new_len, || {
                        match_start += 1;
                        match_start as usize
                    });
                    exact_score += sub_query.len() as Score;
                } else {
                    return None;
                }
            }
            SuffixExact => {
                let total_len = full_search_line.len();
                let trimmed = full_search_line.trim_end();
                let white_space_len = total_len.saturating_sub(trimmed.len());
                if trimmed.ends_with(sub_query) {
                    // In case of underflow, we use i32 here.
                    let mut match_start =
                        total_len as i32 - sub_query.len() as i32 - 1i32 - white_space_len as i32;
                    let new_len = indices.len() + sub_query.len();
                    indices.resize_with(new_len, || {
                        match_start += 1;
                        match_start as usize
                    });
                    exact_score += sub_query.len() as Score;
                } else {
                    return None;
                }
            }
        }
    }

    // Exact search term bonus
    //
    // The shorter search line has a higher score.
    exact_score += (512 / full_search_line.len()) as Score;

    Some((exact_score, indices))
}

/// `Matcher` is composed of two components:
///
///   * `match_scope`: represents the way of extracting the matching piece from the raw line.
///   * `algo`: algorithm used for matching the text.
///   * `bonus`: add a bonus to the result of base `algo`.
#[derive(Debug, Clone, Default)]
pub struct Matcher {
    bonuses: Vec<Bonus>,
    fuzzy_algo: FuzzyAlgorithm,
    match_scope: MatchScope,
    case_matching: CaseMatching,
}

impl Matcher {
    /// Constructs a new instance of [`Matcher`].
    pub fn new(bonus: Bonus, fuzzy_algo: FuzzyAlgorithm, match_scope: MatchScope) -> Self {
        Self {
            bonuses: vec![bonus],
            fuzzy_algo,
            match_scope,
            case_matching: Default::default(),
        }
    }

    /// Constructs a new instance of [`Matcher`] with multiple bonuses.
    pub fn with_bonuses(
        bonuses: Vec<Bonus>,
        fuzzy_algo: FuzzyAlgorithm,
        match_scope: MatchScope,
    ) -> Self {
        Self {
            bonuses,
            fuzzy_algo,
            match_scope,
            case_matching: Default::default(),
        }
    }

    pub fn set_bonuses(mut self, bonuses: Vec<Bonus>) -> Self {
        self.bonuses = bonuses;
        self
    }

    pub fn set_match_scope(mut self, match_scope: MatchScope) -> Self {
        self.match_scope = match_scope;
        self
    }

    pub fn set_case_matching(mut self, case_matching: CaseMatching) -> Self {
        self.case_matching = case_matching;
        self
    }

    /// Returns the sum of bonus score.
    fn calc_bonus(
        &self,
        item: &Arc<dyn ClapItem>,
        base_score: Score,
        base_indices: &[usize],
    ) -> Score {
        self.bonuses
            .iter()
            .map(|b| b.bonus_score(item, base_score, base_indices))
            .sum()
    }

    /// Actually performs the matching algorithm.
    pub fn match_item(&self, item: Arc<dyn ClapItem>, query: &Query) -> Option<MatchedItem> {
        let match_text = item.match_text();

        if match_text.is_empty() {
            return None;
        }

        // Try the inverse terms against the full search line.
        for inverse_term in query.inverse_terms.iter() {
            if inverse_term.match_full_line(match_text) {
                return None;
            }
        }

        // Try the exact terms against the full search line.
        let (exact_score, mut indices) =
            match_exact_terms(query.exact_terms.iter(), match_text, self.case_matching)?;

        // Try the fuzzy terms against the matched text.
        let mut fuzzy_indices = Vec::with_capacity(query.fuzzy_len());
        let mut fuzzy_score = Score::default();

        if let Some(ref fuzzy_text) = item.fuzzy_text(self.match_scope) {
            for term in query.fuzzy_terms.iter() {
                let query = &term.word;
                if let Some(MatchResult { score, indices }) =
                    self.fuzzy_algo
                        .fuzzy_match(query, fuzzy_text, self.case_matching)
                {
                    fuzzy_indices.extend_from_slice(&indices);
                    fuzzy_score += score;
                } else {
                    return None;
                }
            }
        }

        let match_result = if fuzzy_indices.is_empty() {
            let bonus_score = self.calc_bonus(&item, exact_score, &indices);

            indices.sort_unstable();
            indices.dedup();

            MatchResult::new(exact_score + bonus_score, indices)
        } else {
            fuzzy_indices.sort_unstable();
            fuzzy_indices.dedup();

            let bonus_score = self.calc_bonus(&item, fuzzy_score, &fuzzy_indices);

            indices.extend_from_slice(fuzzy_indices.as_slice());
            indices.sort_unstable();
            indices.dedup();

            MatchResult::new(exact_score + bonus_score + fuzzy_score, indices)
        };

        let MatchResult { score, indices } = item.match_result_callback(match_result);

        Some(MatchedItem::new(item, score, indices))
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
    fn test_match_scope_grep_line() {
        let query = "rules";
        let line = "crates/maple_cli/src/lib.rs:2:1:macro_rules! println_json {";
        let matched_item1 = fzy::fuzzy_indices(line, query, CaseMatching::Smart).unwrap();

        let item = MultiItem::from(line.to_string());
        let fuzzy_text = item.fuzzy_text(MatchScope::GrepLine).unwrap();
        let matched_item2 = FuzzyAlgorithm::Fzy
            .fuzzy_match(query, &fuzzy_text, CaseMatching::Smart)
            .unwrap();

        assert_eq!(matched_item1.indices, matched_item2.indices);
        assert!(matched_item2.score > matched_item1.score);
    }

    #[test]
    fn test_match_scope_filename() {
        let query = "lib";
        let line = "crates/extracted_fzy/src/lib.rs";
        let matched_item1 = fzy::fuzzy_indices(line, query, CaseMatching::Smart).unwrap();

        let item = MultiItem::from(line.to_string());
        let fuzzy_text = item.fuzzy_text(MatchScope::FileName).unwrap();
        let matched_item2 = FuzzyAlgorithm::Fzy
            .fuzzy_match(query, &fuzzy_text, CaseMatching::Smart)
            .unwrap();

        assert_eq!(matched_item1.indices, matched_item2.indices);
        assert!(matched_item2.score > matched_item1.score);
    }

    #[test]
    fn test_filename_bonus() {
        let lines = vec![
            "autoload/clap/filter.vim",
            "autoload/clap/provider/files.vim",
            "lua/fzy_filter.lua",
        ];
        let matcher = Matcher::new(Bonus::FileName, FuzzyAlgorithm::Fzy, MatchScope::Full);
        let query = "fil";
        for line in lines {
            let item: Arc<dyn ClapItem> = Arc::new(MultiItem::from(line.to_string()));
            let fuzzy_text = item.fuzzy_text(matcher.match_scope).unwrap();
            let match_result_base = matcher
                .fuzzy_algo
                .fuzzy_match(query, &fuzzy_text, matcher.case_matching)
                .unwrap();
            let match_result_with_bonus = matcher.match_item(item, &query.into()).unwrap();
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
            MatchScope::Full,
        );
        let query: Query = "fo".into();
        let matched_item1 = matcher
            .match_item(Arc::new(lines[0]) as Arc<dyn ClapItem>, &query)
            .unwrap();
        let matched_item2 = matcher
            .match_item(Arc::new(lines[1]) as Arc<dyn ClapItem>, &query)
            .unwrap();
        assert!(matched_item1.indices == matched_item2.indices);
        assert!(matched_item1.score < matched_item2.score);
    }

    #[test]
    fn test_exact_search_term_bonus() {
        let lines = vec!["function foo qwer", "function foo"];
        let matcher = Matcher::new(Default::default(), FuzzyAlgorithm::Fzy, MatchScope::Full);
        let query: Query = "'fo".into();
        let matched_item1 = matcher
            .match_item(Arc::new(lines[0]) as Arc<dyn ClapItem>, &query)
            .unwrap();
        let matched_item2 = matcher
            .match_item(Arc::new(lines[1]) as Arc<dyn ClapItem>, &query)
            .unwrap();
        assert!(matched_item1.indices == matched_item2.indices);
        assert!(matched_item1.score < matched_item2.score);
    }

    #[test]
    fn test_search_syntax() {
        let items = vec![
            Arc::new("autoload/clap/provider/search_history.vim"),
            Arc::new("autoload/clap/provider/files.vim"),
            Arc::new("vim-clap/crates/matcher/src/algo.rs"),
            Arc::new("pythonx/clap/scorer.py"),
        ];

        let matcher = Matcher::new(Bonus::FileName, FuzzyAlgorithm::Fzy, MatchScope::Full);

        let match_with_query = |query: &Query| {
            items
                .clone()
                .into_iter()
                .map(|item| {
                    let item: Arc<dyn ClapItem> = item;
                    matcher.match_item(item, &query)
                })
                .map(|maybe_matched_item| {
                    if let Some(matched_item) = maybe_matched_item {
                        Some(MatchResult::new(matched_item.score, matched_item.indices))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };

        let query: Query = "clap .vim$ ^auto".into();
        let match_results: Vec<_> = match_with_query(&query);
        assert_eq!(
            vec![
                Some(MatchResult::new(
                    763,
                    [0, 1, 2, 3, 9, 10, 11, 12, 37, 38, 39, 40].to_vec()
                )),
                Some(MatchResult::new(
                    776,
                    [0, 1, 2, 3, 9, 10, 11, 12, 28, 29, 30, 31].to_vec()
                )),
                None,
                None
            ],
            match_results
        );

        let query: Query = ".rs$".into();
        let match_results: Vec<_> = match_with_query(&query);
        assert_eq!(
            vec![
                None,
                None,
                Some(MatchResult::new(24, [32, 33, 34].to_vec())),
                None
            ],
            match_results
        );

        let query: Query = "py".into();
        let match_results: Vec<_> = match_with_query(&query);
        assert_eq!(
            vec![
                Some(MatchResult::new(138, [14, 36].to_vec())),
                None,
                None,
                Some(MatchResult::new(383, [0, 1].to_vec()))
            ],
            match_results
        );

        let query: Query = "'py".into();
        let match_results: Vec<_> = match_with_query(&query);
        assert_eq!(
            vec![
                None,
                None,
                None,
                Some(MatchResult::new(25, [0, 1].to_vec()))
            ],
            match_results
        );
    }
}
