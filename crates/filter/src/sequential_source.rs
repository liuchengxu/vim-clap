use crate::MatchedItems;
use matcher::Matcher;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;
use subprocess::Exec;
use types::{ClapItem, MatchedItem, SourceItem};

/// [`SequentialSource`] provides an iterator of [`ClapItem`] which
/// will be processed sequentially.
#[derive(Debug)]
pub enum SequentialSource<I: Iterator<Item = Arc<dyn ClapItem>>> {
    Iterator(I),
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

/// Filters items from a sequential source using the given matcher.
pub fn filter_sequential<I: Iterator<Item = Arc<dyn ClapItem>>>(
    source: SequentialSource<I>,
    matcher: Matcher,
) -> crate::Result<Vec<MatchedItem>> {
    let clap_item_stream: Box<dyn Iterator<Item = Arc<dyn ClapItem>>> = match source {
        SequentialSource::Iterator(iter) => Box::new(iter),
        SequentialSource::Stdin => Box::new(
            std::io::stdin()
                .lock()
                .lines()
                .map_while(Result::ok)
                .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>),
        ),
        SequentialSource::File(path) => Box::new(
            std::io::BufReader::new(std::fs::File::open(path)?)
                .lines()
                .map_while(Result::ok)
                .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>),
        ),
        SequentialSource::Exec(exec) => Box::new(
            std::io::BufReader::new(exec.stream_stdout()?)
                .lines()
                .map_while(Result::ok)
                .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>),
        ),
    };

    Ok(MatchedItems::from(
        clap_item_stream
            .filter_map(|item| matcher.match_item(item))
            .collect::<Vec<_>>(),
    )
    .par_sort()
    .inner())
}
