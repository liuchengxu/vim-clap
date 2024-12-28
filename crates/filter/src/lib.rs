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

use icon::Icon;
use matcher::{Bonus, MatchScope, Matcher, MatcherBuilder};
use rayon::prelude::*;
use std::sync::Arc;
use types::{ClapItem, FileNameItem, GrepItem};

pub use self::parallel_worker::{
    par_dyn_run, par_dyn_run_inprocess, par_dyn_run_list, ParallelInputSource, StdioProgressor,
    TopMatches,
};
pub use self::sequential_source::{filter_sequential, SequentialSource};
pub use self::sequential_worker::dyn_run;
pub use matcher;
pub use types::{CaseMatching, MatchedItem, Query, SourceItem};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Popen(#[from] subprocess::PopenError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct MatchedItems(Vec<MatchedItem>);

impl MatchedItems {
    /// The item with highest score first, the item with lowest score last.
    pub fn par_sort(self) -> Self {
        let mut items = self.0;
        items.par_sort_unstable_by(|v1, v2| v2.cmp(v1));
        Self(items)
    }

    pub fn inner(self) -> Vec<MatchedItem> {
        self.0
    }
}

impl From<Vec<MatchedItem>> for MatchedItems {
    fn from(items: Vec<MatchedItem>) -> Self {
        Self(items)
    }
}

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

/// Performs the synchorous filtering on a small scale of source in parallel.
pub fn par_filter(
    source_items: impl IntoParallelIterator<Item = Arc<dyn ClapItem>>,
    fuzzy_matcher: &Matcher,
) -> Vec<MatchedItem> {
    let matched_items: MatchedItems = source_items
        .into_par_iter()
        .filter_map(|item| fuzzy_matcher.match_item(item))
        .collect::<Vec<_>>()
        .into();
    matched_items.par_sort().inner()
}

/// Performs the synchorous filtering on a small scale of source in parallel.
pub fn par_filter_items(
    source_items: &[Arc<dyn ClapItem>],
    fuzzy_matcher: &Matcher,
) -> Vec<MatchedItem> {
    let matched_items: MatchedItems = source_items
        .into_par_iter()
        .filter_map(|item| fuzzy_matcher.match_item(item.clone()))
        .collect::<Vec<_>>()
        .into();
    matched_items.par_sort().inner()
}
