use crate::{fuzzy_indices_skim, ScorerOutput};
use extracted_fzy::match_and_score_with_positions;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
  // match the file path and line number of grep line.
  static ref GREP_RE: Regex = Regex::new(r"^.*:\d+:\d+:").unwrap();
  // match the tag_name:lnum of tag line.
  static ref TAG_RE: Regex = Regex::new(r"^(.*:\d+)").unwrap();
}

/// Make the arguments order same to Skim's `fuzzy_indices()`.
#[inline]
pub fn fuzzy_indices_fzy(line: &str, query: &str) -> ScorerOutput {
    match_and_score_with_positions(query, line).map(|(score, indices)| (score as i64, indices))
}

/// Do not match the file path when using ripgrep.
#[inline]
fn strip_grep_filepath(line: &str) -> Option<(&str, usize)> {
    GREP_RE
        .find(line)
        .map(|mat| (&line[mat.end()..], mat.end()))
}

#[inline]
pub(super) fn apply_skim_on_grep_line(line: &str, query: &str) -> ScorerOutput {
    strip_grep_filepath(line).and_then(|(truncated_line, offset)| {
        fuzzy_indices_skim(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_fzy_on_grep_line(line: &str, query: &str) -> ScorerOutput {
    strip_grep_filepath(line).and_then(|(truncated_line, offset)| {
        fuzzy_indices_fzy(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
fn file_name_only(line: &str) -> Option<(&str, usize)> {
    let fpath: std::path::PathBuf = line.into();
    fpath
        .file_name()
        .map(|x| x.to_string_lossy().into_owned())
        .map(|fname| (&line[line.len() - fname.len()..], line.len() - fname.len()))
}

#[inline]
pub(super) fn apply_skim_on_file_line(line: &str, query: &str) -> ScorerOutput {
    file_name_only(line).and_then(|(truncated_line, offset)| {
        fuzzy_indices_skim(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_fzy_on_file_line(line: &str, query: &str) -> ScorerOutput {
    file_name_only(line).and_then(|(truncated_line, offset)| {
        fuzzy_indices_fzy(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
fn tag_name_only(line: &str) -> Option<&str> {
    TAG_RE.find(line).map(|x| x.as_str())
}

#[inline]
pub(super) fn apply_skim_on_tag_line(line: &str, query: &str) -> ScorerOutput {
    tag_name_only(line).and_then(|tag_name| fuzzy_indices_skim(tag_name, query))
}

#[inline]
pub(super) fn apply_fzy_on_tag_line(line: &str, query: &str) -> ScorerOutput {
    tag_name_only(line).and_then(|tag_name| fuzzy_indices_fzy(tag_name, query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclude_grep_filepath() {
        let query = "rules";
        let line = "crates/maple_cli/src/lib.rs:2:1:macro_rules! println_json {";
        let (_, origin_indices) = fuzzy_indices_fzy(line, query).unwrap();
        let (_, indices) = apply_fzy_on_grep_line(line, query).unwrap();
        assert_eq!(origin_indices, indices);
    }

    #[test]
    fn test_file_name_only() {
        let query = "lib";
        let line = "crates/extracted_fzy/src/lib.rs";
        let (_, origin_indices) = fuzzy_indices_fzy(line, query).unwrap();
        let (_, indices) = apply_fzy_on_file_line(line, query).unwrap();
        assert_eq!(origin_indices, indices);
    }

    #[test]
    fn test_tag_name_only() {
        let line = "<Backspace>:60       [map]           inoremap <silent> <buffer> <Backspace> <C-R>=clap#handler#bs_action()<CR>  ftplugin/clap_input.vim";
        let mat = TAG_RE.find(line);
        assert_eq!(mat.unwrap().as_str(), "<Backspace>:60");
    }
}
