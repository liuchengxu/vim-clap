use crate::{MatchResult, Score};
use norm::fzf::{FzfParser, FzfV2};
use norm::Metric;

pub fn fuzzy_indices_v2(text: &str, query: &str) -> Option<MatchResult> {
    let mut fzf = FzfV2::new();
    let mut parser = FzfParser::new();
    let query = parser.parse(query);

    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    fzf.distance_and_ranges(query, text, &mut ranges)
        .map(|distance| {
            // norm use i64 as Score, but we use i32.
            MatchResult::new(
                distance.into_score() as Score,
                ranges.into_iter().flatten().collect(),
            )
        })
}
