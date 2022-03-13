// Re-export the fzy algorithm
pub use extracted_fzy::{match_and_score_with_positions, MatchWithPositions};

use extracted_fzy::CaseMatching;

use crate::{MatchResult, Score};

pub struct FzyMatcher {
    case_sensitive: CaseMatching,
}

impl FzyMatcher {
    pub fn new(case_sensitive: CaseMatching) -> Self {
        Self { case_sensitive }
    }
}

/// Make the arguments order same to Skim's `fuzzy_indices()`.
pub fn fuzzy_indices(
    line: &str,
    query: &str,
    case_sensitive: &types::CaseMatching,
) -> Option<MatchResult> {
    let case_sensitive = match case_sensitive {
        types::CaseMatching::Ignore => CaseMatching::Ignore,
        types::CaseMatching::Respect => CaseMatching::Respect,
        types::CaseMatching::SmartCase => CaseMatching::SmartCase,
    };
    match_and_score_with_positions(query, line, case_sensitive)
        .map(|(score, indices)| MatchResult::new(score as Score, indices))
}
