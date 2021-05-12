//! # Why the fork?
//!
//! Okay, the one and only reason for this lib is my OS.
//!
//! Original "rff" crate has `terminal` module which utilizes `std::os::unix`
//! thus it doesn't compile on non-unix OS.
//!
//! # Fork differences
//!
//! * Support "smart case" searching. Ref https://github.com/liuchengxu/vim-clap/pull/541

mod scoring_utils;

use crate::scoring_utils::*;

pub type MatchWithPositions = (Score, Vec<usize>);

pub fn match_and_score_with_positions(needle: &str, haystack: &str) -> Option<MatchWithPositions> {
    let lowercased;
    let haystack = if needle.chars().any(|c| c.is_uppercase()) {
        haystack
    } else {
        lowercased = haystack.to_lowercase();
        &lowercased
    };

    // The another approach to avoid the unnecessary allocation in the case of `needle` contains
    // any uppercase char is using `Option`.
    //
    // Ref https://github.com/liuchengxu/vim-clap/pull/541#discussion_r507020114

    /*
      let lowercase_haystack = if needle.chars().any(|c| c.is_uppercase()) {
          None
      } else {
          Some(haystack.to_lowercase())
      };
      let haystack = lowercase_haystack.as_deref().unwrap_or(haystack);
    */

    matches(needle, haystack)
        .map(|needle_length| score_with_positions(needle, needle_length, haystack))
}

/// Searches for needle's chars in the haystack.
/// Returns `None` if haystack doesn't hold all needle's chars.
/// Returns `Some(len)` with needle's length otherwise.
///
/// # Examples
///
// This is a proper code, that should compile, but `matches()` function is private.
/// ```compile_fail
/// assert_eq!(Some(5), extracted_fzy::matches("amo汉漢", "app/models/order/汉语/漢語"));
/// assert_eq!(6, "汉漢".len()); // Length of this two chars in bytes.
/// ```
#[inline]
fn matches(needle: &str, haystack: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }

    let mut hchars = haystack.chars();

    // Use loop instead of `needle.all()`, to count needle's length.
    let mut needle_length = 0;
    for n in needle.chars() {
        if !hchars.any(|h| eq(n, h)) {
            return None;
        }
        needle_length += 1;
    }
    Some(needle_length)
}

fn score_with_positions(needle: &str, needle_length: usize, haystack: &str) -> (Score, Vec<usize>) {
    // empty needle
    if needle_length == 0 {
        return (SCORE_MIN, vec![]);
    }

    let haystack_length = haystack.chars().count();

    // perfect match
    if needle_length == haystack_length {
        return (SCORE_MAX, (0..needle_length).collect());
    }

    // unreasonably large haystack
    if haystack_length > 1024 {
        return (SCORE_MIN, vec![]);
    }

    #[allow(non_snake_case)]
    let (D, M) = calculate_score(needle, needle_length, haystack, haystack_length);

    let mut positions = vec![0_usize; needle_length];

    {
        let mut match_required = false;
        let mut j = haystack_length - 1;

        for i in (0..needle_length).rev() {
            while j > 0_usize {
                let last = if i > 0 && j > 0 {
                    D.get(i - 1, j - 1)
                } else {
                    SCORE_DEFAULT_BONUS
                };

                let d = D.get(i, j);
                let m = M.get(i, j);

                if d != SCORE_MIN && (match_required || score_eq(d, m)) {
                    match_required =
                        i > 0 && j > 0 && score_eq(m, score_add(last, SCORE_MATCH_CONSECUTIVE));
                    positions[i] = j;
                    j -= 1;
                    break;
                }

                j -= 1
            }
        }
    }

    (M.get(needle_length - 1, haystack_length - 1), positions)
}

