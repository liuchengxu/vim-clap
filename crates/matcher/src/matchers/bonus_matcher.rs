use crate::bonus::Bonus;
use std::sync::Arc;
use types::{ClapItem, Score};

/// [`BonusMatcher`] only tweaks the match score.
#[derive(Debug, Clone, Default)]
pub struct BonusMatcher {
    bonuses: Vec<Bonus>,
}

impl BonusMatcher {
    pub fn new(bonuses: Vec<Bonus>) -> Self {
        Self { bonuses }
    }

    /// Returns the sum of bonus score.
    pub fn calc_item_bonus(
        &self,
        item: &Arc<dyn ClapItem>,
        base_score: Score,
        base_indices: &[usize],
    ) -> Score {
        self.bonuses
            .iter()
            .map(|b| b.item_bonus_score(item, base_score, base_indices))
            .sum()
    }

    /// Returns the sum of bonus score.
    pub fn calc_text_bonus(
        &self,
        bonus_text: &str,
        base_score: Score,
        base_indices: &[usize],
    ) -> Score {
        self.bonuses
            .iter()
            .map(|b| b.text_bonus_score(bonus_text, base_score, base_indices))
            .sum()
    }
}
