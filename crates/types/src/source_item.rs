use pattern::{file_name_only, strip_grep_filepath, tag_name_only};

/// A tuple of match text piece (matching_text, offset_of_matching_text).
pub type FuzzyText<'a> = (&'a str, usize);

#[derive(Debug, Clone, Copy)]
pub enum MatchType {
    Full,
    TagName,
    FileName,
    IgnoreFilePath,
}

impl std::str::FromStr for MatchType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl<T: AsRef<str>> From<T> for MatchType {
    fn from(match_type: T) -> Self {
        match match_type.as_ref().to_lowercase().as_str() {
            "full" => Self::Full,
            "tagname" => Self::TagName,
            "filename" => Self::FileName,
            "ignorefilepath" => Self::IgnoreFilePath,
            _ => Self::Full,
        }
    }
}

/// Text used in the matching algorithm.
pub trait MatchingText<'a> {
    /// Initial full text.
    fn full_text(&self) -> &str;

    /// Text for calculating the bonus score.
    fn bonus_text(&self) -> &str {
        self.full_text()
    }

    /// Text for applying the fuzzy match algorithm.
    ///
    /// The fuzzy matching process only happens when Some(_) is returned.
    fn fuzzy_text(&self, match_ty: &MatchType) -> Option<FuzzyText>;
}

impl<'a> MatchingText<'a> for SourceItem {
    fn full_text(&self) -> &str {
        &self.raw
    }

    fn fuzzy_text(&self, match_type: &MatchType) -> Option<FuzzyText> {
        self.get_fuzzy_text(match_type)
    }
}

impl<'a> MatchingText<'a> for &'a str {
    fn full_text(&self) -> &str {
        self
    }

    fn fuzzy_text(&self, _match_type: &MatchType) -> Option<FuzzyText> {
        Some((self, 0))
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

    pub fn display_text(&self) -> &str {
        if let Some(ref text) = self.display_text {
            text
        } else {
            &self.raw
        }
    }

    pub fn fuzzy_text_or_default(&self) -> &str {
        if let Some((ref text, _)) = self.fuzzy_text {
            text
        } else {
            &self.raw
        }
    }

    pub fn get_fuzzy_text(&self, match_ty: &MatchType) -> Option<FuzzyText> {
        if let Some((ref text, offset)) = self.fuzzy_text {
            return Some((text, offset));
        }
        match match_ty {
            MatchType::Full => Some((&self.raw, 0)),
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
    /// Text for showing the final filtered result.
    ///
    /// Usually in a truncated form for fitting into the display window.
    pub display_text: Option<String>,
}

impl<I: Into<SourceItem>, T> From<(I, T, Vec<usize>)> for FilteredItem<T> {
    fn from((item, score, match_indices): (I, T, Vec<usize>)) -> Self {
        Self {
            source_item: item.into(),
            score,
            match_indices,
            display_text: None,
        }
    }
}

impl<I: Into<SourceItem>, T: Default> From<I> for FilteredItem<T> {
    fn from(item: I) -> Self {
        Self {
            source_item: item.into(),
            score: Default::default(),
            match_indices: Default::default(),
            display_text: None,
        }
    }
}

impl<T> FilteredItem<T> {
    pub fn new<I: Into<SourceItem>>(item: I, score: T, match_indices: Vec<usize>) -> Self {
        (item, score, match_indices).into()
    }

    /// Untruncated display text.
    pub fn source_item_display_text(&self) -> &str {
        self.source_item.display_text()
    }

    /// Maybe truncated display text.
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
}
