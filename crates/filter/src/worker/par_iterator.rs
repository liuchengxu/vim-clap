//! Convert the source item stream to a parallel iterator and run the filtering in parallel.

#![allow(unused)]

use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use rayon::iter::{ParallelBridge, ParallelIterator};

use matcher::Matcher;
use subprocess::Exec;
use types::{ClapItem, MatchedItem, MultiItem, Query};

pub fn run() {}

/// Generate an iterator of [`MatchedItem`] from [`Source::File`].
pub fn par_source_file<'a, P: AsRef<Path>>(
    matcher: &'a Matcher,
    query: &'a Query,
    path: P,
) -> Result<impl ParallelIterator<Item = MatchedItem> + 'a> {
    // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
    // The line stream can contain invalid UTF-8 data.
    Ok(std::io::BufReader::new(std::fs::File::open(path)?)
        .lines()
        .par_bridge()
        .filter_map(|x| {
            x.ok().and_then(|line: String| {
                let item: Arc<dyn ClapItem> = Arc::new(MultiItem::from(line));
                matcher.match_item(item, query)
            })
        }))
}

/// Generate an iterator of [`MatchedItem`] from [`Source::Exec`].
pub fn par_source_exec<'a>(
    matcher: &'a Matcher,
    query: &'a Query,
    exec: Box<Exec>,
) -> Result<impl ParallelIterator<Item = MatchedItem> + 'a> {
    Ok(std::io::BufReader::new(exec.stream_stdout()?)
        .lines()
        .par_bridge()
        .filter_map(|lines_iter| {
            lines_iter.ok().and_then(|line: String| {
                let item: Arc<dyn ClapItem> = Arc::new(MultiItem::from(line));
                matcher.match_item(item, query)
            })
        }))
}
