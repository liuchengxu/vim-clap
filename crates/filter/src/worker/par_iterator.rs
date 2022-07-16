//! Convert the source item stream to a parallel iterator and run the filtering in parallel.

#![allow(unused)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{
    io::BufRead,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use anyhow::Result;
use parking_lot::Mutex;
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use subprocess::Exec;

use icon::{Icon, ICON_LEN};
use matcher::Matcher;
use types::{ClapItem, MatchedItem, MultiItem, Query};
use utility::println_json_with_length;

use crate::{sort_initial_filtered, FilterContext, Source};

/// Parallelable source.
#[derive(Debug)]
pub enum ParSource {
    File(PathBuf),
    Exec(Box<Exec>),
}

/// Returns the ranked results after applying fuzzy filter given the query string and a list of candidates.
pub fn par_dyn_run(
    query: &str,
    par_source: ParSource,
    filter_context: FilterContext,
) -> Result<()> {
    let query: Query = query.into();

    match par_source {
        ParSource::File(file) => {
            par_dyn_run_inner(&query, filter_context, std::fs::File::open(file)?)?;
        }
        ParSource::Exec(exec) => {
            par_dyn_run_inner(&query, filter_context, exec.stream_stdout()?)?;
        }
    }

    Ok(())
}

/// Refresh the top filtered results per 200 ms.
const UPDATE_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug)]
struct BestItems {
    /// Time of last notification.
    past: Instant,
    items: Vec<MatchedItem>,
    last_lines: Vec<String>,
    max_capacity: usize,
    icon: Icon,
    winwidth: usize,
}

impl BestItems {
    fn new(max_capacity: usize, icon: Icon, winwidth: usize) -> Self {
        Self {
            past: Instant::now(),
            items: Vec::new(),
            last_lines: Vec::new(),
            max_capacity,
            icon,
            winwidth,
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
                    let decorated_lines =
                        printer::decorate_lines(self.items.clone(), self.winwidth, self.icon);

                    if self.last_lines != decorated_lines.lines.as_slice() {
                        decorated_lines.print_on_filter_ongoing(matched, processed);
                        self.last_lines = decorated_lines.lines;
                    } else {
                        #[allow(non_upper_case_globals)]
                        const method: &str = "s:process_filter_message";
                        println_json_with_length!(matched, processed, method);
                    }

                    self.past = now;
                }
            }
        }
    }
}

/// Generate an iterator of [`MatchedItem`] from [`Source::List(list)`].
pub fn par_dyn_run_list<'a, 'b: 'a>(
    matcher: &'a Matcher,
    query: &'a Query,
    icon: Icon,
    winwidth: usize,
    list: impl IntoParallelIterator<Item = Arc<dyn ClapItem>> + 'b,
) -> (usize, Vec<MatchedItem>) {
    let matched_count = AtomicUsize::new(0);
    let processed_count = AtomicUsize::new(0);

    let best_items = Arc::new(Mutex::new(BestItems::new(100, icon, winwidth)));

    let matched_items = list
        .into_par_iter()
        .filter_map(|item| {
            let processed = processed_count.fetch_add(1, Ordering::Relaxed);
            matcher.match_item(item, query).map(|matched_item| {
                let matched = matched_count.fetch_add(1, Ordering::Relaxed);

                let mut best_items = best_items.lock();
                best_items.try_push_and_notify(matched_item.clone(), matched, processed);

                matched_item
            })
        })
        .collect::<Vec<_>>();

    let total_processed = processed_count.into_inner();

    (total_processed, matched_items)
}

/// Perform the matching on a stream of [`Source::File`] and `[Source::Exec]` in parallel.
fn par_dyn_run_inner(
    query: &Query,
    filter_context: FilterContext,
    reader: impl Read + Send,
) -> Result<()> {
    let FilterContext {
        icon,
        number,
        winwidth,
        matcher,
    } = filter_context;

    let winwidth = winwidth.unwrap_or(100);

    let matched_count = AtomicUsize::new(0);
    let processed_count = AtomicUsize::new(0);

    let best_items = Arc::new(Mutex::new(BestItems::new(100, icon, winwidth)));

    // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
    // The line stream can contain invalid UTF-8 data.
    let matched_items = std::io::BufReader::new(reader)
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

    let total_processed = processed_count.into_inner();

    let total_matched = matched_items.len();

    let matched_items = sort_initial_filtered(matched_items);

    printer::print_dyn_filter_results(
        matched_items,
        total_matched,
        number.unwrap_or(100),
        winwidth,
        icon,
    );

    Ok(())
}
