use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use rayon::prelude::*;
use subprocess::Exec;

use matcher::Matcher;
use types::{ClapItem, MatchedItem, SourceItem};

/// Source is anything that can produce an iterator of String.
#[derive(Debug)]
pub enum Source<I: Iterator<Item = Arc<dyn ClapItem>>> {
    List(I),
    Stdin,
    File(PathBuf),
    Exec(Box<Exec>),
}

impl<I: Iterator<Item = Arc<dyn ClapItem>>> From<PathBuf> for Source<I> {
    fn from(fpath: PathBuf) -> Self {
        Self::File(fpath)
    }
}

impl<I: Iterator<Item = Arc<dyn ClapItem>>> From<Exec> for Source<I> {
    fn from(exec: Exec) -> Self {
        Self::Exec(Box::new(exec))
    }
}

#[derive(Debug)]
pub struct MatchedItems(Vec<MatchedItem>);

impl MatchedItems {
    /// The item with highest score first, the item with lowest score last.
    pub fn sort(self) -> Self {
        let mut items = self.0;
        items.par_sort_unstable_by(|v1, v2| v2.score.partial_cmp(&v1.score).unwrap());
        Self(items)
    }
}

impl From<Vec<MatchedItem>> for MatchedItems {
    fn from(items: Vec<MatchedItem>) -> Self {
        Self(items)
    }
}

impl From<MatchedItems> for Vec<MatchedItem> {
    fn from(matched_items: MatchedItems) -> Self {
        matched_items.0
    }
}

impl<I: Iterator<Item = Arc<dyn ClapItem>>> Source<I> {
    /// Returns the complete filtered results given `matcher` and `query`.
    ///
    /// This is kind of synchronous filtering, can be used for multi-staged processing.
    pub fn match_items(self, matcher: Matcher) -> Result<MatchedItems> {
        let clap_item_stream: Box<dyn Iterator<Item = Arc<dyn ClapItem>>> = match self {
            Self::List(list) => Box::new(list),
            Self::Stdin => Box::new(
                std::io::stdin()
                    .lock()
                    .lines()
                    .filter_map(Result::ok)
                    .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>),
            ),
            Self::File(path) => Box::new(
                std::io::BufReader::new(std::fs::File::open(path)?)
                    .lines()
                    .filter_map(Result::ok)
                    .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>),
            ),
            Self::Exec(exec) => Box::new(
                std::io::BufReader::new(exec.stream_stdout()?)
                    .lines()
                    .filter_map(Result::ok)
                    .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>),
            ),
        };

        Ok(clap_item_stream
            .filter_map(|item| matcher.match_item(item))
            .collect::<Vec<_>>()
            .into())
    }
}
