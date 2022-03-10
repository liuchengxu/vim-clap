use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::MatchResult;

#[inline]
pub fn fuzzy_indices(text: &str, query: &str) -> Option<MatchResult> {
    SkimMatcherV2::default()
        .fuzzy_indices(text, query)
        .map(|(score, indices)| MatchResult::new(score, indices))
}
