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

use icon::IconPainter;
use matcher::{Algo, Bonus, MatchType, Matcher};

pub use self::dynamic::dyn_run;
pub use self::source::Source;
pub use matcher;
pub use source_item::{FilteredItem, SourceItem};
#[cfg(feature = "enable_dyn")]
pub use subprocess;

/// Context for running the filter.
#[derive(Debug, Clone)]
pub struct FilterContext {
    algo: Option<Algo>,
    number: Option<usize>,
    winwidth: Option<usize>,
    icon_painter: Option<IconPainter>,
    match_type: MatchType,
}

impl Default for FilterContext {
    fn default() -> Self {
        Self {
            algo: None,
            number: None,
            winwidth: None,
            icon_painter: None,
            match_type: MatchType::Full,
        }
    }
}

impl FilterContext {
    pub fn new(
        algo: Option<Algo>,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
        match_type: MatchType,
    ) -> Self {
        Self {
            algo,
            number,
            winwidth,
            icon_painter,
            match_type,
        }
    }

    pub fn algo(mut self, algo: Option<Algo>) -> Self {
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

    pub fn icon_painter(mut self, icon_painter: Option<IconPainter>) -> Self {
        self.icon_painter = icon_painter;
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
pub(crate) fn sort_initial_filtered(filtered: Vec<FilteredItem>) -> Vec<FilteredItem> {
    let mut filtered = filtered;
    filtered.par_sort_unstable_by(|v1, v2| v2.score.partial_cmp(&v1.score).unwrap());
    filtered
}

/// Returns the ranked results after applying the matcher algo
/// given the query String and filtering source.
pub fn sync_run<I: Iterator<Item = SourceItem>>(
    query: &str,
    source: Source<I>,
    algo: Algo,
    match_type: MatchType,
    bonuses: Vec<Bonus>,
) -> Result<Vec<FilteredItem>> {
    let matcher = Matcher::new_with_bonuses(algo, match_type, bonuses);
    let filtered = source.filter(matcher, query)?;
    let ranked = sort_initial_filtered(filtered);
    Ok(ranked)
}
