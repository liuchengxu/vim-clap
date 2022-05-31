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
use matcher::{MatchResult, MatchScope, Matcher};

pub use self::dynamic::dyn_run;
pub use self::source::Source;
pub use matcher;
#[cfg(feature = "dyn-filtering")]
pub use subprocess;
pub use types::{CaseMatching, FilteredItem, Query, SourceItem};

/// Context for running the filter.
#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    icon: Icon,
    number: Option<usize>,
    winwidth: Option<usize>,
    matcher: Matcher,
}

impl FilterContext {
    pub fn new(
        icon: Icon,
        number: Option<usize>,
        winwidth: Option<usize>,
        matcher: Matcher,
    ) -> Self {
        Self {
            icon,
            number,
            winwidth,
            matcher,
        }
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

    pub fn match_scope(mut self, match_scope: MatchScope) -> Self {
        self.matcher = self.matcher.set_match_scope(match_scope);
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
    matcher: Matcher,
) -> Result<Vec<FilteredItem>> {
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
    let mut filtered: Vec<FilteredItem> = source_items
        .into_par_iter()
        .filter_map(|item| {
            fuzzy_matcher
                .match_query(&item, &query)
                .map(|MatchResult { score, indices }| (item, score, indices).into())
        })
        .collect();
    filtered.par_sort_unstable_by(|item1, item2| item2.score.partial_cmp(&item1.score).unwrap());
    filtered
}
