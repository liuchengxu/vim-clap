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

#[derive(Debug, Clone)]
pub struct SourceItem {
    pub raw: String,
    pub match_text: Option<(String, usize)>,
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
        raw.as_str().into()
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
            display_text,
            match_text,
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
