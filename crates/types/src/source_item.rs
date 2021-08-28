use structopt::clap::arg_enum;

use pattern::{file_name_only, strip_grep_filepath, tag_name_only};

/// A tuple of match text piece (matching_text, offset_of_matching_text).
pub type MatchText<'a> = (&'a str, usize);

arg_enum! {
  #[derive(Debug, Clone)]
  pub enum MatchType {
      Full,
      TagName,
      FileName,
      IgnoreFilePath,
  }
}

impl From<String> for MatchType {
    fn from(match_type: String) -> Self {
        match_type.as_str().into()
    }
}

impl From<&String> for MatchType {
    fn from(match_type: &String) -> Self {
        match_type.as_str().into()
    }
}

impl From<&str> for MatchType {
    fn from(match_type: &str) -> Self {
        match match_type.to_lowercase().as_str() {
            "full" => Self::Full,
            "tagname" => Self::TagName,
            "filename" => Self::FileName,
            "ignorefilepath" => Self::IgnoreFilePath,
            _ => Self::Full,
        }
    }
}

/// Extracts the text for running the matcher.
pub trait MatchTextFor<'a> {
    fn match_text_for(&self, match_ty: &MatchType) -> Option<MatchText>;
}

impl<'a> MatchTextFor<'a> for SourceItem {
    fn match_text_for(&self, match_type: &MatchType) -> Option<MatchText> {
        self.match_text_for(match_type)
    }
}

impl<'a> MatchTextFor<'a> for &'a str {
    fn match_text_for(&self, _match_type: &MatchType) -> Option<MatchText> {
        Some((self, 0))
    }
}

#[derive(Debug, Clone)]
pub struct SourceItem {
    /// Raw line content of the input stream.
    pub raw: String,
    /// Text for matching.
    pub match_text: Option<(String, usize)>,
    /// The display text can be built when creating a new source item.
    pub display_text: Option<String>,
}

impl From<&str> for SourceItem {
    fn from(s: &str) -> Self {
        Self {
            raw: s.into(),
            display_text: None,
            match_text: None,
        }
    }
}

impl From<String> for SourceItem {
    fn from(raw: String) -> Self {
        Self {
            raw,
            display_text: None,
            match_text: None,
        }
    }
}

impl SourceItem {
    /// Constructs `SourceItem`.
    pub fn new(
        raw: String,
        match_text: Option<(String, usize)>,
        display_text: Option<String>,
    ) -> Self {
        Self {
            raw,
            match_text,
            display_text,
        }
    }

    pub fn display_text(&self) -> &str {
        if let Some(ref text) = self.display_text {
            text
        } else {
            self.raw.as_str()
        }
    }

    pub fn match_text(&self) -> &str {
        if let Some((ref text, _)) = self.match_text {
            text
        } else {
            self.raw.as_str()
        }
    }

    pub fn match_text_for(&self, match_ty: &MatchType) -> Option<MatchText> {
        if let Some((ref text, offset)) = self.match_text {
            return Some((text, offset));
        }
        match match_ty {
            MatchType::Full => Some((self.raw.as_str(), 0)),
            MatchType::TagName => tag_name_only(self.raw.as_str()).map(|s| (s, 0)),
            MatchType::FileName => file_name_only(self.raw.as_str()),
            MatchType::IgnoreFilePath => strip_grep_filepath(self.raw.as_str()),
        }
    }
}

/// This struct represents the filtered result of [`SourceItem`].
#[derive(Debug, Clone)]
pub struct FilteredItem<T = i64> {
    /// Tuple of (matched line text, filtering score, indices of matched elements)
    pub source_item: SourceItem,
    /// Filtering score.
    pub score: T,
    /// Indices of matched elements.
    ///
    /// The indices may be truncated when truncating the text.
    pub match_indices: Vec<usize>,
    /// The text might be truncated for fitting into the display window.
    pub display_text: Option<String>,
}

impl<T> From<(SourceItem, T, Vec<usize>)> for FilteredItem<T> {
    fn from((source_item, score, match_indices): (SourceItem, T, Vec<usize>)) -> Self {
        Self {
            source_item,
            score,
            match_indices,
            display_text: None,
        }
    }
}

impl<T> From<(String, T, Vec<usize>)> for FilteredItem<T> {
    fn from((text, score, match_indices): (String, T, Vec<usize>)) -> Self {
        Self {
            source_item: text.into(),
            score,
            match_indices,
            display_text: None,
        }
    }
}

impl<T> FilteredItem<T> {
    pub fn new<I: Into<SourceItem>>(item: I, score: T, match_indices: Vec<usize>) -> Self {
        Self {
            source_item: item.into(),
            score,
            match_indices,
            display_text: None,
        }
    }

    pub fn display_text_before_truncated(&self) -> &str {
        self.source_item.display_text()
    }

    pub fn display_text(&self) -> &str {
        if let Some(ref text) = self.display_text {
            text
        } else {
            self.source_item.display_text()
        }
    }

    /// Returns the match indices shifted by `offset`.
    pub fn shifted_indices(&self, offset: usize) -> Vec<usize> {
        self.match_indices.iter().map(|x| x + offset).collect()
    }

    pub fn deconstruct(self) -> (SourceItem, T, Vec<usize>) {
        let Self {
            source_item,
            score,
            match_indices,
            ..
        } = self;
        (source_item, score, match_indices)
    }
}
