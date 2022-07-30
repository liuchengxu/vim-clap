//! Add a bonus to the comment line or the line that can have a declaration.
//!
//! Ref: https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el

use crate::Score;

pub type FileExtension = String;

#[derive(Debug, Clone)]
pub struct Language(FileExtension);

impl<T: AsRef<str>> From<T> for Language {
    fn from(s: T) -> Self {
        Self(s.as_ref().into())
    }
}

impl Language {
    pub fn calc_bonus(&self, bonus_text: &str, base_score: Score) -> Score {
        let trimmed = bonus_text.trim_start();

        if dumb_analyzer::is_comment(trimmed, &self.0) {
            -(base_score / 5)
        } else {
            match dumb_analyzer::calculate_pattern_priority(trimmed, &self.0) {
                Some(priority) => base_score / priority.as_i32(),
                None => 0,
            }
        }
    }
}
