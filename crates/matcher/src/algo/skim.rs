use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::MatchResult;

#[inline]
pub fn fuzzy_indices(text: &str, query: &str) -> MatchResult {
    SkimMatcherV2::default().fuzzy_indices(text, query)
}
