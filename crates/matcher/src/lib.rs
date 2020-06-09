//! This crate provides various matcher algorithms for line oriented search given the query string.
//!
//! The matcher result consists of the score and the indices of matched items.
//!
//! There two steps to match a line:
//!
//! //     raw line
//! //        |
//! //        |  LineSplitter: split out the content to match.
//! //        |
//! //        ↓
//! //  content to match
//! //        |
//! //        |          Algo: apply the match algorithm.
//! //        |
//! //        ↓
//! //   MatcherResult
//!

mod algo;
mod line_splitter;

pub use algo::*;
pub use line_splitter::*;

// Returns the score and indices of matched chars
// when the line is matched given the query,
pub type MatcherResult = Option<(i64, Vec<usize>)>;

/// Returns the appropriate matcher given the algo and line_splitter strategy.
#[inline]
pub fn get_appropriate_matcher(
    algo: &Algo,
    line_splitter: &LineSplitter,
) -> impl Fn(&str, &str) -> MatcherResult {
    match algo {
        Algo::Skim => match line_splitter {
            LineSplitter::Full => skim::fuzzy_indices,
            LineSplitter::TagNameOnly => apply_on_tag_line_skim,
            LineSplitter::FileNameOnly => apply_on_file_line_skim,
            LineSplitter::GrepExcludeFilePath => apply_on_grep_line_skim,
        },
        Algo::Fzy => match line_splitter {
            LineSplitter::Full => fzy::fuzzy_indices,
            LineSplitter::TagNameOnly => apply_on_tag_line_fzy,
            LineSplitter::FileNameOnly => apply_on_file_line_fzy,
            LineSplitter::GrepExcludeFilePath => apply_on_grep_line_fzy,
        },
        Algo::SubString => match line_splitter {
            LineSplitter::Full => substring::substr_indices,
            LineSplitter::TagNameOnly => apply_on_tag_line_substr,
            LineSplitter::FileNameOnly => apply_on_file_line_substr,
            LineSplitter::GrepExcludeFilePath => apply_on_grep_line_substr,
        },
    }
}
