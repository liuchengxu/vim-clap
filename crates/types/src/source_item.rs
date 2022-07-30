use std::borrow::Cow;

use pattern::{extract_file_name, extract_grep_pattern, extract_tag_name};

use crate::matcher::Score;

/// A tuple of match text piece (matching_text, offset_of_matching_text).
#[derive(Debug, Clone)]
pub struct FuzzyText<'a> {
    pub text: &'a str,
    pub matching_start: usize,
}

impl<'a> FuzzyText<'a> {
    pub fn new(text: &'a str, matching_start: usize) -> Self {
        Self {
            text,
            matching_start,
        }
    }
}

impl<'a> From<(&'a str, usize)> for FuzzyText<'a> {
    fn from((text, matching_start): (&'a str, usize)) -> Self {
        Self {
            text,
            matching_start,
        }
    }
}

/// The location that a match should look in.
///
/// Given a query, the match scope can refer to a full string or a substring.
#[derive(Debug, Clone, Copy)]
pub enum MatchScope {
    Full,
    /// `:Clap tags`, `:Clap proj_tags`
    TagName,
    /// `:Clap files`
    FileName,
    /// `:Clap grep2`
    GrepLine,
}

impl Default for MatchScope {
    fn default() -> Self {
        Self::Full
    }
}

impl std::str::FromStr for MatchScope {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl<T: AsRef<str>> From<T> for MatchScope {
    fn from(match_scope: T) -> Self {
        match match_scope.as_ref().to_lowercase().as_str() {
            "full" => Self::Full,
            "tagname" => Self::TagName,
            "filename" => Self::FileName,
            "grepline" => Self::GrepLine,
            _ => Self::Full,
        }
    }
}

/// This trait represents the items used in the entire filter pipeline.
pub trait ClapItem: std::fmt::Debug + Send + Sync + 'static {
    /// Initial raw text.
    fn raw_text(&self) -> &str;

    /// Text for the matching engine.
    ///
    /// Can be used to skip the leading icon, see `LineWithIcon` in `fuzzymatch-rs/src/lib.rs`.
    fn match_text(&self) -> &str {
        self.raw_text()
    }

    /// Text specifically for performing the fuzzy matching, part of the entire
    /// mathcing pipeline.
    ///
    /// The fuzzy matching process only happens when Some(_) is returned
    fn fuzzy_text(&self, match_scope: MatchScope) -> Option<FuzzyText> {
        extract_fuzzy_text(self.match_text(), match_scope)
    }

    // TODO: Each bonus can have its own range of `bonus_text`, make use of MatchScope.
    /// Text for calculating the bonus score to tweak the initial matching score.
    fn bonus_text(&self) -> &str {
        self.match_text()
    }

    /// Constructs a text intended to be displayed on the screen without any decoration (truncation,
    /// icon, etc).
    ///
    /// A concrete type of ClapItem can be structural to facilitate the matching process, in which
    /// case it's necessary to make a formatted String for displaying in the end.
    fn output_text(&self) -> Cow<'_, str> {
        self.raw_text().into()
    }
}

impl ClapItem for SourceItem {
    fn raw_text(&self) -> &str {
        &self.raw
    }

    fn fuzzy_text(&self, match_scope: MatchScope) -> Option<FuzzyText> {
        self.fuzzy_text_or_exact_using_match_scope(match_scope)
    }
}

// Impl [`ClapItem`] for raw String.
//
// In order to filter/calculate bonus for a substring instead of the whole String, a
// custom wrapper is necessary to extract the text for matching/calculating bonus/diplaying, etc.
impl<T: AsRef<str> + std::fmt::Debug + Send + Sync + 'static> ClapItem for T {
    fn raw_text(&self) -> &str {
        self.as_ref()
    }
}

/// This type represents the item for doing the filtering pipeline.
#[derive(Debug, Clone)]
pub struct SourceItem {
    /// Raw line from the initial input stream.
    pub raw: String,
    /// Text for performing the fuzzy match algorithm.
    ///
    /// Could be initialized on creating a new [`SourceItem`].
    pub fuzzy_text: Option<(String, usize)>,
    /// Text for displaying on a window with limited size.
    pub display_text: Option<String>,
}

// NOTE: do not use it when you are dealing with a large number of items.
impl From<&str> for SourceItem {
    fn from(s: &str) -> Self {
        String::from(s).into()
    }
}

impl From<String> for SourceItem {
    fn from(raw: String) -> Self {
        Self {
            raw,
            fuzzy_text: None,
            display_text: None,
        }
    }
}

impl SourceItem {
    /// Constructs a new instance of [`SourceItem`].
    pub fn new(
        raw: String,
        fuzzy_text: Option<(String, usize)>,
        display_text: Option<String>,
    ) -> Self {
        Self {
            raw,
            fuzzy_text,
            display_text,
        }
    }

    pub fn fuzzy_text_or_default(&self) -> &str {
        if let Some((ref text, _)) = self.fuzzy_text {
            text
        } else {
            &self.raw
        }
    }

    pub fn fuzzy_text_or_exact_using_match_scope(
        &self,
        match_scope: MatchScope,
    ) -> Option<FuzzyText> {
        match self.fuzzy_text {
            Some((ref text, offset)) => Some(FuzzyText::new(text, offset)),
            None => extract_fuzzy_text(self.raw.as_str(), match_scope),
        }
    }
}

pub fn extract_fuzzy_text(full: &str, match_scope: MatchScope) -> Option<FuzzyText> {
    match match_scope {
        MatchScope::Full => Some(FuzzyText::new(full, 0)),
        MatchScope::TagName => extract_tag_name(full).map(|s| FuzzyText::new(s, 0)),
        MatchScope::FileName => {
            extract_file_name(full).map(|(s, offset)| FuzzyText::new(s, offset))
        }
        MatchScope::GrepLine => {
            extract_grep_pattern(full).map(|(s, offset)| FuzzyText::new(s, offset))
        }
    }
}

/// This struct represents the filtered result of [`SourceItem`].
#[derive(Debug, Clone)]
pub struct MatchedItem {
    /// Tuple of (matched line text, filtering score, indices of matched elements)
    pub item: SourceItem,
    /// Filtering score.
    pub score: Score,
    /// Indices of matched elements.
    ///
    /// The indices may be truncated when truncating the text.
    pub indices: Vec<usize>,
    /// Text for showing the final filtered result.
    ///
    /// Usually in a truncated form for fitting into the display window.
    pub display_text: Option<String>,
}

impl MatchedItem {
    pub fn new(item: SourceItem, score: Score, indices: Vec<usize>) -> Self {
        Self {
            item,
            score,
            indices,
            display_text: None,
        }
    }

    /// Maybe truncated display text.
    pub fn display_text(&self) -> Cow<str> {
        if let Some(ref text) = self.display_text {
            text.into()
        } else {
            self.item.output_text()
        }
    }

    /// Returns the match indices shifted by `offset`.
    pub fn shifted_indices(&self, offset: usize) -> Vec<usize> {
        self.indices.iter().map(|x| x + offset).collect()
    }
}
