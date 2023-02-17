//! This crate provides various matcher algorithms for line oriented search given the query string.
//!
//! The matcher result consists of the score and the indices of matched items.
//!
//! Matching flow:
//!
//! //        arc<dyn ClapItem>
//! //               |
//! //               ↓
//! //    +----------------------+
//! //    |    InverseMatcher    |
//! //    +----------------------+
//! //               |
//! //               ↓
//! //    +----------------------+
//! //    |    WordMatcher       |
//! //    +----------------------+
//! //               |
//! //               ↓
//! //    +----------------------+
//! //    |    ExactMatcher      |
//! //    +----------------------+
//! //               |
//! //               ↓
//! //    +----------------------+
//! //    |    FuzzyMatcher      |
//! //    +----------------------+
//! //               |      MatchScope: extract the content to match.
//! //               |  FuzzyAlgorithm: run the match algorithm on FuzzyText.
//! //               ↓
//! //    +----------------------+
//! //    |    BonusMatcher      |
//! //    +----------------------+
//! //               |
//! //               ↓
//! //  MatchResult { score, indices }
//!

mod algo;
mod matchers;
#[cfg(test)]
mod tests;

pub use self::algo::{substring, FuzzyAlgorithm};
pub use self::matchers::{
    Bonus, BonusMatcher, ExactMatcher, FuzzyMatcher, InverseMatcher, WordMatcher,
};
use std::path::Path;
use std::sync::Arc;
use types::{CaseMatching, ClapItem, FuzzyText, MatchedItem, Rank, RankCalculator, RankCriterion};

// Re-export types
pub use types::{MatchResult, MatchScope, Query, Score};

#[derive(Debug, Clone, Default)]
pub struct MatcherBuilder {
    bonuses: Vec<Bonus>,
    fuzzy_algo: FuzzyAlgorithm,
    match_scope: MatchScope,
    case_matching: CaseMatching,
    rank_criteria: Vec<RankCriterion>,
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

    pub fn rank_criteria(mut self, sort_criteria: Vec<RankCriterion>) -> Self {
        self.rank_criteria = sort_criteria;
        self
    }

