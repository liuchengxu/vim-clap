//! Convert the source item stream to a parallel iterator and run the filtering in parallel.

#![allow(unused)]

use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use rayon::iter::{ParallelBridge, ParallelIterator};
use subprocess::Exec;

use matcher::Matcher;
use types::{ClapItem, MatchedItem, MultiItem, Query};

use crate::{FilterContext, Source};

/// Returns the ranked results after applying fuzzy filter given the query string and a list of candidates.
pub fn par_dyn_run<I: Iterator<Item = Arc<dyn ClapItem>>>(
    query: &str,
    source: Source<I>,
    filter_context: FilterContext,
) -> Result<()> {
    let FilterContext {
        icon,
        number,
        winwidth,
        matcher,
    } = filter_context;

    let query: Query = query.into();

    match source {
        Source::File(file) => {
            par_source_file(&matcher, &query, file)?;
        }
        _ => todo!("Implement par dyn run"),
    }

    Ok(())
}

/// Generate an iterator of [`MatchedItem`] from [`Source::File`].
pub fn par_source_file<'a, P: AsRef<Path>>(
    matcher: &'a Matcher,
    query: &'a Query,
    path: P,
) -> Result<Vec<MatchedItem>> {
    // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
    // The line stream can contain invalid UTF-8 data.
    let results = std::io::BufReader::new(std::fs::File::open(path)?)
        .lines()
        .par_bridge()
        .filter_map(|x| {
            x.ok().and_then(|line: String| {
                let item: Arc<dyn ClapItem> = Arc::new(MultiItem::from(line));
                matcher.match_item(item, query)
            })
        })
        .collect::<Vec<_>>();

    // println!("results: {results:?}");

    Ok(results)
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
