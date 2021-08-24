use types::SourceItem;

use crate::Score;

/// Returns a bonus score if the match indices of an item include the filename part.
pub(crate) fn calc_bonus_filename(item: &SourceItem, score: Score, indices: &[usize]) -> Score {
    if let Some((_, idx)) = pattern::file_name_only(&item.raw) {
        if item.raw.len() > idx {
            let hits_filename = indices.iter().filter(|x| **x >= idx).count();
            // bonus = base_score * len(matched elements in filename) / len(filename)
            score * hits_filename as i64 / (item.raw.len() - idx) as i64
        } else {
            0
        }
    } else {
        0
    }
}
