use crate::matcher::{MatchResult, Rank};
use icon::Icon;
use pattern::{extract_file_name, extract_grep_pattern, extract_tag_name};
use std::any::Any;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::sync::Arc;

/// Helper trait to convert Arc<dyn ClapItem> to the original concrete type.
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

/// The location that a match should look in.
///
/// Given a query, the match scope can refer to a full string or a substring.
#[derive(Default, Debug, Clone, Copy)]
pub enum MatchScope {
    #[default]
    Full,
    /// `:Clap tags`, `:Clap proj_tags`
    TagName,
    /// `:Clap files`
    FileName,
    /// `:Clap grep`
    GrepLine,
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
pub trait ClapItem: AsAny + std::fmt::Debug + Send + Sync {
    /// Initial raw text.
    fn raw_text(&self) -> &str;

    /// Text for the matching engine.
    ///
    /// Can be used to skip the leading icon, see `LineWithIcon` in `fuzzymatch-rs/src/lib.rs`.
    fn match_text(&self) -> &str {
        self.raw_text()
    }

    /// Text specifically for performing the fuzzy matching, part of the entire
    /// matching pipeline.
    ///
    /// The fuzzy matching process only happens when Some(_) is returned.
    fn fuzzy_text(&self, match_scope: MatchScope) -> Option<FuzzyText<'_>> {
        extract_fuzzy_text(self.match_text(), match_scope)
    }

    // TODO: Each bonus can have its own range of `bonus_text`, make use of MatchScope.
    /// Text for calculating the bonus score to tweak the initial matching score.
    fn bonus_text(&self) -> &str {
        self.match_text()
    }

    /// Callback for the result of `matcher::match_item`.
    ///
    /// Sometimes we need to tweak the indices of matched item for custom output text, e.g.,
    /// `BlinesItem`.
    fn match_result_callback(&self, match_result: MatchResult) -> MatchResult {
        match_result
    }

    /// Constructs a text intended to be displayed on the screen without any decoration (truncation,
    /// icon, etc).
    ///
    /// A concrete type of ClapItem can be structural to facilitate the matching process, in which
    /// case it's necessary to make a formatted String for displaying in the end.
    fn output_text(&self) -> Cow<'_, str> {
        self.raw_text().into()
    }

    /// Returns the icon if enabled and possible.
    fn icon(&self, icon: icon::Icon) -> Option<icon::IconType> {
        icon.icon_kind()
            .map(|icon_kind| icon_kind.icon(&self.output_text()))
    }

    /// Offset in chars for the truncation.
    ///
    /// Used by `blines` to not strip out the line_number during the truncation.
    fn truncation_offset(&self) -> Option<usize> {
        None
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

#[derive(Debug, Clone)]
pub struct GrepItem {
    raw: String,
    end_of_path: usize,
    start_of_line: usize,
}

impl GrepItem {
    pub fn try_new(raw: String) -> Option<Self> {
        let (end_of_path, start_of_line) = pattern::parse_grep_item(&raw)?;
        Some(Self {
            raw,
            end_of_path,
            start_of_line,
        })
    }

    fn file_path(&self) -> &str {
        &self.raw[..self.end_of_path]
    }

    fn line(&self) -> &str {
        &self.raw[self.start_of_line..]
    }
}

impl ClapItem for GrepItem {
    fn raw_text(&self) -> &str {
        &self.raw
    }

    fn fuzzy_text(&self, _match_scope: MatchScope) -> Option<FuzzyText<'_>> {
        Some(FuzzyText::new(self.line(), self.start_of_line))
    }

    fn bonus_text(&self) -> &str {
        self.line()
    }

    fn icon(&self, _icon: Icon) -> Option<icon::IconType> {
        Some(icon::file_icon(self.file_path()))
    }
}

/// Item of `:Clap files`, but only matches the file name instead of the entire file path.
#[derive(Debug, Clone)]
pub struct FileNameItem {
    raw: String,
    file_name_offset: usize,
}

impl FileNameItem {
    pub fn try_new(raw: String) -> Option<Self> {
        let (_file_name, file_name_offset) = pattern::extract_file_name(&raw)?;
        Some(Self {
            raw,
            file_name_offset,
        })
    }

    fn file_name(&self) -> &str {
        &self.raw[self.file_name_offset..]
    }
}

