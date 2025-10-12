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
pub trait SearchProgressUpdate<DisplayLines> {
    /// Updates the number of searches only.
    fn quick_update(&self, total_matched: usize, total_processed: usize);

    /// Refresh the latest lines and number.
    fn update_all(
        &self,
        display_lines: &DisplayLines,
        total_matched: usize,
        total_processed: usize,
    );

    /// Invoked when the search job is complete.
    fn on_finished(
        &self,
        display_lines: DisplayLines,
        total_matched: usize,
        total_processed: usize,
    );
}

/// Separator for non-system plugin actions, `git.reload`.
pub const PLUGIN_ACTION_SEPARATOR: char = '.';

/// Plugin interfaces to users.
pub trait ClapAction {
    fn id(&self) -> &'static str;

    fn actions(&self, _action_type: ActionType) -> &[Action] {
        &[]
    }
}

#[derive(Debug, Clone)]
pub enum ActionType {
    /// Actions that users can interact with.
    Callable,
    /// Internal actions.
    Internal,
    /// All actions.
    All,
}

#[derive(Debug, Clone)]
pub struct Action {
    /// Type of this action.
    pub ty: ActionType,
    /// method used in JSONRPC request for this action.
    pub method: &'static str,
}

impl Action {
    /// Constructs a callable action with specified method.
    pub const fn callable(method: &'static str) -> Self {
        Self {
            ty: ActionType::Callable,
            method,
        }
    }

    /// Constructs an internal action with specified method.
    pub const fn internal(method: &'static str) -> Self {
        Self {
            ty: ActionType::Internal,
            method,
        }
    }
}

/// Small macro for defining an Enum with `variants()` method.
macro_rules! event_enum_with_variants {
    (
      $enum_name:ident {
        $( $variant:ident, )*
      }
    ) => {
          /// Represents a key event.
          #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
          pub enum $enum_name {
              $( $variant, )*
          }

          impl $enum_name {
              /// Returns the list of all variants in string literal.
              pub fn variants() -> &'static [&'static str] {
                  &[ $( stringify!($variant), )* ]
              }

              pub fn parse(autocmd: &str) -> Option<Self> {
                  match autocmd {
                      $( stringify!($variant) => Some(Self::$variant), )*
                      _ => None
                  }
              }
          }
    };
}

event_enum_with_variants!(AutocmdEventType {
    CursorMoved,
    InsertEnter,
    BufNewFile,
    BufEnter,
    BufLeave,
    BufDelete,
    BufWritePost,
    BufWinEnter,
    BufWinLeave,
    TextChanged,
    TextChangedI,
    FileChangedShellPost,
});
