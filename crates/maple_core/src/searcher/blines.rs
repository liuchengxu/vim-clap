use crate::searcher::SearchContext;
use crate::stdio_server::VimProgressor;
use anyhow::Result;
use filter::BestItems;
use matcher::{MatchResult, Matcher};
use std::borrow::Cow;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use types::ProgressUpdate;
use types::{ClapItem, MatchedItem};

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
}

#[derive(Debug)]
enum SearcherMessage {
    Match(MatchedItem),
    ProcessedOne,
}

fn search_lines(
    source_file: PathBuf,
    matcher: Matcher,
    stop_signal: Arc<AtomicBool>,
    item_sender: UnboundedSender<SearcherMessage>,
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
                if line.trim().is_empty() {
                    item_sender
                        .send(SearcherMessage::ProcessedOne)
                        .map_err(|_| ())?;
                } else {
                    let item: Arc<dyn ClapItem> = Arc::new(BlinesItem {
                        raw: line,
                        line_number: index + 1,
                    });

                    if let Some(matched_item) = matcher.match_item(item) {
                        item_sender
                            .send(SearcherMessage::Match(matched_item))
                            .map_err(|_| ())?;
                    } else {
                        item_sender
                            .send(SearcherMessage::ProcessedOne)
                            .map_err(|_| ())?;
                    }
                }
            }

            Ok(())
        });

    Ok(())
}

pub async fn search(source_file: PathBuf, matcher: Matcher, search_context: SearchContext) {
    let SearchContext {
        icon,
        winwidth,
        cwd: _,
        vim,
        stop_signal,
        item_pool_size,
    } = search_context;

    let number = item_pool_size;
    let progressor = VimProgressor::new(vim, stop_signal.clone());

    let mut best_items = BestItems::new(icon, winwidth, number, progressor);

    let (sender, mut receiver) = unbounded_channel();

    std::thread::Builder::new()
        .name("blines-worker".into())
        .spawn({
            let stop_signal = stop_signal.clone();
            || search_lines(source_file, matcher, stop_signal, sender)
        })
        .expect("Failed to spawn blines worker thread");

    let mut total_matched = 0usize;
    let mut total_processed = 0usize;

    let now = std::time::Instant::now();

    while let Some(searcher_message) = receiver.recv().await {
        if stop_signal.load(Ordering::SeqCst) {
            return;
        }

        match searcher_message {
            SearcherMessage::Match(matched_item) => {
                total_matched += 1;
                total_processed += 1;

                best_items.on_new_match(matched_item, total_matched, total_processed);
            }
            SearcherMessage::ProcessedOne => {
                total_processed += 1;
            }
        }
    }

    let elapsed = now.elapsed().as_millis();

    let BestItems {
        items,
        progressor,
        winwidth,
        ..
    } = best_items;

    let display_lines = printer::decorate_lines(items, winwidth, icon);

    progressor.on_finished(display_lines, total_matched, total_processed);

    tracing::debug!(
        "Searching is done, elapsed: {elapsed:?}ms, \
        total_matched: {total_matched:?}, total_processed: {total_processed}",
    );
}