fn calculate_score(
    needle: &str,
    needle_length: usize,
    haystack: &str,
    haystack_length: usize,
) -> (Matrix, Matrix) {
    let bonus = compute_bonus(haystack, haystack_length);

    #[allow(non_snake_case)]
    let mut M = Matrix::new(needle_length, haystack_length);
    #[allow(non_snake_case)]
    let mut D = Matrix::new(needle_length, haystack_length);

    for (i, n) in needle.chars().enumerate() {
        let mut prev_score = SCORE_MIN;
        let gap_score = if i == needle_length - 1 {
            SCORE_GAP_TRAILING
        } else {
            SCORE_GAP_INNER
        };

        for (j, h) in haystack.chars().enumerate() {
            if eq(n, h) {
                let bonus_score = bonus[j];

                let score = match i {
                    0 => score_add(
                        bonus_score,
                        score_mul(score_from_usize(j), SCORE_GAP_LEADING),
                    ),
                    _ if j > 0 => {
                        let m = score_add(M.get(i - 1, j - 1), bonus_score);
                        let d = score_add(D.get(i - 1, j - 1), SCORE_MATCH_CONSECUTIVE);
                        m.max(d)
                    }
                    _ => SCORE_MIN,
                };

                prev_score = score.max(score_add(prev_score, gap_score));

                D.set(i, j, score);
                M.set(i, j, prev_score);
            } else {
                prev_score = score_add(prev_score, gap_score);

                D.set(i, j, SCORE_MIN);
                M.set(i, j, prev_score);
            }
        }
    }

    (D, M)
}

/// Compares two characters
#[inline(always)]
fn eq(a: char, b: char) -> bool {
    a == b
}

/// Compares two characters case-insensitively
///
/// The origin fzy algo uses `eq_ignore_case`, but we just use `eq` now.
#[allow(unused)]
fn eq_ignore_case(a: char, b: char) -> bool {
    match a {
        _ if a == b => true,
        _ if a.is_ascii() || b.is_ascii() => a.eq_ignore_ascii_case(&b),
        _ => a.to_lowercase().eq(b.to_lowercase()),
    }
}

fn compute_bonus(haystack: &str, haystack_length: usize) -> Vec<Score> {
    let mut last_char = '/';

    let len = haystack_length;

    haystack
        .chars()
        .fold(Vec::with_capacity(len), |mut vec, ch| {
            vec.push(bonus_for_char(last_char, ch));
            last_char = ch;
            vec
        })
}

fn bonus_for_char(prev: char, current: char) -> Score {
    match current {
        'a'..='z' | '0'..='9' => bonus_for_prev(prev),
        'A'..='Z' => match prev {
            'a'..='z' => SCORE_MATCH_CAPITAL,
            _ => bonus_for_prev(prev),
        },
        _ => SCORE_DEFAULT_BONUS,
    }
}

fn bonus_for_prev(ch: char) -> Score {
    match ch {
        '/' => SCORE_MATCH_SLASH,
        '-' | '_' | ' ' => SCORE_MATCH_WORD,
        '.' => SCORE_MATCH_DOT,
        _ => SCORE_DEFAULT_BONUS,
    }
}

/// The Matrix type represents a 2-dimensional Matrix.
struct Matrix {
    cols: usize,
    contents: Vec<Score>,
}

impl Matrix {
    /// Creates a new Matrix with the given width and height
    fn new(width: usize, height: usize) -> Matrix {
        Matrix {
            contents: vec![SCORE_STARTER; width * height],
            cols: width,
        }
    }

    /// Returns a reference to the specified coordinates of the Matrix
    fn get(&self, col: usize, row: usize) -> Score {
        debug_assert!(col * row < self.contents.len());
        unsafe { *self.contents.get_unchecked(row * self.cols + col) }
    }

    /// Sets the coordinates of the Matrix to the specified value
    fn set(&mut self, col: usize, row: usize, val: Score) {
        debug_assert!(col * row < self.contents.len());
        unsafe {
            *self.contents.get_unchecked_mut(row * self.cols + col) = val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_insensitive() {
        let result = match_and_score_with_positions("def", "abc DEF ghi");
        assert_eq!(result, Some((552, vec![4, 5, 6])));

        let result = match_and_score_with_positions("def", "abc def ghi");
        assert_eq!(result, Some((552, vec![4, 5, 6])));

        let result = match_and_score_with_positions("xyz", "abc def ghi");
        assert_eq!(result, None);
    }

    #[test]
    fn smart_case() {
        let result = match_and_score_with_positions("Def", "abc Def ghi");
        assert_eq!(result, Some((552, vec![4, 5, 6])));

        let result = match_and_score_with_positions("Def", "abc def ghi");
        assert_eq!(result, None);
    }
}
