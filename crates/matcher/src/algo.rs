use structopt::clap::arg_enum;

use source_item::{MatchTextFor, MatchType};

use crate::MatchResult;

// Implement arg_enum for using it in the command line arguments.
arg_enum! {
  /// Supported line oriented String match algorithm.
  #[derive(Debug, Clone)]
  pub enum Algo {
      Skim,
      Fzy,
      SubString,
  }
}

impl Algo {
    pub fn apply_match<'a, T: MatchTextFor<'a>>(
        &self,
        query: &str,
        item: &T,
        match_type: &MatchType,
    ) -> MatchResult {
        item.match_text_for(match_type).and_then(|(text, offset)| {
            let res = match self {
                Self::Fzy => fzy::fuzzy_indices(text, query),
                Self::Skim => skim::fuzzy_indices(text, query),
                Self::SubString => substring::substr_indices(text, query),
            };
            res.map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
        })
    }
}

pub mod skim {
    use crate::MatchResult;
    use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

    pub fn fuzzy_indices(text: &str, query: &str) -> MatchResult {
        SkimMatcherV2::default().fuzzy_indices(text, query)
    }
}

pub mod fzy {
    // Reexport the fzy algorithm
    pub use extracted_fzy::*;

    /// Make the arguments order same to Skim's `fuzzy_indices()`.
    #[inline]
    pub fn fuzzy_indices(line: &str, query: &str) -> crate::MatchResult {
        match_and_score_with_positions(query, line).map(|(score, indices)| (score as i64, indices))
    }
}

pub mod substring {
    fn find_start_at(slice: &str, start_at: usize, pat: &str) -> Option<usize> {
        slice[start_at..].find(pat).map(|i| start_at + i)
    }

    fn _substr_indices_impl(haystack: &str, niddle: &str) -> Option<(f64, Vec<usize>)> {
        let niddle = niddle.to_lowercase();

        match find_start_at(haystack, 0, &niddle) {
            Some(idx) => {
                let mut positions = Vec::new();

                // For build without overflow checks this could be written as
                // `let mut pos = idx - 1;` with `|| { pos += 1; pos }` closure.
                let mut pos = idx;
                positions.resize_with(
                    niddle.len(),
                    // Simple endless iterator for `idx..` range. Even though it's endless,
                    // it will iterate only `sub_niddle.len()` times.
                    || {
                        pos += 1;
                        pos - 1
                    },
                );

                if positions.is_empty() {
                    return None;
                }

                let calc_score = || {
                    let last_pos = positions.last().unwrap();
                    let match_len = (last_pos + 1 - positions[0]) as f64;

                    (2f64 / (positions[0] + 1) as f64) + 1f64 / (last_pos + 1) as f64 - match_len
                };

                Some((calc_score(), positions))
            }
            None => None,
        }
    }

    fn unordered_substr_indices_impl(haystack: &str, niddle: &str) -> Option<(f64, Vec<usize>)> {
        // unreasonably large haystack
        if haystack.len() > 1024 {
            return None;
        }

        let haystack = haystack.to_lowercase();
        let haystack = haystack.as_str();

        let mut total_score = 0f64;
        let mut positions = Vec::new();
        for sub_niddle in niddle.split_whitespace() {
            if let Some((score, indices)) = _substr_indices_impl(haystack, &sub_niddle) {
                total_score += score;
                positions.extend_from_slice(&indices);
            } else {
                return None;
            }
        }

        if positions.is_empty() {
            return Some((0f64, positions));
        }

        positions.sort_unstable();

        Some((total_score, positions))
    }

    pub fn substr_indices(haystack: &str, niddle: &str) -> Option<(i64, Vec<usize>)> {
        unordered_substr_indices_impl(haystack, niddle)
            .map(|(score, positions)| (score as i64, positions))
    }

    #[test]
    fn test_substr() {
        assert_eq!(
            substr_indices("src/bun/blune", "sr bl"),
            Some((-1, vec![0, 1, 8, 9]))
        );

        assert_eq!(
            substr_indices("src/bun/blune", "bl sr"),
            Some((-1, vec![0, 1, 8, 9]))
        );
    }
}
