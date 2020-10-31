use crate::{
    fzy, skim::fuzzy_indices as fuzzy_indices_skim, substring::substr_indices,
};
use types::{FilterResult, SourceItem};
use pattern::{file_name_only, strip_grep_filepath};
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

// XXX: avoid cloning everywhere below. Cloning is bad.

#[inline]
pub(super) fn apply_on_grep_line_skim(item: SourceItem, query: &str) -> Option<FilterResult> {
    strip_grep_filepath(&item.display.clone()).and_then(|(truncated_line, offset)| {
        fuzzy_indices_skim(truncated_line, query)
            .map(|(score, indices)| (item, score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_grep_line_fzy(item: SourceItem, query: &str) -> Option<FilterResult> {
    strip_grep_filepath(&item.display.clone()).and_then(|(truncated_line, offset)| {
        fzy::fuzzy_indices(truncated_line, query)
            .map(|(score, indices)| (item, score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_grep_line_substr(item: SourceItem, query: &str) -> Option<FilterResult> {
    strip_grep_filepath(&item.display.clone()).and_then(|(truncated_line, offset)| {
        substr_indices(truncated_line, query)
            .map(|(score, indices)| (item, score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_file_line_skim(item: SourceItem, query: &str) -> Option<FilterResult> {
    file_name_only(&item.display.clone()).and_then(|(truncated_line, offset)| {
        fuzzy_indices_skim(truncated_line, query)
            .map(|(score, indices)| (item, score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_file_line_fzy(item: SourceItem, query: &str) -> Option<FilterResult> {
    file_name_only(&item.display.clone()).and_then(|(truncated_line, offset)| {
        fzy::fuzzy_indices(truncated_line, query)
            .map(|(score, indices)| (item, score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_file_line_substr(item: SourceItem, query: &str) -> Option<FilterResult> {
    file_name_only(&item.display.clone()).and_then(|(truncated_line, offset)| {
        substr_indices(truncated_line, query)
            .map(|(score, indices)| (item, score, indices.into_iter().map(|x| x + offset).collect()))
    })
}

#[inline]
pub(super) fn apply_on_tag_line_skim(item: SourceItem, query: &str) -> Option<FilterResult> {
    fuzzy_indices_skim(&item.filter.clone().unwrap(), query)
      .map(|(score, indices)| (item, score, indices))
}

#[inline]
pub(super) fn apply_on_tag_line_fzy(item: SourceItem, query: &str) -> Option<FilterResult> {
    fzy::fuzzy_indices(&item.filter.clone().unwrap(), query)
      .map(|(score, indices)| (item, score, indices))
}

#[inline]
pub(super) fn apply_on_tag_line_substr(item: SourceItem, query: &str) -> Option<FilterResult> {
    substr_indices(&item.filter.clone().unwrap(), query)
      .map(|(score, indices)| (item, score, indices)) 
}

#[inline]
pub(super) fn apply_direct_skim(item: SourceItem, query: &str) -> Option<FilterResult> {
    fuzzy_indices_skim(&item.display, query)
      .map(|(score, indices)| (item, score, indices))
}

#[inline]
pub(super) fn apply_direct_fzy(item: SourceItem, query: &str) -> Option<FilterResult> {
    fzy::fuzzy_indices(&item.display, query)
      .map(|(score, indices)| (item, score, indices))
}

#[inline]
pub(super) fn apply_direct_substr(item: SourceItem, query: &str) -> Option<FilterResult> {
    substr_indices(&item.display, query)
      .map(|(score, indices)| (item, score, indices))
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