    pub fn build(self, query: Query) -> Matcher {
        let Self {
            bonuses,
            fuzzy_algo,
            match_scope,
            case_matching,
            rank_criteria,
        } = self;

        let Query {
            word_terms,
            fuzzy_terms,
            exact_terms,
            inverse_terms,
        } = query;

        let inverse_matcher = InverseMatcher::new(inverse_terms);
        let word_matcher = WordMatcher::new(word_terms);
        let exact_matcher = ExactMatcher::new(exact_terms, case_matching);
        let fuzzy_matcher = FuzzyMatcher::new(match_scope, fuzzy_algo, fuzzy_terms, case_matching);
        let bonus_matcher = BonusMatcher::new(bonuses);

        let rank_calculator = if rank_criteria.is_empty() {
            RankCalculator::default()
        } else {
            RankCalculator::new(rank_criteria)
        };

        Matcher {
            inverse_matcher,
            word_matcher,
            exact_matcher,
            fuzzy_matcher,
            bonus_matcher,
            rank_calculator,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Matcher {
    inverse_matcher: InverseMatcher,
    word_matcher: WordMatcher,
    exact_matcher: ExactMatcher,
    fuzzy_matcher: FuzzyMatcher,
    bonus_matcher: BonusMatcher,
    rank_calculator: RankCalculator,
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

        let (word_score, word_indices) = if !self.word_matcher.is_empty() {
            self.word_matcher.find_matches(match_text)?
        } else {
            (Score::default(), Vec::new())
        };

        let (exact_score, mut exact_indices) = self.exact_matcher.find_matches(match_text)?;
        let (fuzzy_score, mut fuzzy_indices) = self.fuzzy_matcher.find_matches(&item)?;

        // Merge the results from multi matchers.
        let mut match_result = if fuzzy_indices.is_empty() {
            exact_indices.sort_unstable();
            exact_indices.dedup();

            let bonus_score =
                self.bonus_matcher
                    .calc_item_bonus(&item, exact_score, &exact_indices);

            MatchResult::new(exact_score + bonus_score, exact_indices)
        } else {
            fuzzy_indices.sort_unstable();
            fuzzy_indices.dedup();

            let bonus_score =
                self.bonus_matcher
                    .calc_item_bonus(&item, fuzzy_score, &fuzzy_indices);

            let mut indices = exact_indices;

            indices.extend(fuzzy_indices);
            indices.sort_unstable();
            indices.dedup();

            MatchResult::new(exact_score + bonus_score + fuzzy_score, indices)
        };

        if !word_indices.is_empty() {
            match_result.add_score(word_score);
            match_result.extend_indices(word_indices);
        }

        let MatchResult { score, indices } = item.match_result_callback(match_result);

        let begin = indices.first().copied().unwrap_or(0);
        let end = indices.last().copied().unwrap_or(0);
        let length = item.raw_text().len();

        let rank = self
            .rank_calculator
            .calculate_rank(score, begin, end, length);

        Some(MatchedItem::new(item, rank, indices))
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

        let (word_score, word_indices) = if !self.word_matcher.is_empty() {
            self.word_matcher.find_matches(line)?
        } else {
            (Score::default(), Vec::new())
        };

        let ((exact_score, exact_indices), exact_indices_in_path) =
            match self.exact_matcher.find_matches(path) {
                Some((score, indices)) => ((score, indices), true),
                None => (self.exact_matcher.find_matches(line)?, false),
            };

        let fuzzy_text = FuzzyText::new(line, 0);
        let (mut fuzzy_score, mut fuzzy_indices) =
            self.fuzzy_matcher.match_fuzzy_text(&fuzzy_text)?;

        // Apply the word matcher against the line content.
        if !word_indices.is_empty() {
            fuzzy_score += word_score;
            fuzzy_indices.extend(word_indices)
        }

        // Merge the results from multi matchers.
        let (score, exact_indices, fuzzy_indices) = if fuzzy_indices.is_empty() {
            let bonus_score = self
                .bonus_matcher
                .calc_text_bonus(line, exact_score, &exact_indices);

            let mut exact_indices = exact_indices;
            exact_indices.sort_unstable();
            exact_indices.dedup();

            let score = exact_score + bonus_score;

            if exact_indices_in_path {
                (score, exact_indices, Vec::new())
            } else {
                (score, Vec::new(), exact_indices)
            }
        } else {
            fuzzy_indices.sort_unstable();
            fuzzy_indices.dedup();

            let bonus_score = self
                .bonus_matcher
                .calc_text_bonus(line, fuzzy_score, &fuzzy_indices);

            let score = exact_score + bonus_score + fuzzy_score;

            if exact_indices_in_path {
                (score, exact_indices, fuzzy_indices)
            } else {
                let mut indices = exact_indices;
                indices.extend_from_slice(fuzzy_indices.as_slice());
                indices.sort_unstable();
                indices.dedup();

                (score, Vec::new(), indices)
            }
        };

        let begin = exact_indices
            .first()
            .copied()
            .unwrap_or_else(|| fuzzy_indices.first().copied().unwrap_or(0));
        let end = fuzzy_indices
            .last()
            .copied()
            .unwrap_or_else(|| exact_indices.last().copied().unwrap_or(0));
        let length = line.len();

        let rank = self
            .rank_calculator
            .calculate_rank(score, begin, end, length);

        Some(MatchedFileResult {
            rank,
            exact_indices,
            fuzzy_indices,
        })
    }
}

#[derive(Debug)]
pub struct MatchedFileResult {
    pub rank: Rank,
    pub exact_indices: Vec<usize>,
    pub fuzzy_indices: Vec<usize>,
}
