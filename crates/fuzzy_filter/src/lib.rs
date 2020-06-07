//! This crate provides various filter algorithms for linewise filtering.
//!
//! There two steps to filter a line:
//!
//!     raw_line
//!        |
//!        |  LineSplitter: split out the content to be filtered.
//!        |
//!       \|/
//!   content to filter
//!        |
//!        |  Algo: apply the filter algo.
//!        |
//!       \|/
//!   ScorerOutput
//!

mod line_splitter;
mod source;
mod substr;

use anyhow::Result;
use line_splitter::*;
use rayon::prelude::*;
use structopt::clap::arg_enum;

pub use extracted_fzy as fzy;
pub use fuzzy_matcher::skim::fuzzy_indices as fuzzy_indices_skim;
pub use line_splitter::fuzzy_indices_fzy;
pub use source::Source;
#[cfg(feature = "enable_dyn")]
pub use subprocess;
pub use substr::substr_indices;

// Implement arg_enum so that we could control it from the command line.
arg_enum! {
  /// Sometimes we hope to filter on the part of line.
  #[derive(Debug, Clone)]
  pub enum LineSplitter {
      Full,
      TagNameOnly,
      FileNameOnly,
      GrepExcludeFilePath,
  }
}

impl From<&str> for LineSplitter {
    fn from(filtering: &str) -> Self {
        match filtering {
            "Full" => Self::Full,
            "TagNameOnly" => Self::TagNameOnly,
            "FileNameOnly" => Self::FileNameOnly,
            "GrepExcludeFilePath" => Self::GrepExcludeFilePath,
            _ => Self::Full,
        }
    }
}

impl From<String> for LineSplitter {
    fn from(filtering: String) -> Self {
        Self::from(filtering.as_str())
    }
}

// Implement arg_enum for using it in the command line arguments.
arg_enum! {
  /// Supported fuzzy match algorithm.
  #[derive(Debug, Clone)]
  pub enum Algo {
      Skim,
      Fzy,
      SubString,
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

/// Returns the appropriate scorer given the algo and line_splitter strategy.
#[inline]
pub fn get_appropriate_scorer(
    algo: &Algo,
    line_splitter: &LineSplitter,
) -> impl Fn(&str, &str) -> ScorerOutput {
    match algo {
        Algo::Skim => match line_splitter {
            LineSplitter::Full => fuzzy_indices_skim,
            LineSplitter::TagNameOnly => apply_on_tag_line_skim,
            LineSplitter::FileNameOnly => apply_on_file_line_skim,
            LineSplitter::GrepExcludeFilePath => apply_on_grep_line_skim,
        },
        Algo::Fzy => match line_splitter {
            LineSplitter::Full => fuzzy_indices_fzy,
            LineSplitter::TagNameOnly => apply_on_tag_line_fzy,
            LineSplitter::FileNameOnly => apply_on_file_line_fzy,
            LineSplitter::GrepExcludeFilePath => apply_on_grep_line_fzy,
        },
        Algo::SubString => match line_splitter {
            LineSplitter::Full => substr_indices,
            LineSplitter::TagNameOnly => apply_on_tag_line_substr,
            LineSplitter::FileNameOnly => apply_on_file_line_substr,
            LineSplitter::GrepExcludeFilePath => apply_on_grep_line_substr,
        },
    }
}
