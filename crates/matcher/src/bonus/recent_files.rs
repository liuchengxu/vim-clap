use crate::Score;

#[derive(Debug, Clone)]
pub struct RecentFiles(Vec<String>);

impl RecentFiles {
    pub fn calc_bonus(&self, bonus_text: &str, base_score: Score) -> Score {
        if self.0.iter().any(|s| s.contains(bonus_text)) {
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
