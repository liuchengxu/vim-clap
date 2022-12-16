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
//! //    Apply InverseMatcher
//! //        |
//! //        |
//! //        |
//! //        ↓
//! //    Apply ExactMatcher
//! //        |
//! //        |
//! //        |
//! //        ↓
//! //    Apply FuzzyMatcher
//! //        |
//! //        |  MatchScope: extract the content to match.
//! //        |  FuzzyAlgorithm: run the match algorithm on FuzzyText.
//! //        |
//! //        ↓
//! //    Apply BonusMatcher
//! //        |
//! //        |
//! //        |
//! //        ↓
//! //   MatchResult
//!

mod algo;
mod bonus;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// Re-export types
pub use self::algo::{fzy, skim, substring, FuzzyAlgorithm};
pub use self::bonus::cwd::Cwd;
pub use self::bonus::language::Language;
pub use self::bonus::Bonus;
use crate::substring::substr_indices;
use types::{CaseMatching, MatchedItem};
pub use types::{
    ClapItem, ExactTerm, ExactTermType, FuzzyTerm, FuzzyTermType, FuzzyText, InverseTerm,
    MatchResult, MatchScope, Query, Score, SearchTerm, SourceItem, TermType,
};

#[derive(Debug, Clone, Default)]
pub struct InverseMatcher {
    inverse_terms: Vec<InverseTerm>,
}

impl InverseMatcher {
    pub fn new(inverse_terms: Vec<InverseTerm>) -> Self {
        Self { inverse_terms }
    }

    pub fn inverse_terms(&self) -> &[InverseTerm] {
        &self.inverse_terms
    }

