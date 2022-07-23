//! Convert the source item stream to a parallel iterator and run the filtering in parallel.

use std::io::{BufRead, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use parking_lot::Mutex;
use rayon::iter::{Empty, IntoParallelIterator, ParallelBridge, ParallelIterator};
use subprocess::Exec;

use icon::Icon;
use types::{ClapItem, MatchedItem, MultiItem, Query};
use utility::println_json_with_length;

use crate::FilterContext;

/// Refresh the top filtered results per 200 ms.
const UPDATE_INTERVAL: Duration = Duration::from_millis(200);

/// Parallelable source.
#[derive(Debug)]
pub enum ParSource {
    File(PathBuf),
    Exec(Box<Exec>),
}

/// Returns the ranked results after applying fuzzy filter given the query string and a list of candidates.
pub fn par_dyn_run(
    query: &str,
    filter_context: FilterContext,
    par_source: ParSource,
) -> Result<()> {
    let query: Query = query.into();

    match par_source {
        ParSource::File(file) => {
            par_dyn_run_inner::<Empty<_>, _>(
                &query,
                filter_context,
                ParSourceInner::Lines(std::fs::File::open(file)?),
            )?;
        }
        ParSource::Exec(exec) => {
            par_dyn_run_inner::<Empty<_>, _>(
                &query,
                filter_context,
                ParSourceInner::Lines(exec.stream_stdout()?),
            )?;
        }
    }

    Ok(())
}

/// Generate an iterator of [`MatchedItem`] from a parallelable iterator.
pub fn par_dyn_run_list<'a, 'b: 'a>(
    query: &'a str,
    filter_context: FilterContext,
    items: impl IntoParallelIterator<Item = Arc<dyn ClapItem>> + 'b,
) {
    let query: Query = query.into();
    par_dyn_run_inner::<_, std::io::Empty>(&query, filter_context, ParSourceInner::Items(items))
        .expect("Matching items in parallel can not fail");
}

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
            items: Vec::with_capacity(max_capacity),
            last_lines: Vec::with_capacity(max_capacity),
            max_capacity,
            icon,
            winwidth,
        }
    }

    fn try_push_and_notify(&mut self, new: MatchedItem, matched: usize, processed: usize) {
        if self.items.len() <= self.max_capacity {
            self.items.push(new);
            self.items.sort_unstable_by(|a, b| b.score.cmp(&a.score));

            let now = Instant::now();
            if now > self.past + UPDATE_INTERVAL {
                let display_lines =
                    printer::decorate_lines(self.items.clone(), self.winwidth, self.icon);
                display_lines.print_on_dyn_run(matched, processed);
                self.last_lines = display_lines.lines;
                self.past = now;
            }
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
                    let display_lines =
                        printer::decorate_lines(self.items.clone(), self.winwidth, self.icon);

                    // TODO: the lines are the same, but the highlights are not.
                    if self.last_lines != display_lines.lines.as_slice() {
                        display_lines.print_on_dyn_run(matched, processed);
                        self.last_lines = display_lines.lines;
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

enum ParSourceInner<I: IntoParallelIterator<Item = Arc<dyn ClapItem>>, R: Read + Send> {
    Items(I),
    Lines(R),
}

/// Perform the matching on a stream of [`Source::File`] and `[Source::Exec]` in parallel.
fn par_dyn_run_inner<I: IntoParallelIterator<Item = Arc<dyn ClapItem>>, R: Read + Send>(
    query: &Query,
    filter_context: FilterContext,
    parallel_source: ParSourceInner<I, R>,
) -> Result<()> {
    let FilterContext {
        icon,
        number,
        winwidth,
        matcher,
    } = filter_context;

    let winwidth = winwidth.unwrap_or(100);
    let number = number.unwrap_or(100);

    let matched_count = AtomicUsize::new(0);
    let processed_count = AtomicUsize::new(0);

    let best_items = Arc::new(Mutex::new(BestItems::new(number, icon, winwidth)));

    let process_item = |item: Arc<dyn ClapItem>, processed: usize| {
        if let Some(matched_item) = matcher.match_item(item, query) {
            let matched = matched_count.fetch_add(1, Ordering::Relaxed);

            let mut best_items = best_items.lock();
            best_items.try_push_and_notify(matched_item, matched, processed);
            drop(best_items);
        }
    };

    match parallel_source {
        ParSourceInner::Items(items) => {
            items.into_par_iter().for_each(|item| {
                let processed = processed_count.fetch_add(1, Ordering::SeqCst);
                process_item(item, processed);
            });
        }
        ParSourceInner::Lines(reader) => {
            // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
            // The line stream can contain invalid UTF-8 data.
            std::io::BufReader::new(reader)
                .lines()
                .filter_map(Result::ok)
                .par_bridge()
                .for_each(|line: String| {
                    let processed = processed_count.fetch_add(1, Ordering::SeqCst);
                    let item: Arc<dyn ClapItem> = Arc::new(MultiItem::from(line));
                    process_item(item, processed);
                });
        }
    }

    let total_matched = matched_count.into_inner();
    let total_processed = processed_count.into_inner();

    let matched_items = Arc::try_unwrap(best_items)
        .expect("More than one strong reference")
        .into_inner()
        .items;

    printer::print_dyn_matched_items(
        matched_items,
        total_matched,
        Some(total_processed),
        winwidth,
        icon,
    );

    Ok(())
}
