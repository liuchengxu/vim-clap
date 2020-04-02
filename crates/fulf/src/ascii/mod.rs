//! Working with ASCII-only strings.
//!
//! Cheating!

mod matcher;
pub use matcher::{ascii_from_bytes, matcher};

use crate::scoring_utils::*;

#[inline]
pub fn score_with_positions(needle: &[u8], haystack: &[u8]) -> (Score, Vec<usize>) {
    let needle_length = needle.len();
    // empty needle
    if needle_length == 0 {
        return (SCORE_MIN, vec![]);
    }

    let haystack_length = haystack.len();

    // perfect match
    if needle_length == haystack_length {
        return (SCORE_MAX, (0..needle_length).collect());
    }

    let (d, m) = calculate_score(needle, haystack);
    let mut positions = vec![0_usize; needle_length];

    {
        let mut match_required = false;
        let mut j = haystack_length - 1;

        for i in (0..needle_length).rev() {
            while j > (0_usize) {
                let last = if i > 0 && j > 0 {
                    d.get(i - 1, j - 1)
                } else {
                    SCORE_DEFAULT_BONUS
                };

                let d = d.get(i, j);
                let m = m.get(i, j);

                if d != SCORE_MIN && (match_required || score_eq(d, m)) {
                    if i > 0 && j > 0 && score_eq(m, score_add(last, SCORE_MATCH_CONSECUTIVE)) {
                        match_required = true;
                    }

                    positions[i] = j;

                    break;
                }

                j -= 1
            }
        }
    }

    (m.get(needle_length - 1, haystack_length - 1), positions)
}

#[inline]
fn calculate_score(needle: &[u8], haystack: &[u8]) -> (Matrix, Matrix) {
    let bonus = compute_bonus(haystack);

    let needle_length = needle.len();
    let haystack_length = haystack.len();

    let mut m = Matrix::new(needle_length, haystack_length);
    let mut d = Matrix::new(needle_length, haystack_length);

    for (i, &n) in needle.iter().enumerate() {
        let mut prev_score = SCORE_MIN;
        let gap_score = if i == needle_length - 1 {
            SCORE_GAP_TRAILING
        } else {
            SCORE_GAP_INNER
        };

        for (j, &h) in haystack.iter().enumerate() {
            if eq(n, h) {
                let bonus_score = bonus[j];

                let score = match i {
                    0 => score_add(
                        bonus_score,
                        score_mul(score_from_usize(j), SCORE_GAP_LEADING),
                    ),
                    _ if j > 0 => {
                        let m = m.get(i - 1, j - 1);
                        let d = d.get(i - 1, j - 1);

                        let m = score_add(m, bonus_score);
                        let d = score_add(d, SCORE_MATCH_CONSECUTIVE);

                        (m).max(d)
                    }
                    _ => SCORE_MIN,
                };

                prev_score = score.max(score_add(prev_score, gap_score));

                d.set(i, j, score);
                m.set(i, j, prev_score);
            } else {
                prev_score = score_add(prev_score, gap_score);

                d.set(i, j, SCORE_MIN);
                m.set(i, j, prev_score);
            }
        }
    }

    (d, m)
}

/// Compares two characters case-insensitively
#[inline]
fn eq(a: u8, b: u8) -> bool {
    a.eq_ignore_ascii_case(&b)
}

#[inline]
fn compute_bonus(haystack: &[u8]) -> Vec<Score> {
    let mut last_char = b'/';

    let len = haystack.len();

    haystack
        .iter()
        .fold(Vec::with_capacity(len), |mut vec, &ch| {
            vec.push(bonus_for_char(last_char, ch));
            last_char = ch;
            vec
        })
}

#[inline]
fn bonus_for_char(prev: u8, current: u8) -> Score {
    match current {
        b'a'..=b'z' | b'0'..=b'9' => bonus_for_prev(prev),
        b'A'..=b'Z' => match prev {
            b'a'..=b'z' => SCORE_MATCH_CAPITAL,
            _ => bonus_for_prev(prev),
        },
        _ => SCORE_DEFAULT_BONUS,
    }
}

#[inline]
fn bonus_for_prev(ch: u8) -> Score {
    match ch {
        b'/' => SCORE_MATCH_SLASH,
        b'-' | b'_' | b' ' => SCORE_MATCH_WORD,
        b'.' => SCORE_MATCH_DOT,
        _ => SCORE_DEFAULT_BONUS,
    }
}
