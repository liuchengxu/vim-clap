use crate::search_term::{ExactTerm, FuzzyTerm, InverseTerm, SearchTerm, TermType, WordTerm};

/// [`Query`] represents the structural search info parsed from the initial user input.
#[derive(Debug, Clone)]
pub struct Query {
    pub word_terms: Vec<WordTerm>,
    pub exact_terms: Vec<ExactTerm>,
    pub fuzzy_terms: Vec<FuzzyTerm>,
    pub inverse_terms: Vec<InverseTerm>,
}

impl<T: AsRef<str>> From<T> for Query {
    fn from(query: T) -> Self {
        let query = query.as_ref();

        let mut word_terms = Vec::new();
        let mut exact_terms = Vec::new();
        let mut fuzzy_terms = Vec::new();
        let mut inverse_terms = Vec::new();

        for token in query.split_whitespace() {
            let SearchTerm { ty, text } = token.into();

            match ty {
                TermType::Word => word_terms.push(WordTerm { text }),
                TermType::Exact(term_ty) => exact_terms.push(ExactTerm::new(term_ty, text)),
                TermType::Fuzzy(term_ty) => fuzzy_terms.push(FuzzyTerm::new(term_ty, text)),
                TermType::Inverse(term_ty) => inverse_terms.push(InverseTerm::new(term_ty, text)),
            }
        }

        Self {
            word_terms,
            exact_terms,
            fuzzy_terms,
            inverse_terms,
        }
    }
}

impl Query {
    pub fn fuzzy_len(&self) -> usize {
        self.fuzzy_terms.iter().map(|f| f.len()).sum()
    }
}
