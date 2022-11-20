use crate::MatchResult;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use types::{CaseMatching, Score};

// TODO: do not have to create an instance of SkimMatcherV2 each time.
#[inline]
pub fn fuzzy_indices(text: &str, query: &str, case_matching: CaseMatching) -> Option<MatchResult> {
    let skim_matcher = SkimMatcherV2::default();
    let skim_matcher = match case_matching {
        CaseMatching::Ignore => skim_matcher.ignore_case(),
        CaseMatching::Respect => skim_matcher.respect_case(),
        CaseMatching::Smart => skim_matcher.smart_case(),
    };
    // skim uses i64 as Score, but we use i32.
    skim_matcher
        .fuzzy_indices(text, query)
        .map(|(score, indices)| MatchResult::new(score as Score, indices))
}
