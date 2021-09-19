//! This crate provides the feature of filtering a stream of lines.
//!
//! Given a stream of lines:
//!
//! 1. apply the matcher algorithm on each of them.
//! 2. sort the all lines with a match result.
//! 3. print the top rated filtered lines to stdout.

mod dynamic;
mod source;

use anyhow::Result;
use rayon::prelude::*;

use icon::Icon;
use matcher::{Bonus, FuzzyAlgorithm, MatchType, Matcher};

pub use self::dynamic::dyn_run;
pub use self::source::Source;
pub use matcher;
#[cfg(feature = "enable_dyn")]
pub use subprocess;
pub use types::{FilteredItem, Query, SourceItem};

/// Context for running the filter.
#[derive(Debug, Clone)]
pub struct FilterContext {
    algo: FuzzyAlgorithm,
    icon: Icon,
    number: Option<usize>,
    winwidth: Option<usize>,
    match_type: MatchType,
}

impl Default for FilterContext {
    fn default() -> Self {
        Self {
            algo: Default::default(),
            icon: Default::default(),
            number: None,
            winwidth: None,
            match_type: MatchType::Full,
        }
    }
}

impl FilterContext {
    pub fn new(
        algo: FuzzyAlgorithm,
        icon: Icon,
        number: Option<usize>,
        winwidth: Option<usize>,
        match_type: MatchType,
    ) -> Self {
        Self {
            algo,
            icon,
            number,
            winwidth,
            match_type,
        }
    }

    pub fn algo(mut self, algo: FuzzyAlgorithm) -> Self {
        self.algo = algo;
        self
    }

    pub fn number(mut self, number: Option<usize>) -> Self {
        self.number = number;
        self
    }

    pub fn winwidth(mut self, winwidth: Option<usize>) -> Self {
        self.winwidth = winwidth;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = icon;
        self
    }

    pub fn match_type(mut self, match_type: MatchType) -> Self {
        self.match_type = match_type;
        self
    }
}

/// Sorts the filtered result by the filter score.
///
/// The item with highest score first, the item with lowest score last.
pub fn sort_initial_filtered(filtered: Vec<FilteredItem>) -> Vec<FilteredItem> {
    let mut filtered = filtered;
    filtered.par_sort_unstable_by(|v1, v2| v2.score.partial_cmp(&v1.score).unwrap());
    filtered
}

/// Returns the ranked results after applying the matcher algo
/// given the query String and filtering source.
pub fn sync_run<I: Iterator<Item = SourceItem>>(
    query: &str,
    source: Source<I>,
    algo: FuzzyAlgorithm,
    match_type: MatchType,
    bonuses: Vec<Bonus>,
) -> Result<Vec<FilteredItem>> {
    let matcher = Matcher::with_bonuses(algo, match_type, bonuses);
    let query: Query = query.into();
    let filtered = source.filter_and_collect(matcher, &query)?;
    let ranked = sort_initial_filtered(filtered);
    Ok(ranked)
}

/// Performs the synchorous filtering on a small scale of source in parallel.
pub fn par_filter(
    query: impl Into<Query>,
    source_items: Vec<SourceItem>,
    fuzzy_matcher: &Matcher,
) -> Vec<FilteredItem> {
    let query: Query = query.into();
    let mut filtered = source::par_filter_impl(source_items, fuzzy_matcher, &query);
    filtered.par_sort_unstable_by(|item1, item2| item2.score.partial_cmp(&item1.score).unwrap());
    filtered
}
