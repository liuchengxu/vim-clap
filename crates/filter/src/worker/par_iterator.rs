//! Convert the source item stream to a parallel iterator and run the filtering in parallel.

use std::io::{BufRead, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use matcher::MatchScope;
use parking_lot::Mutex;
use printer::DisplayLines;
use rayon::iter::{Empty, IntoParallelIterator, ParallelBridge, ParallelIterator};
use subprocess::Exec;
use types::ProgressUpdate;

use icon::Icon;
use types::{ClapItem, FileNameItem, GrepItem, MatchedItem, Query, SourceItem};
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
///
/// Suitable for invoking the maple CLI command from shell, which will stop everything once the
/// parent is canceled.
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
struct BestItems<P: ProgressUpdate<DisplayLines>> {
    /// Time of last notification.
    past: Instant,
    items: Vec<MatchedItem>,
    last_lines: Vec<String>,
    max_capacity: usize,
    icon: Icon,
    winwidth: usize,
    progressor: P,
}

#[derive(Debug)]
pub struct StdioProgressor;

impl ProgressUpdate<DisplayLines> for StdioProgressor {
    fn update_progress(
        &self,
        maybe_display_lines: Option<&DisplayLines>,
        matched: usize,
        processed: usize,
    ) {
        #[allow(non_upper_case_globals)]
        const method: &str = "s:process_filter_message";

        if let Some(display_lines) = maybe_display_lines {
            let DisplayLines {
                lines,
                indices,
                truncated_map,
                icon_added,
            } = display_lines;

            if truncated_map.is_empty() {
                println_json_with_length!(method, lines, indices, icon_added, matched, processed);
            } else {
                println_json_with_length!(
                    method,
                    lines,
                    indices,
                    icon_added,
                    matched,
                    processed,
                    truncated_map
                );
            }
        } else {
            println_json_with_length!(matched, processed, method);
        }
    }

    fn update_progress_on_finished(
        &self,
        display_lines: DisplayLines,
        total_matched: usize,
        total_processed: usize,
    ) {
        let DisplayLines {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = display_lines;

        #[allow(non_upper_case_globals)]
        const method: &str = "s:process_filter_message";
        println_json_with_length!(
            method,
            lines,
            indices,
            icon_added,
            truncated_map,
            total_matched,
            total_processed
        );
    }
}

impl<P: ProgressUpdate<DisplayLines>> BestItems<P> {
    fn new(max_capacity: usize, icon: Icon, winwidth: usize, progressor: P) -> Self {
        Self {
            past: Instant::now(),
            items: Vec::with_capacity(max_capacity),
            last_lines: Vec::with_capacity(max_capacity),
            max_capacity,
            icon,
            winwidth,
            progressor,
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
                self.progressor
                    .update_progress(Some(&display_lines), matched, processed);
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
                        self.progressor
                            .update_progress(Some(&display_lines), matched, processed);
                        self.last_lines = display_lines.lines;
                    } else {
                        self.progressor.update_progress(None, matched, processed)
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
fn par_dyn_run_inner<I, R>(
    query: &Query,
    filter_context: FilterContext,
    parallel_source: ParSourceInner<I, R>,
) -> Result<()>
where
    I: IntoParallelIterator<Item = Arc<dyn ClapItem>>,
    R: Read + Send,
{
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

    let best_items = Mutex::new(BestItems::new(number, icon, winwidth, StdioProgressor));

    let process_item = |item: Arc<dyn ClapItem>, processed: usize| {
        if let Some(matched_item) = matcher.match_item(item, query) {
            let matched = matched_count.fetch_add(1, Ordering::SeqCst);

            // TODO: not use mutex?
            let mut best_items = best_items.lock();
            best_items.try_push_and_notify(matched_item, matched, processed);
            drop(best_items);
        }
    };

    match parallel_source {
        ParSourceInner::Items(items) => items.into_par_iter().for_each(|item| {
            let processed = processed_count.fetch_add(1, Ordering::SeqCst);
            process_item(item, processed);
        }),
        ParSourceInner::Lines(reader) => {
            // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
            // The line stream can contain invalid UTF-8 data.
            std::io::BufReader::new(reader)
                .lines()
                .filter_map(Result::ok)
                .par_bridge()
                .for_each(|line: String| {
                    let processed = processed_count.fetch_add(1, Ordering::SeqCst);
                    let item: Arc<dyn ClapItem> = match matcher.match_scope() {
                        MatchScope::GrepLine => {
                            if let Some(grep_item) = GrepItem::try_new(line) {
                                Arc::new(grep_item)
                            } else {
                                // Continue the next item
                                return;
                            }
                        }
                        MatchScope::FileName => {
                            if let Some(file_name_item) = FileNameItem::try_new(line) {
                                Arc::new(file_name_item)
                            } else {
                                // Continue the next item
                                return;
                            }
                        }
                        _ => Arc::new(SourceItem::from(line)),
                    };
                    process_item(item, processed);
                });
        }
    }

    let total_matched = matched_count.into_inner();
    let total_processed = processed_count.into_inner();

    let BestItems {
        items, progressor, ..
    } = best_items.into_inner();

    let matched_items = items;

    let display_lines = printer::decorate_lines(matched_items, winwidth, icon);
    progressor.update_progress_on_finished(display_lines, total_matched, total_processed);

    Ok(())
}

/// Similar to `[par_dyn_run]`, but used in the process which means we need to cancel the command
/// creating the items manually in order to cancel the task ASAP.
pub fn par_dyn_run_inprocess<P>(
    query: &str,
    filter_context: FilterContext,
    par_source: ParSource,
    progressor: P,
    stop_signal: Arc<AtomicBool>,
) -> Result<()>
where
    P: ProgressUpdate<DisplayLines> + Send,
{
    let query: Query = query.into();

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

    let best_items = Mutex::new(BestItems::new(number, icon, winwidth, progressor));

    let process_item = |item: Arc<dyn ClapItem>, processed: usize| {
        if let Some(matched_item) = matcher.match_item(item, &query) {
            let matched = matched_count.fetch_add(1, Ordering::SeqCst);

            // TODO: not use mutex?
            let mut best_items = best_items.lock();
            best_items.try_push_and_notify(matched_item, matched, processed);
            drop(best_items);
        }
    };

    let read: Box<dyn std::io::Read + Send> = match par_source {
        ParSource::File(file) => Box::new(std::fs::File::open(file)?),
        ParSource::Exec(exec) => Box::new(exec.detached().stream_stdout()?),
    };

    // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
    // The line stream can contain invalid UTF-8 data.
    let res = std::io::BufReader::new(read)
        .lines()
        .filter_map(Result::ok)
        .par_bridge()
        .try_for_each(|line: String| {
            if stop_signal.load(Ordering::Relaxed) {
                // Note that even the stop signal has been received, the thread created by
                // rayon does not exit actually, it just tries to stop the work ASAP.
                Err(())
            } else {
                let processed = processed_count.fetch_add(1, Ordering::SeqCst);
                let item: Arc<dyn ClapItem> = match matcher.match_scope() {
                    MatchScope::GrepLine => {
                        if let Some(grep_item) = GrepItem::try_new(line) {
                            Arc::new(grep_item)
                        } else {
                            // Continue the next item
                            return Ok(());
                        }
                    }
                    MatchScope::FileName => {
                        if let Some(file_name_item) = FileNameItem::try_new(line) {
                            Arc::new(file_name_item)
                        } else {
                            // Continue the next item
                            return Ok(());
                        }
                    }
                    _ => Arc::new(SourceItem::from(line)),
                };
                process_item(item, processed);
                Ok(())
            }
        });

    let total_matched = matched_count.into_inner();
    let total_processed = processed_count.into_inner();

    if res.is_err() {
        tracing::debug!(
            ?query,
            ?total_matched,
            ?total_processed,
            "`par_dyn_run_inprocess` return early due to the stop signal arrived."
        );
        return Ok(());
    }

    let BestItems {
        items, progressor, ..
    } = best_items.into_inner();

    let matched_items = items;

    let display_lines = printer::decorate_lines(matched_items, winwidth, icon);
    progressor.update_progress_on_finished(display_lines, total_matched, total_processed);

    Ok(())
}