    /// Returns `true` if any inverse matching is satisfied, which means the item should be
    /// ignored.
    pub fn match_any(&self, match_text: &str) -> bool {
        self.inverse_terms
            .iter()
            .any(|inverse_term| inverse_term.is_match(match_text))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExactMatcher {
    exact_terms: Vec<ExactTerm>,
    case_matching: CaseMatching,
}

impl ExactMatcher {
    pub fn new(exact_terms: Vec<ExactTerm>, case_matching: CaseMatching) -> Self {
        Self {
            exact_terms,
            case_matching,
        }
    }

    pub fn exact_terms(&self) -> &[ExactTerm] {
        &self.exact_terms
    }

    /// Returns an optional tuple of (score, indices) if all the exact searching terms are satisfied.
    pub fn find_matches(&self, full_search_line: &str) -> Option<(Score, Vec<usize>)> {
        let mut indices = Vec::<usize>::new();
        let mut exact_score = Score::default();

        for term in &self.exact_terms {
            let sub_query = &term.word;

            match term.ty {
                ExactTermType::Exact => {
                    if let Some((score, sub_indices)) =
                        substr_indices(full_search_line, sub_query, self.case_matching)
                    {
                        indices.extend_from_slice(&sub_indices);
                        exact_score += score.max(sub_query.len() as Score);
                    } else {
                        return None;
                    }
                }
                ExactTermType::PrefixExact => {
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
                ExactTermType::SuffixExact => {
                    let total_len = full_search_line.len();
                    let trimmed = full_search_line.trim_end();
                    let white_space_len = total_len.saturating_sub(trimmed.len());
                    if trimmed.ends_with(sub_query) {
                        // In case of underflow, we use i32 here.
                        let mut match_start = total_len as i32
                            - sub_query.len() as i32
                            - 1i32
                            - white_space_len as i32;
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
}

#[derive(Debug, Clone, Default)]
pub struct FuzzyMatcher {
    match_scope: MatchScope,
    fuzzy_algo: FuzzyAlgorithm,
    fuzzy_terms: Vec<FuzzyTerm>,
    case_matching: CaseMatching,
}

impl FuzzyMatcher {
    pub fn new(
        fuzzy_terms: Vec<FuzzyTerm>,
        case_matching: CaseMatching,
        fuzzy_algo: FuzzyAlgorithm,
        match_scope: MatchScope,
    ) -> Self {
        Self {
            fuzzy_terms,
            case_matching,
            fuzzy_algo,
            match_scope,
        }
    }

    pub fn find_matches(&self, item: &Arc<dyn ClapItem>) -> Option<(Score, Vec<usize>)> {
        let fuzzy_len = self.fuzzy_terms.iter().map(|f| f.len()).sum();

        // Try the fuzzy terms against the matched text.
        let mut fuzzy_indices = Vec::with_capacity(fuzzy_len);
        let mut fuzzy_score = Score::default();

        if let Some(ref fuzzy_text) = item.fuzzy_text(self.match_scope) {
            for term in self.fuzzy_terms.iter() {
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

        Some((fuzzy_score, fuzzy_indices))
    }
}

#[derive(Debug, Clone, Default)]
pub struct BonusMatcher {
    bonuses: Vec<Bonus>,
}

impl BonusMatcher {
    pub fn new(bonuses: Vec<Bonus>) -> Self {
        Self { bonuses }
    }

    /// Returns the sum of bonus score.
    pub fn calc_bonus(
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
}

#[derive(Debug, Clone, Default)]
pub struct MatcherBuilder {
    bonuses: Vec<Bonus>,
    fuzzy_algo: FuzzyAlgorithm,
    match_scope: MatchScope,
    case_matching: CaseMatching,
}

impl MatcherBuilder {
    /// Create a new matcher builder with a default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bonuses(mut self, bonuses: Vec<Bonus>) -> Self {
        self.bonuses = bonuses;
        self
    }

    pub fn fuzzy_algo(mut self, algo: FuzzyAlgorithm) -> Self {
        self.fuzzy_algo = algo;
        self
    }

    pub fn match_scope(mut self, match_scope: MatchScope) -> Self {
        self.match_scope = match_scope;
        self
    }

    pub fn case_matching(mut self, case_matching: CaseMatching) -> Self {
        self.case_matching = case_matching;
        self
    }

    pub fn build(self, query: Query) -> Matcher {
        let Self {
            bonuses,
            fuzzy_algo,
            match_scope,
            case_matching,
        } = self;

        let Query {
            inverse_terms,
            exact_terms,
            fuzzy_terms,
        } = query;

        let inverse_matcher = InverseMatcher::new(inverse_terms);
        let exact_matcher = ExactMatcher::new(exact_terms, case_matching);
        let fuzzy_matcher = FuzzyMatcher::new(fuzzy_terms, case_matching, fuzzy_algo, match_scope);
        let bonus_matcher = BonusMatcher::new(bonuses);

        Matcher {
            inverse_matcher,
            exact_matcher,
            fuzzy_matcher,
            bonus_matcher,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Matcher {
    inverse_matcher: InverseMatcher,
    exact_matcher: ExactMatcher,
    fuzzy_matcher: FuzzyMatcher,
    bonus_matcher: BonusMatcher,
}

impl Matcher {
    // TODO: refactor this.
    pub fn match_scope(&self) -> MatchScope {
        self.fuzzy_matcher.match_scope
    }

    /// Actually performs the matching algorithm.
    pub fn match_item(&self, item: Arc<dyn ClapItem>) -> Option<MatchedItem> {
        let match_text = item.match_text();

        if match_text.is_empty() {
            return None;
        }

        // Try the inverse terms against the full search line.
        if self.inverse_matcher.match_any(match_text) {
            return None;
        }

        let (exact_score, mut indices) = self.exact_matcher.find_matches(match_text)?;

        let (fuzzy_score, mut fuzzy_indices) = self.fuzzy_matcher.find_matches(&item)?;

        // Merge the results from multi matchers.
        let match_result = if fuzzy_indices.is_empty() {
            let bonus_score = self.bonus_matcher.calc_bonus(&item, exact_score, &indices);

            indices.sort_unstable();
            indices.dedup();

            MatchResult::new(exact_score + bonus_score, indices)
        } else {
            fuzzy_indices.sort_unstable();
            fuzzy_indices.dedup();

            let bonus_score = self
                .bonus_matcher
                .calc_bonus(&item, fuzzy_score, &fuzzy_indices);

            indices.extend_from_slice(fuzzy_indices.as_slice());
            indices.sort_unstable();
            indices.dedup();

            MatchResult::new(exact_score + bonus_score + fuzzy_score, indices)
        };

        let MatchResult { score, indices } = item.match_result_callback(match_result);

        Some(MatchedItem::new(item, score, indices))
    }
}

#[derive(Debug, Default)]
pub struct InverseMatcherWithRecord {
    processed: AtomicU64,
    inverse_matcher: InverseMatcher,
}

impl InverseMatcherWithRecord {
    pub fn processed(self) -> u64 {
        self.processed.into_inner()
    }
}

impl grep_matcher::Matcher for InverseMatcherWithRecord {
    type Captures = grep_matcher::NoCaptures;
    type Error = String;

    fn find_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<Option<grep_matcher::Match>, Self::Error> {
        self.processed.fetch_add(1, Ordering::SeqCst);

        let line = std::str::from_utf8(haystack).map_err(|e| format!("{e}"))?;
        if self.inverse_matcher.match_any(line) {
            return Ok(None);
        }

        // Signal there is a match and should be processed in the sink later.
        Ok(Some(grep_matcher::Match::zero(at)))
    }

    fn new_captures(&self) -> Result<Self::Captures, Self::Error> {
        Ok(grep_matcher::NoCaptures::new())
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

        let item = SourceItem::from(line.to_string());
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

        let item = SourceItem::from(line.to_string());
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
        let query = "fil";
        let matcher = MatcherBuilder::new()
            .bonuses(vec![Bonus::FileName])
            .build(query.into());
        for line in lines {
            let item: Arc<dyn ClapItem> = Arc::new(SourceItem::from(line.to_string()));
            let fuzzy_text = item.fuzzy_text(matcher.match_scope()).unwrap();
            let match_result_base = matcher
                .fuzzy_matcher
                .fuzzy_algo
                .fuzzy_match(query, &fuzzy_text, matcher.fuzzy_matcher.case_matching)
                .unwrap();
            let match_result_with_bonus = matcher.match_item(item).unwrap();
            assert!(match_result_base.indices == match_result_with_bonus.indices);
            assert!(match_result_with_bonus.score > match_result_base.score);
        }
    }

    #[test]
    fn test_language_keyword_bonus() {
        let lines = vec!["hellorsr foo", "function foo"];
        let query: Query = "fo".into();
        let matcher = MatcherBuilder::new()
            .bonuses(vec![Bonus::Language("vim".into())])
            .build(query);
        let matched_item1 = matcher
            .match_item(Arc::new(lines[0]) as Arc<dyn ClapItem>)
            .unwrap();
        let matched_item2 = matcher
            .match_item(Arc::new(lines[1]) as Arc<dyn ClapItem>)
            .unwrap();
        assert!(matched_item1.indices == matched_item2.indices);
        assert!(matched_item1.score < matched_item2.score);
    }

    #[test]
    fn test_exact_search_term_bonus() {
        let lines = vec!["function foo qwer", "function foo"];
        let query: Query = "'fo".into();
        let matcher = MatcherBuilder::new().build(query);
        let matched_item1 = matcher
            .match_item(Arc::new(lines[0]) as Arc<dyn ClapItem>)
            .unwrap();
        let matched_item2 = matcher
            .match_item(Arc::new(lines[1]) as Arc<dyn ClapItem>)
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

        let match_with_query = |query: Query| {
            let matcher = MatcherBuilder::new()
                .bonuses(vec![Bonus::FileName])
                .build(query);
            items
                .clone()
                .into_iter()
                .map(|item| {
                    let item: Arc<dyn ClapItem> = item;
                    matcher.match_item(item)
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
        let match_results: Vec<_> = match_with_query(query);
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
        let match_results: Vec<_> = match_with_query(query);
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
        let match_results: Vec<_> = match_with_query(query);
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
        let match_results: Vec<_> = match_with_query(query);
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
