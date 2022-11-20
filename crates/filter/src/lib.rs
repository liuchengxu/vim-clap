//! This crate provides the feature of filtering a stream of lines.
//!
//! Given a stream of lines:
//!
//! 1. apply the matcher algorithm on each of them.
//! 2. sort the all lines with a match result.
//! 3. print the top rated filtered lines to stdout.

mod parallel_worker;
mod sequential_source;
mod sequential_worker;

use anyhow::Result;
use icon::Icon;
use matcher::{Bonus, ClapItem, MatchScope, Matcher};
use rayon::prelude::*;
use std::sync::Arc;
use types::{FileNameItem, GrepItem};

pub use self::parallel_worker::{
    par_dyn_run, par_dyn_run_inprocess, par_dyn_run_list, ParallelSource, StdioProgressor,
};
pub use self::sequential_source::{filter_sequential, SequentialSource};
pub use self::sequential_worker::dyn_run;
pub use matcher;
pub use types::{CaseMatching, MatchedItem, Query, SourceItem};

/// Converts the raw line into a clap item.
pub(crate) fn to_clap_item(match_scope: MatchScope, line: String) -> Option<Arc<dyn ClapItem>> {
    match match_scope {
        MatchScope::GrepLine => {
            GrepItem::try_new(line).map(|item| Arc::new(item) as Arc<dyn ClapItem>)
        }
        MatchScope::FileName => {
            FileNameItem::try_new(line).map(|item| Arc::new(item) as Arc<dyn ClapItem>)
        }
        _ => Some(Arc::new(SourceItem::from(line))),
    }
}

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

    pub fn bonuses(mut self, bonuses: Vec<Bonus>) -> Self {
        self.matcher = self.matcher.set_bonuses(bonuses);
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
    query: &str,
    source: SequentialSource<I>,
    matcher: Matcher,
) -> Result<Vec<MatchedItem>> {
    let query: Query = query.into();
    let matched_items = filter_sequential(source, matcher, &query)?;
    Ok(sort_matched_items(matched_items))
}

/// Performs the synchorous filtering on a small scale of source in parallel.
pub fn par_filter(
    query: impl Into<Query>,
    source_items: Vec<SourceItem>,
    fuzzy_matcher: &Matcher,
) -> Vec<MatchedItem> {
    let query: Query = query.into();
    let matched_items = source_items
        .into_par_iter()
        .filter_map(|item| {
            let item: Arc<dyn ClapItem> = Arc::new(item);
            fuzzy_matcher.match_item(item, &query)
        })
        .collect::<Vec<_>>();
    sort_matched_items(matched_items)
}

/// Performs the synchorous filtering on a small scale of source in parallel.
pub fn par_filter_items(
    query: impl Into<Query>,
    source_items: &[Arc<dyn ClapItem>],
    fuzzy_matcher: &Matcher,
) -> Vec<MatchedItem> {
    let query: Query = query.into();
    let matched_items = source_items
        .into_par_iter()
        .filter_map(|item| fuzzy_matcher.match_item(item.clone(), &query))
        .collect::<Vec<_>>();
    sort_matched_items(matched_items)
}
