use crate::Score;

/// Returns a bonus score if the match indices of an item include the file name part.
///
/// Formula:
///   bonus_score = base_score * len(matched_elements_in_file_name) / len(file_name)
pub(crate) fn calc_bonus_file_name(file_path: &str, score: Score, indices: &[usize]) -> Score {
    // TODO: since the pattern is always `crates/pattern/src/lib.rs`, we could use a more efficient file name parsing then.
    match pattern::find_file_name(file_path) {
        Some((_, idx)) if file_path.len() > idx => {
            let hits_in_file_name = indices.iter().filter(|x| **x >= idx).count();
            score * hits_in_file_name as i64 / (file_path.len() - idx) as i64
        }
        _ => 0,
    }
}
