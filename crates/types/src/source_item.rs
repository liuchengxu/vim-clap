use std::any::Any;
use std::sync::Arc;

use pattern::{extract_file_name, extract_grep_pattern, extract_tag_name};

pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

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
pub trait ClapItem: AsAny + std::fmt::Debug + Send + Sync + 'static {
    /// Initial raw text.
    fn raw_text(&self) -> &str;

    /// Text for the matching engine.
    ///
    /// Can be used to skip the leading icon.
    fn match_text(&self) -> &str {
        self.raw_text()
    }

    // TODO: replace with `fuzzy_text(&self) -> Option<FuzzyText>` and add new variant accordingly.
    // - MultiSourceItem(Full)
    // - TagNameItem
    // - FileNameItem
    // - GrepLine
    /// Text specifically for performing the fuzzy matching, part of the entire
    /// mathcing pipeline.
    ///
    /// The fuzzy matching process only happens when Some(_) is returned.
    fn fuzzy_text(&self, match_scope: &MatchScope) -> Option<FuzzyText>;

    // TODO: Each bonus can have its own range of `bonus_text`, make use of MatchScope.
    /// Text for calculating the bonus score.
    fn bonus_text(&self) -> &str {
        self.match_text()
    }

    fn display_text(&self) -> &str {
        self.raw_text()
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

    fn fuzzy_text(&self, _match_scope: &MatchScope) -> Option<FuzzyText> {
        Some(FuzzyText {
            text: self.as_ref(),
            matching_start: 0,
        })
    }
}

// TODO: Deprecate MultiSourceItem with various wrappers:
// - FullItem
// - BLinesItem
// - GrepLineItem
// - FileNameItem
impl ClapItem for MultiSourceItem {
    fn raw_text(&self) -> &str {
        &self.raw
    }

    fn fuzzy_text(&self, match_scope: &MatchScope) -> Option<FuzzyText> {
        self.fuzzy_text_or_exact_using_match_scope(match_scope)
    }

    fn display_text(&self) -> &str {
        self.display_text_or_raw()
    }
}

/// This type represents the item for doing the filtering pipeline.
#[derive(Debug, Clone)]
pub struct MultiSourceItem {
    /// Raw line from the initial input stream.
    pub raw: String,
    /// Text for performing the fuzzy match algorithm.
    ///
    /// Could be initialized on creating a new [`MultiSourceItem`].
    pub fuzzy_text: Option<(String, usize)>,
    /// Text for displaying on a window with limited size.
    pub display_text: Option<String>,
}

impl From<String> for MultiSourceItem {
    fn from(raw: String) -> Self {
        Self {
            raw,
            fuzzy_text: None,
            display_text: None,
        }
    }
}

impl MultiSourceItem {
    /// Constructs a new instance of [`MultiSourceItem`].
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

    pub fn display_text_or_raw(&self) -> &str {
        match self.display_text {
            Some(ref text) => text,
            None => &self.raw,
        }
    }

    pub fn fuzzy_text_or_exact_using_match_scope(
        &self,
        match_scope: &MatchScope,
    ) -> Option<FuzzyText> {
        match self.fuzzy_text {
            Some((ref text, offset)) => Some(FuzzyText::new(text, offset)),
            None => extract_fuzzy_text(self.raw.as_str(), match_scope),
        }
    }
}

pub fn extract_fuzzy_text<'a>(full: &'a str, match_scope: &MatchScope) -> Option<FuzzyText<'a>> {
    match match_scope {
        MatchScope::Full => Some(FuzzyText::new(full, 0)),
        MatchScope::TagName => extract_tag_name(full).map(|s| FuzzyText::new(s, 0)),
        MatchScope::FileName => extract_file_name(full).map(Into::into),
        MatchScope::GrepLine => extract_grep_pattern(full).map(Into::into),
    }
}

/// This struct represents the filtered result of [`MultiSourceItem`].
#[derive(Debug, Clone)]
pub struct MatchedItem<T = i64> {
    /// Tuple of (matched line text, filtering score, indices of matched elements)
    pub item: Arc<dyn ClapItem>,
    /// Filtering score.
    pub score: T,
    /// Indices of matched elements.
    ///
    /// The indices may be truncated when truncating the text.
    pub match_indices: Vec<usize>,
    /// Text for showing the final filtered result.
    ///
    /// Usually in a truncated form for fitting into the display window.
    pub display_text: Option<String>,
}

impl<T> MatchedItem<T> {
    pub fn new(item: Arc<dyn ClapItem>, score: T, match_indices: Vec<usize>) -> Self {
        Self {
            item,
            score,
            match_indices,
            display_text: None,
        }
    }

    /// Untruncated display text.
    pub fn source_item_display_text(&self) -> &str {
        self.item.display_text()
    }

    /// Maybe truncated display text.
    pub fn display_text(&self) -> &str {
        if let Some(ref text) = self.display_text {
            text
        } else {
            self.item.display_text()
        }
    }

    /// Returns the match indices shifted by `offset`.
    pub fn shifted_indices(&self, offset: usize) -> Vec<usize> {
        self.match_indices.iter().map(|x| x + offset).collect()
    }
}
