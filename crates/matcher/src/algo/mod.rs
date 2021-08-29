pub mod fzy;
pub mod skim;
pub mod substring;

use structopt::clap::arg_enum;

use types::{MatchType, MatchingText};

use crate::MatchResult;

// Implement arg_enum for using it in the command line arguments.
arg_enum! {
  /// Supported fuzzy match algorithm.
  #[derive(Debug, Clone)]
  pub enum FuzzyAlgorithm {
      Skim,
      Fzy,
  }
}

impl Default for FuzzyAlgorithm {
    fn default() -> Self {
        Self::Fzy
    }
}

impl FuzzyAlgorithm {
    /// Does the fuzzy match against the match text.
    pub fn fuzzy_match<'a, T: MatchingText<'a>>(
        &self,
        query: &str,
        item: &T,
        match_type: &MatchType,
    ) -> MatchResult {
        item.fuzzy_text(match_type).and_then(|(text, offset)| {
            let res = match self {
                Self::Fzy => fzy::fuzzy_indices(text, query),
                Self::Skim => skim::fuzzy_indices(text, query),
            };
            res.map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
        })
    }
}
