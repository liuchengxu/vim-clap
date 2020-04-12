mod source;

use anyhow::Result;
use rayon::prelude::*;
use structopt::clap::arg_enum;

pub use source::Source;
#[cfg(feature = "enable_dyn")]
pub use subprocess;

// Implement arg_enum for using it in the command line arguments.
arg_enum! {
  /// Supported fuzzy match algorithm.
  #[derive(Debug)]
  pub enum Algo {
      Skim,
      Fzy,
  }
}

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
