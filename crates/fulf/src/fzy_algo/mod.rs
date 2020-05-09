pub mod ascii;
pub mod scoring_utils;
pub mod utf8;

use {scoring_utils::*, std::mem};

/// Implementors could be scored by the algorithm.
///
/// Implemented for `char` and `&u8`.
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
/// But this trait is implemented for strings via `.chars()` method.
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

/// The main function to score the things.
///
/// This function doesn't check the string for validity, only scores it.
/// Probably, you wanted to use `match_and_score_with_positions()`
/// from the utf8 or ascii modules?
pub fn score_with_positions<A, S>(
    needle: A,
    needle_length: usize,
    haystack: A,
    prealloced_matricies: &mut (Vec<Score>, Vec<Score>),
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
    let (D, M) = calculate_score(
        needle,
        needle_length,
        haystack,
        haystack_length,
        prealloced_matricies,
    );

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

    // Get the score.
    let score = M.get(needle_length - 1, haystack_length - 1);
    // Put the matrix storage back.
    mem::replace(prealloced_matricies, (M.destroy(), D.destroy()));
    // Return the score and positions.
    (score, positions)
}

fn calculate_score<A, S>(
    needle: A,
    needle_length: usize,
    haystack: A,
    haystack_length: usize,
    prealloced_matricies: &mut (Vec<Score>, Vec<Score>),
) -> (Matrix, Matrix)
where
    A: FzyScorable,
    A::FzyIter: Iterator<Item = S>,
    S: FzyItem,
{
    let bonus = compute_bonus(haystack, haystack_length);

    let (m, d) = mem::take(prealloced_matricies);

    #[allow(non_snake_case)]
    let mut M = Matrix::new(needle_length, haystack_length, m);
    #[allow(non_snake_case)]
    let mut D = Matrix::new(needle_length, haystack_length, d);

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
    fn new(width: usize, height: usize, storage: Vec<Score>) -> Matrix {
        /// Initializer.
        ///
        /// That's very strange, but I hadn't found initializer for `Copy`
        /// items within vector's functions, only `resize_with()` for `Clone`.
        fn init_vec<T: Copy>(v: &mut Vec<T>, init: T, len: usize) {
            v.clear();
            v.reserve_exact(len);

            let mut ptr = v.as_mut_ptr();
            for _ in 0..len {
                //x SAFETY: this follows the restrictions of `add()`.
                unsafe {
                    std::ptr::write(ptr, init);
                    ptr = ptr.add(1);
                }
            }
            //x SAFETY: `T` is `Copy`,
            //x `v.reserve_exact(len);` gives enough capacity,
            //x and all items in the range of (0..len) were
            //x initialized in the loop up there.
            unsafe {
                v.set_len(len);
            }
        }

        let mut storage = storage;
        init_vec(&mut storage, SCORE_STARTER, width * height);

        Matrix {
            contents: storage,
            cols: width,
        }
    }

    /// Returns the inner vector from the matrix.
    fn destroy(self) -> Vec<Score> {
        self.contents
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
