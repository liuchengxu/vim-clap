mod matcher;
mod query;
mod search_term;
mod source_item;

pub use self::matcher::{parse_criteria, MatchResult, Rank, RankCalculator, RankCriterion, Score};
pub use self::query::Query;
pub use self::search_term::{
    ExactTerm, ExactTermType, FuzzyTerm, FuzzyTermType, InverseTerm, InverseTermType, SearchTerm,
    TermType, WordTerm,
};
pub use self::source_item::{
    extract_fuzzy_text, AsAny, ClapItem, FileNameItem, FuzzyText, GrepItem, MatchScope,
    MatchedItem, SourceItem,
};

#[derive(Clone, Copy, Debug, Default)]
pub enum CaseMatching {
    Ignore,
    Respect,
    #[default]
    Smart,
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

/// Show the filtering progress.
pub trait ProgressUpdate<DisplayLines> {
    fn update_brief(&self, total_matched: usize, total_processed: usize);

    fn update_all(
        &self,
        display_lines: &DisplayLines,
        total_matched: usize,
        total_processed: usize,
    );

    fn on_finished(
        &self,
        display_lines: DisplayLines,
        total_matched: usize,
        total_processed: usize,
    );
}
