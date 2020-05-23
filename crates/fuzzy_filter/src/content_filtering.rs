use crate::{fuzzy_indices_skim, substr_indices, ScorerOutput};
use extracted_fzy::match_and_score_with_positions;
use pattern::{file_name_only, strip_grep_filepath, tag_name_only};

/// Make the arguments order same to Skim's `fuzzy_indices()`.
#[inline]
pub fn fuzzy_indices_fzy(line: &str, query: &str) -> ScorerOutput {
    match_and_score_with_positions(query, line).map(|(score, indices)| (score as i64, indices))
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
pub(super) fn apply_substr_on_grep_line(line: &str, query: &str) -> ScorerOutput {
    strip_grep_filepath(line).and_then(|(truncated_line, offset)| {
        substr_indices(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
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
pub(super) fn apply_substr_on_file_line(line: &str, query: &str) -> ScorerOutput {
    file_name_only(line).and_then(|(truncated_line, offset)| {
        substr_indices(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_skim_on_tag_line(line: &str, query: &str) -> ScorerOutput {
    tag_name_only(line).and_then(|tag_name| fuzzy_indices_skim(tag_name, query))
}

#[inline]
pub(super) fn apply_fzy_on_tag_line(line: &str, query: &str) -> ScorerOutput {
    tag_name_only(line).and_then(|tag_name| fuzzy_indices_fzy(tag_name, query))
}

#[inline]
pub(super) fn apply_substr_on_tag_line(line: &str, query: &str) -> ScorerOutput {
    tag_name_only(line).and_then(|tag_name| substr_indices(tag_name, query))
}
