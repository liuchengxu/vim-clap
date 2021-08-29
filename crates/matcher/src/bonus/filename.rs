use crate::Score;

/// Returns a bonus score if the match indices of an item include the filename part.
///
/// Formula: bonus_score = base_score * len(matched_elements_in_filename) / len(filename)
pub(crate) fn calc_bonus_filename(bonus_text: &str, score: Score, indices: &[usize]) -> Score {
    if let Some((_, idx)) = pattern::file_name_only(bonus_text) {
        if bonus_text.len() > idx {
            let hits_filename = indices.iter().filter(|x| **x >= idx).count();
            score * hits_filename as i64 / (bonus_text.len() - idx) as i64
        } else {
            0
        }
    } else {
        0
    }
}
