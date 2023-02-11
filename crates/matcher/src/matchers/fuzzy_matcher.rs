use crate::algo::FuzzyAlgorithm;
use std::sync::Arc;
use types::{CaseMatching, ClapItem, FuzzyTerm, FuzzyText, MatchResult, MatchScope, Score};

#[derive(Debug, Clone, Default)]
pub struct FuzzyMatcher {
    pub match_scope: MatchScope,
    pub fuzzy_algo: FuzzyAlgorithm,
    pub fuzzy_terms: Vec<FuzzyTerm>,
    pub case_matching: CaseMatching,
}

impl FuzzyMatcher {
    pub fn new(
        match_scope: MatchScope,
        fuzzy_algo: FuzzyAlgorithm,
        fuzzy_terms: Vec<FuzzyTerm>,
        case_matching: CaseMatching,
    ) -> Self {
        Self {
            match_scope,
            fuzzy_algo,
            fuzzy_terms,
            case_matching,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.fuzzy_terms.is_empty()
    }

    pub fn find_matches(&self, item: &Arc<dyn ClapItem>) -> Option<(Score, Vec<usize>)> {
        item.fuzzy_text(self.match_scope)
            .as_ref()
            .and_then(|fuzzy_text| self.match_fuzzy_text(fuzzy_text))
    }

    pub fn match_fuzzy_text(&self, fuzzy_text: &FuzzyText) -> Option<(Score, Vec<usize>)> {
        let fuzzy_len = self.fuzzy_terms.iter().map(|f| f.len()).sum();

        // Try the fuzzy terms against the matched text.
        let mut fuzzy_indices = Vec::with_capacity(fuzzy_len);
        let mut fuzzy_score = Score::default();

        for term in self.fuzzy_terms.iter() {
            let query = &term.text;
            if let Some(MatchResult { score, indices }) =
                self.fuzzy_algo
                    .fuzzy_match(query, fuzzy_text, self.case_matching)
            {
                fuzzy_score += score;
                fuzzy_indices.extend(indices);
            } else {
                return None;
            }
        }

        Some((fuzzy_score, fuzzy_indices))
    }
}
