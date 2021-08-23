//! Add a bonus score for the file matching current working directory.

use types::SourceItem;

use crate::Score;

/// Used for recent_files provider.
///
/// Each entry of recent_files provider is an absolute path String.
#[derive(Clone, Debug)]
pub struct Cwd {
    /// Absolute path String.
    pub abs_path: String,
}

impl From<String> for Cwd {
    fn from(abs_path: String) -> Self {
        Self { abs_path }
    }
}

impl Cwd {
    pub fn calc_bonus(&self, item: &SourceItem, base_score: Score) -> Score {
        if item.raw.starts_with(&self.abs_path) {
            base_score / 2
        } else {
            0
        }
    }
}
