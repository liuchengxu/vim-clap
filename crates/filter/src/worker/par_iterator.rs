//! Convert the source item stream to a parallel iterator and run the filtering in parallel.

#![allow(unused)]

use std::path::Path;
use std::sync::Arc;
use std::{
    io::BufRead,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use anyhow::Result;
use parking_lot::Mutex;
use rayon::iter::{ParallelBridge, ParallelIterator};
use subprocess::Exec;

use matcher::Matcher;
use types::{ClapItem, MatchedItem, MultiItem, Query};

use crate::{sort_initial_filtered, FilterContext, Source};

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

    let (total_processed, filtered) = match source {
        Source::File(file) => par_source_file(&matcher, &query, file)?,
        _ => todo!("Implement par dyn run"),
    };

    let total_matched = filtered.len();

    let ranked = sort_initial_filtered(filtered);

    printer::print_dyn_filter_results(
        ranked,
        total_matched,
        number.unwrap_or(100),
        winwidth.unwrap_or(100),
        icon,
    );

    Ok(())
}

/// Refresh the top filtered results per 300 ms.
const UPDATE_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Debug)]
pub struct BestItems {
    /// Time of last notification.
    past: Instant,
    items: Vec<MatchedItem>,
    last_lines: Vec<String>,
    max_capacity: usize,
}

impl BestItems {
    fn new(max_capacity: usize) -> Self {
        Self {
            past: Instant::now(),
            items: Vec::new(),
            last_lines: Vec::new(),
            max_capacity,
        }
    }

    fn try_push_and_notify(&mut self, new: MatchedItem, matched: usize, processed: usize) {
        if self.items.len() <= self.max_capacity {
            self.items.push(new);
            self.items.sort_unstable_by(|a, b| b.score.cmp(&a.score));
        } else {
            let last = self
                .items
                .last_mut()
                .expect("Max capacity is non-zero; qed");

            if new.score > last.score {
                *last = new;
                self.items.sort_unstable_by(|a, b| b.score.cmp(&a.score));
            }

            if matched % 16 == 0 || processed % 16 == 0 {
                let now = Instant::now();
                if now > self.past + UPDATE_INTERVAL {
                    // if let Some(ref last_seen) = self.last_seen {
                    // TODO: calculate diff beween last_seen and current results.
                    // Send the diff to the client?
                    // }

                    // self.last_seen.replace(self.items.clone());
                    println!("====== [{matched}/{processed}]");

                    self.past = now;
                }
            }
        }
    }
}

/// Generate an iterator of [`MatchedItem`] from [`Source::File`].
pub fn par_source_file<'a, P: AsRef<Path>>(
    matcher: &'a Matcher,
    query: &'a Query,
    path: P,
) -> Result<(usize, Vec<MatchedItem>)> {
    let matched_count = AtomicUsize::new(0);
    let processed_count = AtomicUsize::new(0);

    let best_items = Arc::new(Mutex::new(BestItems::new(100)));

    // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
    // The line stream can contain invalid UTF-8 data.
    let matched_items = std::io::BufReader::new(std::fs::File::open(path)?)
        .lines()
        .par_bridge()
        .filter_map(|x| {
            x.ok().and_then(|line: String| {
                let processed = processed_count.fetch_add(1, Ordering::Relaxed);

                let item: Arc<dyn ClapItem> = Arc::new(MultiItem::from(line));
                matcher.match_item(item, query).map(|matched_item| {
                    let matched = matched_count.fetch_add(1, Ordering::Relaxed);

                    let mut best_items = best_items.lock();
                    best_items.try_push_and_notify(matched_item.clone(), matched, processed);

                    matched_item
                })
            })
        })
        .collect::<Vec<_>>();

    let total_processed = processed_count.load(Ordering::Relaxed);

    Ok((total_processed, matched_items))
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
