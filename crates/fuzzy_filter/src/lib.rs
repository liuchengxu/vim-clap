//! This crate provides the feature of filtering a stream of lines.
//!
//! Given a stream of lines, apply the matcher on each of them and then
//! sort the all lines with a match result, returns the top rated filtered lines.

mod source;

use anyhow::Result;
use matcher::Algo;
use rayon::prelude::*;

pub use source::Source;
#[cfg(feature = "enable_dyn")]
pub use subprocess;

/// Tuple of (matched line text, filtering score, indices of matched elements)
pub type FuzzyMatchedLineInfo = (String, i64, Vec<usize>);

/// Returns the ranked results after applying the fuzzy filter
/// given the query String and filtering source.
pub fn fuzzy_filter_and_rank<I: Iterator<Item = String>>(
    query: &str,
    source: Source<I>,
    algo: Algo,
) -> Result<Vec<FuzzyMatchedLineInfo>> {
    let mut ranked = source.fuzzy_filter(algo, query)?;

    ranked.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

    Ok(ranked)
}