impl ClapItem for FileNameItem {
    fn raw_text(&self) -> &str {
        &self.raw
    }

    fn fuzzy_text(&self, _match_scope: MatchScope) -> Option<FuzzyText<'_>> {
        Some(FuzzyText::new(self.file_name(), self.file_name_offset))
    }

    fn icon(&self, _icon: Icon) -> Option<icon::IconType> {
        Some(icon::file_icon(&self.raw))
    }
}

/// This type represents multiple kinds of concrete Clap item from providers like grep,
/// proj_tags, files, etc.
#[derive(Debug, Clone)]
pub struct SourceItem {
    /// Raw line from the initial input stream.
    pub raw: String,
    /// Text for performing the fuzzy match algorithm.
    ///
    /// Could be initialized on creating a new [`SourceItem`].
    pub fuzzy_text: Option<(String, usize)>,
    /// Text for displaying.
    pub output_text: Option<String>,
}

impl From<String> for SourceItem {
    fn from(raw: String) -> Self {
        Self {
            raw,
            fuzzy_text: None,
            output_text: None,
        }
    }
}

impl SourceItem {
    /// Constructs a new instance of [`SourceItem`].
    pub fn new(
        raw: String,
        fuzzy_text: Option<(String, usize)>,
        output_text: Option<String>,
    ) -> Self {
        Self {
            raw,
            fuzzy_text,
            output_text,
        }
    }

    pub fn output_text_or_raw(&self) -> &str {
        match self.output_text {
            Some(ref text) => text,
            None => &self.raw,
        }
    }

    pub fn fuzzy_text_or_exact_using_match_scope(
        &self,
        match_scope: MatchScope,
    ) -> Option<FuzzyText<'_>> {
        match self.fuzzy_text {
            Some((ref text, offset)) => Some(FuzzyText::new(text, offset)),
            None => extract_fuzzy_text(self.raw.as_str(), match_scope),
        }
    }
}

impl ClapItem for SourceItem {
    fn raw_text(&self) -> &str {
        &self.raw
    }

    fn fuzzy_text(&self, match_scope: MatchScope) -> Option<FuzzyText<'_>> {
        self.fuzzy_text_or_exact_using_match_scope(match_scope)
    }

    fn output_text(&self) -> Cow<'_, str> {
        self.output_text_or_raw().into()
    }
}

pub fn extract_fuzzy_text(full: &str, match_scope: MatchScope) -> Option<FuzzyText<'_>> {
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
    pub item: Arc<dyn ClapItem>,
    /// Item rank.
    pub rank: Rank,
    /// Indices of matched elements.
    ///
    /// The indices may be truncated when truncating the text.
    pub indices: Vec<usize>,
    /// Text for showing the final filtered result.
    ///
    /// Usually in a truncated form for fitting into the display window.
    pub display_text: Option<String>,
    /// Untruncated display text.
    pub output_text: Option<String>,
}

impl PartialEq for MatchedItem {
    fn eq(&self, other: &Self) -> bool {
        self.rank.eq(&other.rank)
    }
}

impl Eq for MatchedItem {}

impl Ord for MatchedItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank.cmp(&other.rank)
    }
}

impl PartialOrd for MatchedItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Arc<dyn ClapItem>> for MatchedItem {
    fn from(item: Arc<dyn ClapItem>) -> Self {
        Self {
            item,
            rank: Rank::default(),
            indices: Vec::new(),
            display_text: None,
            output_text: None,
        }
    }
}

impl MatchedItem {
    pub fn new(item: Arc<dyn ClapItem>, rank: Rank, indices: Vec<usize>) -> Self {
        Self {
            item,
            rank,
            indices,
            display_text: None,
            output_text: None,
        }
    }

    /// Maybe truncated display text.
    pub fn display_text(&self) -> Cow<'_, str> {
        self.display_text
            .as_ref()
            .map(Into::into)
            .unwrap_or_else(|| self.item.output_text())
    }

    pub fn output_text(&self) -> Cow<'_, str> {
        self.output_text
            .as_ref()
            .map(Into::into)
            .unwrap_or_else(|| self.item.output_text())
    }

    /// Returns the match indices shifted by `offset`.
    pub fn shifted_indices(&self, offset: usize) -> Vec<usize> {
        self.indices.iter().map(|x| x + offset).collect()
    }
}
