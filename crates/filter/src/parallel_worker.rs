//! Convert the source item stream to a parallel iterator and run the filtering in parallel.

use crate::{to_clap_item, FilterContext};
use parking_lot::Mutex;
use printer::{println_json_with_length, DisplayLines, Printer};
use rayon::iter::{Empty, IntoParallelIterator, ParallelBridge, ParallelIterator};
use std::cmp::Ordering as CmpOrdering;
use std::io::{BufRead, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use subprocess::Exec;
use types::{ClapItem, MatchedItem, Query, SearchProgressUpdate};

/// Represents a source for parallel processing (e.g, file or command output).
#[derive(Debug)]
pub enum ParallelInputSource {
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
    input_source: ParallelInputSource,
) -> crate::Result<()> {
    let query: Query = query.into();

    match input_source {
        ParallelInputSource::File(file) => {
            run_parallel_filter::<Empty<_>, _>(
                query,
                filter_context,
                ParallelSource::Lines(std::fs::File::open(file)?),
            )?;
        }
        ParallelInputSource::Exec(exec) => {
            run_parallel_filter::<Empty<_>, _>(
                query,
                filter_context,
                ParallelSource::Lines(exec.stream_stdout()?),
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
    run_parallel_filter::<_, std::io::Empty>(query, filter_context, ParallelSource::Items(items))
        .expect("Matching items in parallel can not fail");
}

/// Manages the top N matches based on scores.
#[derive(Debug)]
pub struct TopMatches<P: SearchProgressUpdate<DisplayLines>> {
    /// Time of last notification.
    pub last_update_time: Instant,
    /// Top N items.
    pub items: Vec<MatchedItem>,
    pub last_lines: Vec<String>,
    pub last_visible_highlights: Vec<Vec<usize>>,
    pub max_capacity: usize,
    pub progressor: P,
    pub update_interval: Duration,
    pub printer: Printer,
}

impl<P: SearchProgressUpdate<DisplayLines>> TopMatches<P> {
    pub fn new(
        printer: Printer,
        max_capacity: usize,
        progressor: P,
        update_interval: Duration,
    ) -> Self {
        Self {
            printer,
            last_update_time: Instant::now(),
            items: Vec::with_capacity(max_capacity),
            last_lines: Vec::with_capacity(max_capacity),
            last_visible_highlights: Vec::with_capacity(max_capacity),
            max_capacity,
            progressor,
            update_interval,
        }
    }

    fn sort(&mut self) {
        self.items.sort_unstable_by(|a, b| b.cmp(a));
    }

    pub fn on_new_match(
        &mut self,
        matched_item: MatchedItem,
        total_matched: usize,
        total_processed: usize,
    ) {
        if self.items.len() < self.max_capacity {
            self.items.push(matched_item);
            self.sort();

            let now = Instant::now();
            if now > self.last_update_time + self.update_interval {
                let display_lines = self.printer.to_display_lines(self.items.clone());
                self.progressor
                    .update_all(&display_lines, total_matched, total_processed);
                self.last_lines = display_lines.lines;
                self.last_update_time = now;
            }
        } else {
            let last = self
                .items
                .last_mut()
                .expect("Max capacity is non-zero; qed");

            let new = matched_item;
            if let CmpOrdering::Greater = new.cmp(last) {
                *last = new;
                self.sort();
            }

            if total_matched % 16 == 0 || total_processed % 16 == 0 {
                let now = Instant::now();
                if now > self.last_update_time + self.update_interval {
                    let display_lines = self.printer.to_display_lines(self.items.clone());

                    let visible_highlights = display_lines
                        .indices
                        .iter()
                        .map(|line_highlights| {
                            line_highlights
                                .iter()
                                .copied()
                                .filter(|&x| x <= self.printer.line_width)
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();

                    if self.last_lines != display_lines.lines.as_slice()
                        || self.last_visible_highlights != visible_highlights
                    {
                        self.progressor
                            .update_all(&display_lines, total_matched, total_processed);
                        self.last_lines = display_lines.lines;
                        self.last_visible_highlights = visible_highlights;
                    } else {
                        self.progressor.quick_update(total_matched, total_processed)
                    }

                    self.last_update_time = now;
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct StdioProgressor;

impl SearchProgressUpdate<DisplayLines> for StdioProgressor {
    fn quick_update(&self, matched: usize, processed: usize) {
        #[allow(non_upper_case_globals)]
        const deprecated_method: &str = "clap#legacy#state#process_filter_message";

        println_json_with_length!(matched, processed, deprecated_method);
    }

    fn update_all(&self, display_lines: &DisplayLines, matched: usize, processed: usize) {
        #[allow(non_upper_case_globals)]
        const deprecated_method: &str = "clap#legacy#state#process_filter_message";

        let DisplayLines {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = display_lines;

        if truncated_map.is_empty() {
            println_json_with_length!(
                deprecated_method,
                lines,
                indices,
                icon_added,
                matched,
                processed
            );
        } else {
            println_json_with_length!(
                deprecated_method,
                lines,
                indices,
                icon_added,
                matched,
                processed,
                truncated_map
            );
        }
    }

    fn on_finished(
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
        const deprecated_method: &str = "clap#legacy#state#process_filter_message";
        println_json_with_length!(
            deprecated_method,
            lines,
            indices,
            icon_added,
            truncated_map,
            total_matched,
            total_processed
        );
    }
}

enum ParallelSource<I: IntoParallelIterator<Item = Arc<dyn ClapItem>>, R: Read + Send> {
    Items(I),
    Lines(R),
}

/// Runs the core fuzzy matching process on a data source in parallel.
fn run_parallel_filter<I, R>(
    query: Query,
    filter_context: FilterContext,
    parallel_source: ParallelSource<I, R>,
) -> std::io::Result<()>
where
    I: IntoParallelIterator<Item = Arc<dyn ClapItem>>,
    R: Read + Send,
{
    let FilterContext {
        icon,
        number,
        winwidth,
        matcher_builder,
    } = filter_context;

    let matcher = matcher_builder.build(query);

    let winwidth = winwidth.unwrap_or(100);
    let number = number.unwrap_or(100);

    let matched_count = AtomicUsize::new(0);
    let processed_count = AtomicUsize::new(0);

    let printer = Printer::new(winwidth, icon);
    let top_matches = Mutex::new(TopMatches::new(
        printer,
        number,
        StdioProgressor,
        Duration::from_millis(200),
    ));

    let process_item = |item: Arc<dyn ClapItem>| {
        let processed = processed_count.fetch_add(1, Ordering::SeqCst);
        if let Some(matched_item) = matcher.match_item(item) {
            let matched = matched_count.fetch_add(1, Ordering::SeqCst);

            // TODO: not use mutex?
            let mut top_matches = top_matches.lock();
            top_matches.on_new_match(matched_item, matched, processed);
            drop(top_matches);
        }
    };

    match parallel_source {
        ParallelSource::Items(items) => items.into_par_iter().for_each(process_item),
        ParallelSource::Lines(reader) => {
            // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
            // The line stream can contain invalid UTF-8 data.
            std::io::BufReader::new(reader)
                .lines()
                .map_while(Result::ok)
                .par_bridge()
                .for_each(|line: String| {
                    if let Some(item) = to_clap_item(matcher.match_scope(), line) {
                        process_item(item);
                    }
                });
        }
    }

    let total_matched = matched_count.into_inner();
    let total_processed = processed_count.into_inner();

    let TopMatches {
        items,
        progressor,
        printer,
        ..
    } = top_matches.into_inner();

    let matched_items = items;

    let display_lines = printer.to_display_lines(matched_items);
    progressor.on_finished(display_lines, total_matched, total_processed);

    Ok(())
}

/// Similar to `[par_dyn_run]`, but used in the process which means we need to cancel the command
/// creating the items manually in order to cancel the task ASAP.
pub fn par_dyn_run_inprocess<P>(
    query: &str,
    filter_context: FilterContext,
    input_source: ParallelInputSource,
    progressor: P,
    stop_signal: Arc<AtomicBool>,
) -> std::io::Result<()>
where
    P: SearchProgressUpdate<DisplayLines> + Send,
{
    let query: Query = query.into();

    let FilterContext {
        icon,
        number,
        winwidth,
        matcher_builder,
    } = filter_context;

    let matcher = matcher_builder.build(query);

    let winwidth = winwidth.unwrap_or(100);
    let number = number.unwrap_or(100);

    let matched_count = AtomicUsize::new(0);
    let processed_count = AtomicUsize::new(0);

    let printer = Printer::new(winwidth, icon);
    let top_matches = Mutex::new(TopMatches::new(
        printer,
        number,
        progressor,
        Duration::from_millis(200),
    ));

    let process_item = |item: Arc<dyn ClapItem>, processed: usize| {
        if let Some(matched_item) = matcher.match_item(item) {
            let matched = matched_count.fetch_add(1, Ordering::SeqCst);

            // TODO: not use mutex?
            let mut top_matches = top_matches.lock();
            top_matches.on_new_match(matched_item, matched, processed);
            drop(top_matches);
        }
    };

    let read: Box<dyn std::io::Read + Send> = match input_source {
        ParallelInputSource::File(file) => Box::new(std::fs::File::open(file)?),
        ParallelInputSource::Exec(exec) => Box::new(
            exec.detached()
                .stream_stdout()
                .map_err(|e| std::io::Error::other(e.to_string()))?,
        ), // TODO: kill the exec command ASAP/ Run the exec command in another blocking task.
    };

    // To avoid Err(Custom { kind: InvalidData, error: "stream did not contain valid UTF-8" })
    // The line stream can contain invalid UTF-8 data.
    let res = std::io::BufReader::new(read)
        .lines()
        .map_while(Result::ok)
        .par_bridge()
        .try_for_each(|line: String| {
            if stop_signal.load(Ordering::SeqCst) {
                tracing::debug!(?matcher, "[par_dyn_run_inprocess] stop signal received");
                // Note that even the stop signal has been received, the thread created by
                // rayon does not exit actually, it just tries to stop the work ASAP.
                Err(())
            } else {
                let processed = processed_count.fetch_add(1, Ordering::SeqCst);
                if let Some(item) = to_clap_item(matcher.match_scope(), line) {
                    process_item(item, processed);
                }
                Ok(())
            }
        });

    let total_matched = matched_count.into_inner();
    let total_processed = processed_count.into_inner();

    if res.is_err() {
        tracing::debug!(
            ?total_matched,
            ?total_processed,
            "[par_dyn_run_inprocess] return early due to the stop signal arrived."
        );
        return Ok(());
    }

    let TopMatches {
        items,
        progressor,
        printer,
        ..
    } = top_matches.into_inner();

    let matched_items = items;

    let display_lines = printer.to_display_lines(matched_items);
    progressor.on_finished(display_lines, total_matched, total_processed);

    Ok(())
}
