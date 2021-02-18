use source_item::SourceItem;

use crate::Score;

#[derive(Debug, Clone)]
pub struct RecentFiles(Vec<String>);

impl RecentFiles {
    pub fn calc_bonus(&self, item: &SourceItem, base_score: Score) -> Score {
        if let Err(bonus) = self.0.iter().try_for_each(|s| {
            if s.contains(&item.raw) {
                let bonus = base_score / 3;
                Err(bonus)
            } else {
                Ok(())
            }
        }) {
            bonus
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
