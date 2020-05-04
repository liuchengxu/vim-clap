//! Working with ASCII-only strings.
//!
//! Cheating!

mod bytelines;
pub use bytelines::ByteLines;
mod matcher;
pub use matcher::{ascii_from_bytes, matcher};

use crate::{fzy_algo::score_with_positions, scoring_utils::MatchWithPositions};

#[inline]
pub fn match_and_score_with_positions(
    needle: &[u8],
    haystack: &[u8],
) -> Option<MatchWithPositions> {
    matcher(haystack, needle).map(|_| score_with_positions(needle, needle.len(), haystack))
}
