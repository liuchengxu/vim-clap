use crate::scoring_utils::*;

pub trait FzyItem: Copy {
    /// Virtual char: inserted before the first real char.
    ///
    /// It's used to compute the score for the first real char.
    const INIT: Self;

    /// Compares two characters case-insensitively.
    fn eq(a: Self, b: Self) -> bool;

    fn bonus_for_char(prev: Self, current: Self) -> Score;

    fn bonus_for_prev(ch: Self) -> Score;
}

use FzyItem as FzyI;

// Implementing for the reference because slice iter gives references.
impl FzyItem for &u8 {
    const INIT: Self = &b'/';

    #[inline]
    fn eq(a: Self, b: Self) -> bool {
        a.eq_ignore_ascii_case(b)
    }

    #[inline]
    fn bonus_for_char(prev: Self, current: Self) -> Score {
        match current {
            b'a'..=b'z' | b'0'..=b'9' => FzyI::bonus_for_prev(prev),
            b'A'..=b'Z' => match prev {
                b'a'..=b'z' => SCORE_MATCH_CAPITAL,
                _ => FzyI::bonus_for_prev(prev),
            },
            _ => SCORE_DEFAULT_BONUS,
        }
    }

    #[inline]
    fn bonus_for_prev(ch: Self) -> Score {
        match ch {
            b'/' => SCORE_MATCH_SLASH,
            b'-' | b'_' | b' ' => SCORE_MATCH_WORD,
            b'.' => SCORE_MATCH_DOT,
            _ => SCORE_DEFAULT_BONUS,
        }
    }
}

impl FzyItem for char {
    const INIT: Self = '/';

    #[inline]
    fn eq(a: char, b: char) -> bool {
        a == b
            || if a.is_ascii() || b.is_ascii() {
                a.eq_ignore_ascii_case(&b)
            } else {
                a.to_lowercase().eq(b.to_lowercase())
            }
    }

    #[inline]
    fn bonus_for_char(prev: char, current: char) -> Score {
        match current {
            'a'..='z' | '0'..='9' => FzyI::bonus_for_prev(prev),
            'A'..='Z' => match prev {
                'a'..='z' => SCORE_MATCH_CAPITAL,
                _ => FzyI::bonus_for_prev(prev),
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
}

/// The `IntoIterator` trait is not implemented for strings.
pub trait FzyScorable: Copy {
    type FzyIter: Iterator;

    fn fzy_iter(self) -> Self::FzyIter;
}

impl<'a> FzyScorable for &'a [u8] {
    type FzyIter = std::slice::Iter<'a, u8>;

    fn fzy_iter(self) -> Self::FzyIter {
        self.iter()
    }
}

impl<'a> FzyScorable for &'a str {
    type FzyIter = std::str::Chars<'a>;

    fn fzy_iter(self) -> Self::FzyIter {
        self.chars()
    }
}

pub fn score_with_positions<A, S>(
    needle: A,
    needle_length: usize,
    haystack: A,
) -> (Score, Vec<usize>)
where
    A: FzyScorable,
    A::FzyIter: Iterator<Item = S>,
    S: FzyItem,
{
    // empty needle
    if needle_length == 0 {
        return (SCORE_MIN, vec![]);
    }

    let haystack_length = haystack.fzy_iter().count();

    // perfect match
    if needle_length == haystack_length {
        return (SCORE_MAX, (0..needle_length).collect());
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

                j -= 1;
            }
        }
    }

    (M.get(needle_length - 1, haystack_length - 1), positions)
}

fn calculate_score<A, S>(
    needle: A,
    needle_length: usize,
    haystack: A,
    haystack_length: usize,
) -> (Matrix, Matrix)
where
    A: FzyScorable,
    A::FzyIter: Iterator<Item = S>,
    S: FzyItem,
{
    let bonus = compute_bonus(haystack, haystack_length);

    #[allow(non_snake_case)]
    let mut M = Matrix::new(needle_length, haystack_length);
    #[allow(non_snake_case)]
    let mut D = Matrix::new(needle_length, haystack_length);

    for (i, n) in needle.fzy_iter().enumerate() {
        let mut prev_score = SCORE_MIN;
        let gap_score = if i == needle_length - 1 {
            SCORE_GAP_TRAILING
        } else {
            SCORE_GAP_INNER
        };

        for (j, h) in haystack.fzy_iter().enumerate() {
            if S::eq(n, h) {
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

fn compute_bonus<A, S>(haystack: A, haystack_length: usize) -> Vec<Score>
where
    A: FzyScorable,
    A::FzyIter: Iterator<Item = S>,
    S: FzyItem,
{
    let mut last_char = S::INIT;

    let len = haystack_length;

    haystack
        .fzy_iter()
        .fold(Vec::with_capacity(len), |mut vec, ch| {
            vec.push(FzyI::bonus_for_char(last_char, ch));
            last_char = ch;
            vec
        })
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
        debug_assert!(row * self.cols + col < self.contents.len());
        unsafe { *self.contents.get_unchecked(row * self.cols + col) }
    }

    /// Sets the coordinates of the Matrix to the specified value
    fn set(&mut self, col: usize, row: usize, val: Score) {
        debug_assert!(row * self.cols + col < self.contents.len());
        unsafe {
            *self.contents.get_unchecked_mut(row * self.cols + col) = val;
        }
    }
}
