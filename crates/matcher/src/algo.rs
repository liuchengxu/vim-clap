use structopt::clap::arg_enum;

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

pub mod skim {
    // Reexport the skim algorithm, mainly `fuzzy_indices()`.
    pub use fuzzy_matcher::skim::*;
}

pub mod fzy {
    // Reexport the fzy algorithm
    pub use extracted_fzy::*;

    /// Make the arguments order same to Skim's `fuzzy_indices()`.
    #[inline]
    pub fn fuzzy_indices(line: &str, query: &str) -> crate::MatcherResult {
        match_and_score_with_positions(query, line).map(|(score, indices)| (score as i64, indices))
    }
}

pub mod substring {
    fn find_start_at(slice: &str, start_at: usize, pat: &str) -> Option<usize> {
        slice[start_at..].find(pat).map(|i| start_at + i)
    }

    fn substr_indices_impl(haystack: &str, niddle: &str) -> Option<(f64, Vec<usize>)> {
        // unreasonably large haystack
        if haystack.len() > 1024 {
            return None;
        }

        let haystack = haystack.to_lowercase();
        let haystack = haystack.as_str();

        let mut offset = 0;
        let mut positions = Vec::new();
        for sub_niddle in niddle.split_whitespace() {
            let sub_niddle = sub_niddle.to_lowercase();

            match find_start_at(haystack, offset, &sub_niddle) {
                Some(idx) => {
                    offset = idx + sub_niddle.len();
                    // For build without overflow checks this could be written as
                    // `let mut pos = idx - 1;` with `|| { pos += 1; pos }` closure.
                    let mut pos = idx;
                    positions.resize_with(
                        positions.len() + sub_niddle.len(),
                        // Simple endless iterator for `idx..` range. Even though it's endless,
                        // it will iterate only `sub_niddle.len()` times.
                        || {
                            pos += 1;
                            pos - 1
                        },
                    );
                }
                None => return None,
            }
        }

        if positions.is_empty() {
            return Some((0f64, positions));
        }

        let last_pos = positions.last().unwrap();
        let match_len = (last_pos + 1 - positions[0]) as f64;

        Some((
            (2f64 / (positions[0] + 1) as f64) + 1f64 / (last_pos + 1) as f64 - match_len,
            positions,
        ))
    }

    pub fn substr_indices(haystack: &str, niddle: &str) -> Option<(i64, Vec<usize>)> {
        substr_indices_impl(haystack, niddle).map(|(score, positions)| (score as i64, positions))
    }

    #[test]
    fn test_substr() {
        println!("{:?}", substr_indices("sr bl", "src/bun/blune"));
    }
}
