use matcher::WordMatcher;
use std::io::Result;
use std::path::Path;
use utils::read_lines_from;

pub fn find_highlights(
    source_file: &Path,
    start: usize,
    end: usize,
    word: String,
) -> Result<Vec<(usize, usize)>> {
    let word_matcher = WordMatcher::new(vec![word.into()]);
    Ok(read_lines_from(source_file, start, end - start)?
        .enumerate()
        .filter_map(|(idx, line)| {
            word_matcher
                .find_matches(&line)
                .and_then(|(_score, indices)| indices.get(0).copied())
                .map(|highlight_start| (idx + start + 1, highlight_start))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_cursor_word() {
        let res = find_highlights(
            Path::new(
                "/home/xlc/.vim/plugged/vim-clap/crates/maple_core/src/highlight_cursor_word.rs",
            ),
            1,
            30,
            "line".to_string(),
        )
        .unwrap();
        println!("res: {res:?}");
    }
}
