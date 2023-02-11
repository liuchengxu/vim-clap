use crate::algo::substring::substr_indices;
use types::{CaseMatching, ExactTerm, ExactTermType, Score};

#[derive(Debug, Clone, Default)]
pub struct ExactMatcher {
    pub exact_terms: Vec<ExactTerm>,
    pub case_matching: CaseMatching,
}

impl ExactMatcher {
    pub fn new(exact_terms: Vec<ExactTerm>, case_matching: CaseMatching) -> Self {
        Self {
            exact_terms,
            case_matching,
        }
    }

    /// Returns an optional tuple of (score, indices) if all the exact searching terms are satisfied.
    pub fn find_matches(&self, full_search_line: &str) -> Option<(Score, Vec<usize>)> {
        let mut indices = Vec::<usize>::new();
        let mut exact_score = Score::default();

        if full_search_line.is_empty() {
            return None;
        }

        for term in &self.exact_terms {
            let sub_query = &term.text;

            match term.ty {
                ExactTermType::Exact => {
                    if let Some((score, sub_indices)) =
                        substr_indices(full_search_line, sub_query, self.case_matching)
                    {
                        indices.extend_from_slice(&sub_indices);
                        exact_score += score.max(sub_query.len() as Score);
                    } else {
                        return None;
                    }
                }
                ExactTermType::PrefixExact => {
                    let trimmed = full_search_line.trim_start();
                    let white_space_len = full_search_line.len().saturating_sub(trimmed.len());
                    if trimmed.starts_with(sub_query) {
                        let mut match_start = -1i32 + white_space_len as i32;
                        let new_len = indices.len() + sub_query.len();
                        indices.resize_with(new_len, || {
                            match_start += 1;
                            match_start as usize
                        });
                        exact_score += sub_query.len() as Score;
                    } else {
                        return None;
                    }
                }
                ExactTermType::SuffixExact => {
                    let total_len = full_search_line.len();
                    let trimmed = full_search_line.trim_end();
                    let white_space_len = total_len.saturating_sub(trimmed.len());
                    if trimmed.ends_with(sub_query) {
                        // In case of underflow, we use i32 here.
                        let mut match_start = total_len as i32
                            - sub_query.len() as i32
                            - 1i32
                            - white_space_len as i32;
                        let new_len = indices.len() + sub_query.len();
                        indices.resize_with(new_len, || {
                            match_start += 1;
                            match_start as usize
                        });
                        exact_score += sub_query.len() as Score;
                    } else {
                        return None;
                    }
                }
            }
        }

        // Add an exact search term bonus whether the exact matches exist or not.
        //
        // The shorter search line has a higher score.
        exact_score += (512 / full_search_line.len()) as Score;

        Some((exact_score, indices))
    }
}
