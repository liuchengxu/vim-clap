mod content_filtering;
mod source;

use anyhow::Result;
use content_filtering::*;
use rayon::prelude::*;
use structopt::clap::arg_enum;

pub use content_filtering::fuzzy_indices_fzy;
pub use fuzzy_matcher::skim::fuzzy_indices as fuzzy_indices_skim;
pub use source::Source;
#[cfg(feature = "enable_dyn")]
pub use subprocess;

// Implement arg_enum so that we could control it from the command line.
arg_enum! {
  /// Sometimes we hope to filter on the part of line.
  #[derive(Debug, Clone)]
  pub enum ContentFiltering {
      Full,
      FileNameOnly,
      GrepExcludeFilePath,
  }
}

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

// Returns the score and indices of matched chars
// when the line is matched given the query,
pub type ScorerOutput = Option<(i64, Vec<usize>)>;

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

/// Returns the appropriate scorer given the algo and content_filtering strategy.
#[inline]
pub fn get_appropriate_scorer(
    algo: Algo,
    content_filtering: ContentFiltering,
) -> impl Fn(&str, &str) -> ScorerOutput {
    match algo {
        Algo::Skim => match content_filtering {
            ContentFiltering::Full => fuzzy_indices_skim,
            ContentFiltering::FileNameOnly => apply_skim_on_file_line,
            ContentFiltering::GrepExcludeFilePath => apply_skim_on_grep_line,
        },
        Algo::Fzy => match content_filtering {
            ContentFiltering::Full => fuzzy_indices_fzy,
            ContentFiltering::FileNameOnly => apply_fzy_on_file_line,
            ContentFiltering::GrepExcludeFilePath => apply_fzy_on_grep_line,
        },
    }
}
