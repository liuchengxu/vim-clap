pub mod fzy;
pub mod skim;
pub mod substring;

use crate::MatchResult;
use types::{CaseMatching, FuzzyText};

#[derive(Debug, Clone, Copy, Default)]
pub enum FuzzyAlgorithm {
    Skim,
    #[default]
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

impl FuzzyAlgorithm {
    pub fn fuzzy_match(
        &self,
        query: &str,
        fuzzy_text: &FuzzyText,
        case_matching: CaseMatching,
    ) -> Option<MatchResult> {
        let FuzzyText {
            text,
            matching_start,
        } = fuzzy_text;

        let fuzzy_result = match self {
            Self::Fzy => fzy::fuzzy_indices(text, query, case_matching),
            Self::Skim => skim::fuzzy_indices(text, query, case_matching),
        };
        fuzzy_result.map(|MatchResult { score, indices }| {
            let mut indices = indices;
            indices.iter_mut().for_each(|x| *x += matching_start);
            MatchResult::new(score, indices)
        })
    }
}
