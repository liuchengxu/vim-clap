use types::InverseTerm;

#[derive(Debug, Clone, Default)]
pub struct InverseMatcher {
    inverse_terms: Vec<InverseTerm>,
}

impl InverseMatcher {
    pub fn new(inverse_terms: Vec<InverseTerm>) -> Self {
        Self { inverse_terms }
    }

    pub fn inverse_terms(&self) -> &[InverseTerm] {
        &self.inverse_terms
    }

    /// Returns `true` if any inverse matching is satisfied, which means the item should be
    /// ignored.
    pub fn match_any(&self, match_text: &str) -> bool {
        self.inverse_terms
            .iter()
            .any(|inverse_term| inverse_term.exact_matched(match_text))
    }
}
