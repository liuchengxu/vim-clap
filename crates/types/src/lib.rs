mod query;
mod search_term;
mod source_item;

pub use self::query::Query;
pub use self::search_term::{
    ExactTerm, ExactTermType, FuzzyTerm, FuzzyTermType, InverseTerm, InverseTermType, SearchTerm,
    TermType,
};
pub use self::source_item::{FilteredItem, FuzzyText, MatchingText, MatchingTextKind, SourceItem};

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
