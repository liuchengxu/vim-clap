use types::Score;

/// Returns a bonus score if the match indices of an item include the file name part.
///
/// Formula:
///   bonus_score = base_score * len(matched_elements_in_file_name) / len(file_name)
pub(crate) fn calc_bonus_file_name(file_path: &str, score: Score, indices: &[usize]) -> Score {
    match pattern::extract_file_name(file_path) {
        Some((file_name, idx)) if !file_name.is_empty() => {
            let hits_in_file_name = indices.iter().filter(|x| **x >= idx).count();
            score * hits_in_file_name as Score / file_name.len() as Score
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_bonus_file_name() {
        let bonus_score = calc_bonus_file_name("autoload/clap/action.vim", 10, &[1, 2, 3, 20, 25]);
        assert_eq!(bonus_score, 2);
    }
}
