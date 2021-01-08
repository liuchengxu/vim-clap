//! Working with ASCII-only strings.
//!
//! Cheating!

use {
    super::{
        score_with_positions,
        scoring_utils::{MatchWithPositions, Score},
    },
    memchr::memchr,
    std::cmp,
};

#[inline]
pub fn match_and_score_with_positions(
    needle: &[u8],
    haystack: &[u8],
    prealloced_matricies: &mut (Vec<Score>, Vec<Score>),
) -> Option<MatchWithPositions> {
    matcher(haystack, needle)
        .map(|_| score_with_positions(needle, needle.len(), haystack, prealloced_matricies))
}

type LineMetaData = ();

/// Checks the line, returns `Some()` if it will provide some score.
#[inline]
pub fn matcher(line: &[u8], needle: &[u8]) -> Option<LineMetaData> {
    let mut nee_len = needle.len();
    let mut line = line;

    for &letter in needle.iter() {
        if line.len() < nee_len {
            return None;
        }

        let rcase_idx = reverse_ascii_case(letter).and_then(|rev_letter| memchr(rev_letter, line));

        let ocase_idx = memchr(letter, line);

        // ASCII letter length is always 1, so this is always 1 + ...
        let next_idx = 1 + match (ocase_idx, rcase_idx) {
            (Some(o_idx), Some(r_idx)) => cmp::min(o_idx, r_idx),
            (Some(o_idx), None) => o_idx,
            (None, Some(r_idx)) => r_idx,
            (None, None) => return None,
        };

        nee_len -= 1;
        line = &line[next_idx..];
    }

    Some(())
}

/// Reverses the case of a byte:
/// lowercase letter becomes uppercased,
/// and uppercase letter becomes lowercased.
///
/// # Returns
///
/// * `Some(rev)`, if the byte is alphabetic,
/// where `rev` is a byte with reversed case.
///
/// * `None` otherwise.
///
/// # Examples
///
/// ```ignore
/// use fulf::ascii::reverse_ascii_case;
///
/// let iter = b"Hello, World!"
///     .iter()
///     .cloned()
///     .filter_map(reverse_ascii_case);
///
/// assert!(iter.eq(b"hELLOwORLD".iter().cloned()));
/// ```
#[inline]
fn reverse_ascii_case(byte: u8) -> Option<u8> {
    let is_alphabetic = byte.is_ascii_alphabetic();
    if is_alphabetic {
        // Reverse the state of the fifth bit.
        Some(byte ^ ((is_alphabetic as u8) << 5))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverse_case() {
        let iter = b"Hello, World!"
            .iter()
            .cloned()
            .filter_map(reverse_ascii_case);

        assert!(iter.eq(b"hELLOwORLD".iter().cloned()));
    }
}
