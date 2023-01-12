use types::Score;

/// Returns a bonus score if the match indices of an item include the file name part.
///
/// Formula:
///   bonus_score = base_score * len(matched_elements_in_file_name) / len(file_name)
pub(crate) fn calc_bonus_file_name(file_path: &str, score: Score, indices: &[usize]) -> Score {
    match pattern::extract_file_name(file_path) {
        Some((_, idx)) if file_path.len() > idx => {
            let hits_in_file_name = indices.iter().filter(|x| **x >= idx).count();
            score * hits_in_file_name as Score / (file_path.len() - idx) as Score
        }
        _ => 0,
    }
}
