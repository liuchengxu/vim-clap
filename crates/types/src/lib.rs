mod matcher;
mod query;
mod search_term;
mod source_item;

pub use self::matcher::{MatchResult, Score};
pub use self::query::Query;
pub use self::search_term::{
    ExactTerm, ExactTermType, FuzzyTerm, FuzzyTermType, InverseTerm, InverseTermType, SearchTerm,
    TermType,
};
pub use self::source_item::{
    extract_fuzzy_text, AsAny, ClapItem, FuzzyText, MatchScope, MatchedItem, MultiSourceItem,
};

/// The preview content is usually part of a file.
#[derive(Clone, Debug)]
pub struct PreviewInfo {
    pub start: usize,
    pub end: usize,
    /// Line number of the line that should be highlighed in the preview window.
    pub highlight_lnum: usize,
    /// [start, end] of the source file.
    pub lines: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum CaseMatching {
    Ignore,
    Respect,
    Smart,
}

impl Default for CaseMatching {
    fn default() -> Self {
        Self::Smart
    }
}

impl std::str::FromStr for CaseMatching {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.into())
    }
}

impl<T: AsRef<str>> From<T> for CaseMatching {
    fn from(case_matching: T) -> Self {
        match case_matching.as_ref().to_lowercase().as_str() {
            "ignore" => Self::Ignore,
            "respect" => Self::Respect,
            _ => Self::Smart,
        }
    }
}

impl CaseMatching {
    pub fn is_case_sensitive(&self, query: &str) -> bool {
        match self {
            Self::Ignore => false,
            Self::Respect => true,
            Self::Smart => query.chars().any(|c| c.is_uppercase()),
        }
    }
}
