use crate::search_term::{ExactTerm, FuzzyTerm, InverseTerm, SearchTerm, TermType};

#[derive(Debug, Clone)]
pub struct Query {
    pub fuzzy_terms: Vec<FuzzyTerm>,
    pub exact_terms: Vec<ExactTerm>,
    pub inverse_terms: Vec<InverseTerm>,
}

impl From<&str> for Query {
    fn from(query: &str) -> Self {
        let mut fuzzy_terms = Vec::new();
        let mut exact_terms = Vec::new();
        let mut inverse_terms = Vec::new();

        for token in query.split_whitespace() {
            let SearchTerm { ty, word } = token.into();

            match ty {
                TermType::Fuzzy(term_ty) => fuzzy_terms.push(FuzzyTerm::new(term_ty, word)),
                TermType::Exact(term_ty) => exact_terms.push(ExactTerm::new(term_ty, word)),
                TermType::Inverse(term_ty) => inverse_terms.push(InverseTerm::new(term_ty, word)),
            }
        }

        Self {
            fuzzy_terms,
            exact_terms,
            inverse_terms,
        }
    }
}

impl From<String> for Query {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}
