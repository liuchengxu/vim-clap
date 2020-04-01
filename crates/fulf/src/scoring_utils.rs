use std::convert::TryFrom;

pub(crate) type Score = i32;
pub(crate) type MatchWithPositions = (Score, Vec<usize>);

pub type ScoringResult = (Box<str>, Score, Box<[usize]>);
pub type MWP = ScoringResult;

pub(crate) const SCORE_STARTER: Score = 0;

pub(crate) const SCORE_DEFAULT_BONUS: Score = 0;
pub(crate) const SCORE_MAX: Score = Score::max_value();
pub(crate) const SCORE_MIN: Score = Score::min_value();
pub(crate) const SCORE_GAP_LEADING: Score = -1;
pub(crate) const SCORE_GAP_TRAILING: Score = -1;
pub(crate) const SCORE_GAP_INNER: Score = -2;
pub(crate) const SCORE_MATCH_CONSECUTIVE: Score = 200;
pub(crate) const SCORE_MATCH_SLASH: Score = 180;
pub(crate) const SCORE_MATCH_WORD: Score = 160;
pub(crate) const SCORE_MATCH_CAPITAL: Score = 140;
pub(crate) const SCORE_MATCH_DOT: Score = 120;

/// Returns `true` if scores can be considered equal
/// and `false` if not.
#[inline]
pub(crate) fn score_eq(score: Score, rhs: Score) -> bool {
    score == rhs
}

/// Adds `rhs` to the score and returns the result.
#[inline]
pub(crate) fn score_add(score: Score, rhs: Score) -> Score {
    score.saturating_add(rhs)
}

/// Multiplies `score` by `rhs`.
#[inline]
pub(crate) fn score_mul(score: Score, rhs: Score) -> Score {
    score.saturating_mul(rhs)
}

#[inline]
pub(crate) fn score_from_usize(u: usize) -> Score {
    Score::try_from(u).unwrap_or(SCORE_MAX)
}

/// The Matrix type represents a 2-dimensional Matrix.
pub(crate) struct Matrix {
    cols: usize,
    contents: Vec<Score>,
}

impl Matrix {
    /// Creates a new Matrix with the given width and height
    #[inline]
    pub fn new(width: usize, height: usize) -> Matrix {
        Matrix {
            contents: vec![SCORE_STARTER; width * height],
            cols: width,
        }
    }

    /// Returns a reference to the specified coordinates of the Matrix
    #[inline]
    pub fn get(&self, col: usize, row: usize) -> Score {
        debug_assert!(col * row < self.contents.len());
        unsafe { *self.contents.get_unchecked(row * self.cols + col) }
    }

    /// Sets the coordinates of the Matrix to the specified value
    #[inline]
    pub fn set(&mut self, col: usize, row: usize, val: Score) {
        debug_assert!(col * row < self.contents.len());
        unsafe {
            *self.contents.get_unchecked_mut(row * self.cols + col) = val;
        }
    }
}
