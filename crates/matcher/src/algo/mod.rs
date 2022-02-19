pub mod fzy;
pub mod skim;
pub mod substring;

use types::{MatchingText, MatchingTextKind};

use crate::MatchResult;

/// Supported fuzzy match algorithm.
#[derive(Debug, Clone, Copy)]
pub enum FuzzyAlgorithm {
    Skim,
    Fzy,
}

impl std::str::FromStr for FuzzyAlgorithm {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl<T: AsRef<str>> From<T> for FuzzyAlgorithm {
    fn from(algo: T) -> Self {
        match algo.as_ref().to_lowercase().as_str() {
            "skim" => Self::Skim,
            "fzy" => Self::Fzy,
            _ => Self::Fzy,
        }
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
        matching_text_kind: &MatchingTextKind,
    ) -> MatchResult {
        item.fuzzy_text(matching_text_kind)
            .and_then(|(text, offset)| {
                let res = match self {
                    Self::Fzy => fzy::fuzzy_indices(text, query),
                    Self::Skim => skim::fuzzy_indices(text, query),
                };
                res.map(|(score, mut indices)| {
                    indices.iter_mut().for_each(|x| *x += offset);
                    (score, indices)
                })
            })
    }
}
