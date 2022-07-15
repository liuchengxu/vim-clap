//! Convert the source item stream to a parallel iterator and run the filtering in parallel.

#![allow(unused)]

use std::path::Path;
use std::sync::Arc;
use std::{
    io::BufRead,
    sync::atomic::{AtomicUsize, Ordering},
};

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
    let matched_count = AtomicUsize::new(0);
    let processed_count = AtomicUsize::new(0);

    // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
    // The line stream can contain invalid UTF-8 data.
    let results = std::io::BufReader::new(std::fs::File::open(path)?)
        .lines()
        .par_bridge()
        .filter_map(|x| {
            x.ok().and_then(|line: String| {
                let processed = processed_count.fetch_add(1, Ordering::Relaxed);

                let item: Arc<dyn ClapItem> = Arc::new(MultiItem::from(line));
                matcher.match_item(item, query).map(|matched_item| {
                    let matched = matched_count.fetch_add(1, Ordering::Relaxed);

                    if matched % 64 == 0 || processed % 1024 == 0 {
                        println!("====== [{matched}/{processed}]");
                    }

                    matched_item
                })
            })
        })
        .collect::<Vec<_>>();

    let matched_count = matched_count.load(Ordering::Relaxed);
    let total_count = processed_count.load(Ordering::Relaxed);
    println!("====== [{matched_count}/{total_count}]");

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
