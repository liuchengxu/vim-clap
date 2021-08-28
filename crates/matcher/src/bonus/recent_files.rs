use crate::Score;

#[derive(Debug, Clone)]
pub struct RecentFiles(Vec<String>);

impl RecentFiles {
    pub fn calc_bonus(&self, full_line: &str, base_score: Score) -> Score {
        if self.0.iter().any(|s| s.contains(full_line)) {
            base_score / 3
        } else {
            0
        }
    }
}

impl From<Vec<String>> for RecentFiles {
    fn from(inner: Vec<String>) -> Self {
        Self(inner)
    }
}
