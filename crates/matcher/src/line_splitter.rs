use crate::{
    fzy, skim::fuzzy_indices as fuzzy_indices_skim, substring::substr_indices, MatcherResult,
};
use pattern::{file_name_only, strip_grep_filepath, tag_name_only};
use structopt::clap::arg_enum;

// Implement arg_enum so that we could control it from the command line.
arg_enum! {
  /// Sometimes we hope to filter on the part of line.
  #[derive(Debug, Clone)]
  pub enum LineSplitter {
      Full,
      TagNameOnly,
      FileNameOnly,
      GrepExcludeFilePath,
  }
}

impl From<&str> for LineSplitter {
    fn from(filtering: &str) -> Self {
        match filtering {
            "Full" => Self::Full,
            "TagNameOnly" => Self::TagNameOnly,
            "FileNameOnly" => Self::FileNameOnly,
            "GrepExcludeFilePath" => Self::GrepExcludeFilePath,
            _ => Self::Full,
        }
    }
}

impl From<String> for LineSplitter {
    fn from(filtering: String) -> Self {
        Self::from(filtering.as_str())
    }
}

#[inline]
pub(super) fn apply_on_grep_line_skim(line: &str, query: &str) -> MatcherResult {
    strip_grep_filepath(line).and_then(|(truncated_line, offset)| {
        fuzzy_indices_skim(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_grep_line_fzy(line: &str, query: &str) -> MatcherResult {
    strip_grep_filepath(line).and_then(|(truncated_line, offset)| {
        fzy::fuzzy_indices(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_grep_line_substr(line: &str, query: &str) -> MatcherResult {
    strip_grep_filepath(line).and_then(|(truncated_line, offset)| {
        substr_indices(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_file_line_skim(line: &str, query: &str) -> MatcherResult {
    file_name_only(line).and_then(|(truncated_line, offset)| {
        fuzzy_indices_skim(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_file_line_fzy(line: &str, query: &str) -> MatcherResult {
    file_name_only(line).and_then(|(truncated_line, offset)| {
        fzy::fuzzy_indices(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_file_line_substr(line: &str, query: &str) -> MatcherResult {
    file_name_only(line).and_then(|(truncated_line, offset)| {
        substr_indices(truncated_line, query)
            .map(|(score, indices)| (score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_tag_line_skim(line: &str, query: &str) -> MatcherResult {
    tag_name_only(line).and_then(|tag_name| fuzzy_indices_skim(tag_name, query))
}

#[inline]
pub(super) fn apply_on_tag_line_fzy(line: &str, query: &str) -> MatcherResult {
    tag_name_only(line).and_then(|tag_name| fzy::fuzzy_indices(tag_name, query))
}

#[inline]
pub(super) fn apply_on_tag_line_substr(line: &str, query: &str) -> MatcherResult {
    tag_name_only(line).and_then(|tag_name| substr_indices(tag_name, query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclude_grep_filepath() {
        let query = "rules";
        let line = "crates/maple_cli/src/lib.rs:2:1:macro_rules! println_json {";
        let (_, origin_indices) = fzy::fuzzy_indices(line, query).unwrap();
        let (_, indices) = apply_on_grep_line_fzy(line, query).unwrap();
        assert_eq!(origin_indices, indices);
    }

    #[test]
    fn test_file_name_only() {
        let query = "lib";
        let line = "crates/extracted_fzy/src/lib.rs";
        let (_, origin_indices) = fzy::fuzzy_indices(line, query).unwrap();
        let (_, indices) = apply_on_file_line_fzy(line, query).unwrap();
        assert_eq!(origin_indices, indices);
    }
}
