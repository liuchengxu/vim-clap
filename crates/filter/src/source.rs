use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use subprocess::Exec;

use matcher::Matcher;
use types::{ClapItem, MatchedItem, Query, SourceItem};

/// Source is anything that can produce an iterator of String.
#[derive(Debug)]
pub enum SequentialSource<I: Iterator<Item = Arc<dyn ClapItem>>> {
    List(I),
    Stdin,
    File(PathBuf),
    Exec(Box<Exec>),
}

impl<I: Iterator<Item = Arc<dyn ClapItem>>> From<PathBuf> for SequentialSource<I> {
    fn from(fpath: PathBuf) -> Self {
        Self::File(fpath)
    }
}

impl<I: Iterator<Item = Arc<dyn ClapItem>>> From<Exec> for SequentialSource<I> {
    fn from(exec: Exec) -> Self {
        Self::Exec(Box::new(exec))
    }
}

impl<I: Iterator<Item = Arc<dyn ClapItem>>> SequentialSource<I> {
    /// Returns the complete filtered results given `matcher` and `query`.
    ///
    /// This is kind of synchronous filtering, can be used for multi-staged processing.
    pub fn run_and_collect(self, matcher: Matcher, query: &Query) -> Result<Vec<MatchedItem>> {
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
            .filter_map(|item| matcher.match_item(item, query))
            .collect())
    }
}
