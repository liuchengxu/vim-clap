// Re-export the fzy algorithm
pub use extracted_fzy::*;

use crate::{MatchResult, Score};

/// Make the arguments order same to Skim's `fuzzy_indices()`.
#[inline]
pub fn fuzzy_indices(line: &str, query: &str) -> MatchResult {
    match_and_score_with_positions(query, line).map(|(score, indices)| (score as Score, indices))
}
