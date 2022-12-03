//! This crate provides the feature of filtering a stream of lines.
//!
//! Given a stream of lines:
//!
//! 1. apply the matcher algorithm on each of them.
//! 2. sort the all lines with a match result.
//! 3. print the top rated filtered lines to stdout.

mod source;
mod worker;

use std::sync::Arc;

use anyhow::Result;
use rayon::prelude::*;

use icon::Icon;
use matcher::{Bonus, ClapItem, MatchScope, Matcher, MatcherBuilder};

pub use self::source::Source;
pub use self::worker::iterator::dyn_run;
pub use self::worker::par_iterator::{par_dyn_run, par_dyn_run_list, ParSource};
pub use matcher;
pub use types::{CaseMatching, MatchedItem, Query, SourceItem};

/// Context for running the filter.
#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    icon: Icon,
    number: Option<usize>,
    winwidth: Option<usize>,
    matcher_builder: MatcherBuilder,
}

impl FilterContext {
    pub fn new(
        icon: Icon,
        number: Option<usize>,
        winwidth: Option<usize>,
        matcher_builder: MatcherBuilder,
    ) -> Self {
        Self {
            icon,
            number,
            winwidth,
            matcher_builder,
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
        self.matcher_builder = self.matcher_builder.match_scope(match_scope);
        self
    }

    pub fn bonuses(mut self, bonuses: Vec<Bonus>) -> Self {
        self.matcher_builder = self.matcher_builder.bonuses(bonuses);
        self
    }
}

/// Sorts the filtered result by the filter score.
///
/// The item with highest score first, the item with lowest score last.
pub fn sort_matched_items(matched_items: Vec<MatchedItem>) -> Vec<MatchedItem> {
    let mut items = matched_items;
    items.par_sort_unstable_by(|v1, v2| v2.score.partial_cmp(&v1.score).unwrap());
    items
}

/// Returns the ranked results after applying the matcher algo
/// given the query String and filtering source.
pub fn sync_run<I: Iterator<Item = Arc<dyn ClapItem>>>(
    source: Source<I>,
    matcher: Matcher,
) -> Result<Vec<MatchedItem>> {
    let matched_items = source.run_and_collect(matcher)?;
    Ok(sort_matched_items(matched_items))
}

/// Performs the synchorous filtering on a small scale of source in parallel.
pub fn par_filter(source_items: Vec<SourceItem>, fuzzy_matcher: &Matcher) -> Vec<MatchedItem> {
    let matched_items = source_items
        .into_par_iter()
        .filter_map(|item| {
            let item: Arc<dyn ClapItem> = Arc::new(item);
            fuzzy_matcher.match_item(item)
        })
        .collect::<Vec<_>>();
    sort_matched_items(matched_items)
}

/// Performs the synchorous filtering on a small scale of source in parallel.
pub fn par_filter_items(
    source_items: &[Arc<dyn ClapItem>],
    fuzzy_matcher: &Matcher,
) -> Vec<MatchedItem> {
    let matched_items = source_items
        .into_par_iter()
        .filter_map(|item| fuzzy_matcher.match_item(item.clone()))
        .collect::<Vec<_>>();
    sort_matched_items(matched_items)
}
