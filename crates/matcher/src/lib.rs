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
#[cfg(test)]
mod tests;

use std::path::Path;
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

        // Add an exact search term bonus whether the exact matches exist or not.
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
        item.fuzzy_text(self.match_scope)
            .as_ref()
            .and_then(|fuzzy_text| self.match_fuzzy_text(fuzzy_text))
    }

    pub fn match_fuzzy_text(&self, fuzzy_text: &FuzzyText) -> Option<(Score, Vec<usize>)> {
        let fuzzy_len = self.fuzzy_terms.iter().map(|f| f.len()).sum();

        // Try the fuzzy terms against the matched text.
        let mut fuzzy_indices = Vec::with_capacity(fuzzy_len);
        let mut fuzzy_score = Score::default();

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
    pub fn calc_item_bonus(
        &self,
        item: &Arc<dyn ClapItem>,
        base_score: Score,
        base_indices: &[usize],
    ) -> Score {
        self.bonuses
            .iter()
            .map(|b| b.item_bonus_score(item, base_score, base_indices))
            .sum()
    }

    /// Returns the sum of bonus score.
    pub fn calc_text_bonus(
        &self,
        bonus_text: &str,
        base_score: Score,
        base_indices: &[usize],
    ) -> Score {
        self.bonuses
            .iter()
            .map(|b| b.text_bonus_score(bonus_text, base_score, base_indices))
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
            let bonus_score = self
                .bonus_matcher
                .calc_item_bonus(&item, exact_score, &indices);

            indices.sort_unstable();
            indices.dedup();

            MatchResult::new(exact_score + bonus_score, indices)
        } else {
            fuzzy_indices.sort_unstable();
            fuzzy_indices.dedup();

            let bonus_score =
                self.bonus_matcher
                    .calc_item_bonus(&item, fuzzy_score, &fuzzy_indices);

            indices.extend_from_slice(fuzzy_indices.as_slice());
            indices.sort_unstable();
            indices.dedup();

            MatchResult::new(exact_score + bonus_score + fuzzy_score, indices)
        };

        let MatchResult { score, indices } = item.match_result_callback(match_result);

        Some(MatchedItem::new(item, score, indices))
    }

    /// Actually performs the matching algorithm.
    pub fn match_file_result(&self, path: &Path, line: &str) -> Option<MatchedFileResult> {
        if line.is_empty() {
            return None;
        }

        let path = path.to_str()?;

        // Try the inverse terms against the full search line.
        if self.inverse_matcher.match_any(line) || self.inverse_matcher.match_any(path) {
            return None;
        }

        let ((exact_score, exact_indices), exact_indices_in_path) =
            match self.exact_matcher.find_matches(path) {
                Some((score, indices)) => ((score, indices), true),
                None => (self.exact_matcher.find_matches(line)?, false),
            };

        let fuzzy_text = FuzzyText::new(line, 0);
        let (fuzzy_score, mut fuzzy_indices) = self.fuzzy_matcher.match_fuzzy_text(&fuzzy_text)?;

        // Merge the results from multi matchers.
        let matched_file_result = if fuzzy_indices.is_empty() {
            let bonus_score = self
                .bonus_matcher
                .calc_text_bonus(line, exact_score, &exact_indices);

            let mut exact_indices = exact_indices;
            exact_indices.sort_unstable();
            exact_indices.dedup();

            if exact_indices_in_path {
                MatchedFileResult {
                    score: exact_score + bonus_score,
                    exact_indices,
                    fuzzy_indices: Vec::new(),
                }
            } else {
                MatchedFileResult {
                    score: exact_score + bonus_score,
                    exact_indices: Vec::new(),
                    fuzzy_indices: exact_indices,
                }
            }
        } else {
            fuzzy_indices.sort_unstable();
            fuzzy_indices.dedup();

            let bonus_score = self
                .bonus_matcher
                .calc_text_bonus(line, fuzzy_score, &fuzzy_indices);

            let score = exact_score + bonus_score + fuzzy_score;

            if exact_indices_in_path {
                MatchedFileResult {
                    exact_indices,
                    fuzzy_indices,
                    score,
                }
            } else {
                let mut indices = exact_indices;
                indices.extend_from_slice(fuzzy_indices.as_slice());
                indices.sort_unstable();
                indices.dedup();

                MatchedFileResult {
                    exact_indices: Vec::new(),
                    fuzzy_indices: indices,
                    score,
                }
            }
        };

        Some(matched_file_result)
    }
}

#[derive(Debug)]
pub struct MatchedFileResult {
    pub exact_indices: Vec<usize>,
    pub fuzzy_indices: Vec<usize>,
    pub score: Score,
}
