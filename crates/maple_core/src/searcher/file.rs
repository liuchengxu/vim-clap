use crate::searcher::SearchContext;
use crate::stdio_server::SearchProgressor;
use filter::BestItems;
use matcher::{MatchResult, Matcher};
use printer::Printer;
use std::borrow::Cow;
use std::io::{BufRead, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use types::{ClapItem, MatchedItem, SearchProgressUpdate};

#[derive(Debug)]
pub struct BlinesItem {
    pub raw: String,
    pub line_number: usize,
}

impl ClapItem for BlinesItem {
    fn raw_text(&self) -> &str {
        self.raw.as_str()
    }

    fn output_text(&self) -> Cow<'_, str> {
        format!("{} {}", self.line_number, self.raw).into()
    }

    fn match_result_callback(&self, match_result: MatchResult) -> MatchResult {
        let mut match_result = match_result;
        match_result.indices.iter_mut().for_each(|x| {
            *x += utils::display_width(self.line_number) + 1;
        });
        match_result
    }

    fn truncation_offset(&self) -> Option<usize> {
        Some(utils::display_width(self.line_number))
    }
}

fn search_lines(
    source_file: PathBuf,
    matcher: Matcher,
    stop_signal: Arc<AtomicBool>,
    item_sender: UnboundedSender<MatchedItem>,
    total_processed: Arc<AtomicUsize>,
) -> Result<()> {
    let source_file = std::fs::File::open(source_file)?;

    let index = AtomicUsize::new(0);
    let _ = std::io::BufReader::new(source_file)
        .lines()
        .try_for_each(|maybe_line| {
            if stop_signal.load(Ordering::SeqCst) {
                return Err(());
            }

            if let Ok(line) = maybe_line {
                let index = index.fetch_add(1, Ordering::SeqCst);
                total_processed.fetch_add(1, Ordering::Relaxed);

                if !line.trim().is_empty() {
                    let item: Arc<dyn ClapItem> = Arc::new(BlinesItem {
                        raw: line,
                        line_number: index + 1,
                    });

                    if let Some(matched_item) = matcher.match_item(item) {
                        item_sender.send(matched_item).map_err(|_| ())?;
                    }
                }
            }

            Ok(())
        });

    Ok(())
}

/// Search lines in a single file.
pub async fn search(
    query: String,
    source_file: PathBuf,
    matcher: Matcher,
    search_context: SearchContext,
) {
    let SearchContext {
        icon,
        line_width,
        paths: _,
        vim,
        stop_signal,
        item_pool_size,
    } = search_context;

    let printer = Printer::new(line_width, icon);
    let number = item_pool_size;
    let progressor = SearchProgressor::new(vim, stop_signal.clone());

    let mut best_items = BestItems::new(printer, number, progressor, Duration::from_millis(200));

    let (sender, mut receiver) = unbounded_channel();

    let total_processed = Arc::new(AtomicUsize::new(0));

    {
        let total_processed = total_processed.clone();
        std::thread::Builder::new()
            .name("blines-worker".into())
            .spawn({
                let stop_signal = stop_signal.clone();
                || search_lines(source_file, matcher, stop_signal, sender, total_processed)
            })
            .expect("Failed to spawn blines worker thread");
    }

    let mut total_matched = 0usize;

    let now = std::time::Instant::now();

    while let Some(matched_item) = receiver.recv().await {
        if stop_signal.load(Ordering::SeqCst) {
            return;
        }
        total_matched += 1;
        let total_processed = total_processed.load(Ordering::Relaxed);
        best_items.on_new_match(matched_item, total_matched, total_processed);
    }

    if stop_signal.load(Ordering::SeqCst) {
        return;
    }

    let elapsed = now.elapsed().as_millis();

    let BestItems {
        items,
        progressor,
        printer,
        ..
    } = best_items;

    let display_lines = printer.to_display_lines(items);
    let total_processed = total_processed.load(Ordering::SeqCst);

    progressor.on_finished(display_lines, total_matched, total_processed);

    tracing::debug!(
        total_processed,
        total_matched,
        ?query,
        "Searching completed in {elapsed:?}ms"
    );
}
