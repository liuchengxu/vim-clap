//! Working with utf8-encoded strings.

use crate::scoring_utils::*;

#[inline]
pub fn match_and_score_with_positions(needle: &str, haystack: &str) -> Option<MatchWithPositions> {
    match matches(needle, haystack) {
        Some(needle_length) => {
            let (score, positions) = score_with_positions(needle, needle_length, haystack);
            Some((score, positions))
        }
        None => None,
    }
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
    if needle.is_empty() || needle == haystack {
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

#[inline]
pub fn score_with_positions(
    needle: &str,
    needle_length: usize,
    haystack: &str,
) -> (Score, Vec<usize>) {
    // empty needle
    if needle_length == 0 {
        return (SCORE_MIN, vec![]);
    }

    let haystack_length = haystack.chars().count();

    // perfect match
    if needle_length == haystack_length {
        return (SCORE_MAX, (0..needle_length).collect());
    }

    let (d, m) = calculate_score(needle, needle_length, haystack, haystack_length);
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
fn calculate_score(
    needle: &str,
    needle_length: usize,
    haystack: &str,
    haystack_length: usize,
) -> (Matrix, Matrix) {
    let bonus = compute_bonus(haystack, haystack_length);

    let mut m = Matrix::new(needle_length, haystack_length);
    let mut d = Matrix::new(needle_length, haystack_length);

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
fn eq(a: char, b: char) -> bool {
    match a {
        _ if a == b => true,
        _ if a.is_ascii() || b.is_ascii() => a.eq_ignore_ascii_case(&b),
        _ => a.to_lowercase().eq(b.to_lowercase()),
    }
}

#[inline]
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

#[inline]
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

#[inline]
fn bonus_for_prev(ch: char) -> Score {
    match ch {
        '/' => SCORE_MATCH_SLASH,
        '-' | '_' | ' ' => SCORE_MATCH_WORD,
        '.' => SCORE_MATCH_DOT,
        _ => SCORE_DEFAULT_BONUS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abc() {
        let abc = "abc";
        let cba = "cba";
        let res = match_and_score_with_positions(abc, cba);
        // assert!(res.is_some());
        assert!(res.is_none());
        println!("{:?}", res);
    }
}
