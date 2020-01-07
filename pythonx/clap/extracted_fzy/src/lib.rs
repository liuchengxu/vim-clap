//! Okay, the one and only reason for this lib is my OS.
//!
//! Original "rff" crate has `terminal` module which utilizes `std::os::unix`
//! thus it doesn't compile on non-unix OS.

use std::f64::{INFINITY, NEG_INFINITY};

pub const SCORE_MAX: f64 = INFINITY;
pub const SCORE_MIN: f64 = NEG_INFINITY;
pub const SCORE_GAP_LEADING: f64 = -0.005;
pub const SCORE_GAP_TRAILING: f64 = -0.005;
pub const SCORE_GAP_INNER: f64 = -0.01;
pub const SCORE_MATCH_CONSECUTIVE: f64 = 1.0;
pub const SCORE_MATCH_SLASH: f64 = 0.9;
pub const SCORE_MATCH_WORD: f64 = 0.8;
pub const SCORE_MATCH_CAPITAL: f64 = 0.7;
pub const SCORE_MATCH_DOT: f64 = 0.6;

pub type MatchWithPositions<'a> = (f64, Vec<usize>);

pub fn match_and_score_with_positions<'a>(
    needle: &str,
    haystack: &'a str,
) -> Option<MatchWithPositions<'a>> {
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
/// ```
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

fn score_with_positions(needle: &str, needle_length: usize, haystack: &str) -> (f64, Vec<usize>) {
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

    let (d, m) = calculate_score(needle, needle_length, haystack, haystack_length);
    let mut positions = vec![0 as usize; needle_length];

    {
        let mut match_required = false;
        let mut j = haystack_length - 1;

        for i in (0..needle_length).rev() {
            while j > (0 as usize) {
                let last = if i > 0 && j > 0 {
                    d.get(i - 1, j - 1)
                } else {
                    0.0
                };

                let d = d.get(i, j);
                let m = m.get(i, j);

                if d != SCORE_MIN && (match_required || d == m) {
                    if i > 0 && j > 0 && m == last + SCORE_MATCH_CONSECUTIVE {
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
                    0 => ((j as f64) * SCORE_GAP_LEADING) + bonus_score,
                    _ if j > 0 => {
                        let m = m.get(i - 1, j - 1);
                        let d = d.get(i - 1, j - 1);

                        let m = m + bonus_score;
                        let d = d + SCORE_MATCH_CONSECUTIVE;

                        (m).max(d)
                    }
                    _ => SCORE_MIN,
                };

                prev_score = score.max(prev_score + gap_score);

                d.set(i, j, score);
                m.set(i, j, prev_score);
            } else {
                prev_score += gap_score;

                d.set(i, j, SCORE_MIN);
                m.set(i, j, prev_score);
            }
        }
    }

    (d, m)
}

/// Compares two characters case-insensitively
#[inline(always)]
fn eq(a: char, b: char) -> bool {
    match a {
        _ if a == b => true,
        _ if a.is_ascii() || b.is_ascii() => a.eq_ignore_ascii_case(&b),
        _ => a.to_lowercase().eq(b.to_lowercase()),
    }
}

fn compute_bonus(haystack: &str, haystack_length: usize) -> Vec<f64> {
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

fn bonus_for_char(prev: char, current: char) -> f64 {
    match current {
        'a'..='z' | '0'..='9' => bonus_for_prev(prev),
        'A'..='Z' => match prev {
            'a'..='z' => SCORE_MATCH_CAPITAL,
            _ => bonus_for_prev(prev),
        },
        _ => 0.0,
    }
}

fn bonus_for_prev(ch: char) -> f64 {
    match ch {
        '/' => SCORE_MATCH_SLASH,
        '-' | '_' | ' ' => SCORE_MATCH_WORD,
        '.' => SCORE_MATCH_DOT,
        _ => 0.0,
    }
}

/// The Matrix type represents a 2-dimensional Matrix.
struct Matrix {
    cols: usize,
    contents: Vec<f64>,
}

impl Matrix {
    /// Creates a new Matrix with the given width and height
    fn new(width: usize, height: usize) -> Matrix {
        Matrix {
            contents: vec![0.0; width * height],
            cols: width,
        }
    }

    /// Returns a reference to the specified coordinates of the Matrix
    fn get(&self, col: usize, row: usize) -> f64 {
        debug_assert!(col * row < self.contents.len());
        unsafe { *self.contents.get_unchecked(row * self.cols + col) }
    }

    /// Sets the coordinates of the Matrix to the specified value
    fn set(&mut self, col: usize, row: usize, val: f64) {
        debug_assert!(col * row < self.contents.len());
        unsafe {
            *self.contents.get_unchecked_mut(row * self.cols + col) = val;
        }
    }
}
