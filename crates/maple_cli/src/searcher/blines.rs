use super::UPDATE_INTERVAL;
use anyhow::Result;
use icon::Icon;
use matcher::MatchResult;
use matcher::Matcher;
use printer::DisplayLines;
use std::borrow::Cow;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use types::ProgressUpdate;
use types::{ClapItem, MatchedItem};

#[derive(Debug)]
struct BlinesItem {
    raw: String,
    line_number: usize,
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
            *x += crate::utils::display_width(self.line_number) + 1;
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
    let source_file = std::fs::File::open(&source_file)?;

    let index = AtomicUsize::new(0);
    let _ = std::io::BufReader::new(source_file)
        .lines()
        .try_for_each(|x| {
            if stop_signal.load(Ordering::SeqCst) {
                return Err(());
            }

            if let Ok(line) = x {
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

    // let filter_context = if let Some(extension) = self
    // .input
    // .extension()
    // .and_then(|s| s.to_str().map(|s| s.to_string()))
    // {
    // params
    // .into_filter_context()
    // .bonuses(vec![Bonus::Language(extension.into())])
    // } else {
    // params.into_filter_context()
    // };

    Ok(())
}

#[derive(Debug)]
struct BestItems {
    /// Time of last notification.
    past: Instant,
    items: Vec<MatchedItem>,
    last_lines: Vec<String>,
    last_visible_highlights: Vec<Vec<usize>>,
    max_capacity: usize,
}

impl BestItems {
    fn new(max_capacity: usize) -> Self {
        Self {
            past: Instant::now(),
            items: Vec::with_capacity(max_capacity),
            last_lines: Vec::with_capacity(max_capacity),
            last_visible_highlights: Vec::with_capacity(max_capacity),
            max_capacity,
        }
    }

    fn sort(&mut self) {
        self.items.sort_unstable_by(|a, b| b.score.cmp(&a.score));
    }
}

pub async fn search<P: ProgressUpdate<DisplayLines>>(
    source_file: PathBuf,
    matcher: Matcher,
    stop_signal: Arc<AtomicBool>,
    number: usize,
    icon: Icon,
    winwidth: usize,
    progressor: P,
) {
    let mut best_items = BestItems::new(number);

    let (sender, mut receiver) = unbounded_channel();

    std::thread::Builder::new()
        .name("blines-worker".into())
        .spawn({
            let stop_signal = stop_signal.clone();
            || search_lines(source_file, matcher, stop_signal, sender)
        })
        .expect("Failed to spawn searcher worker thread");

    let mut total_matched = 0usize;
    let mut total_processed = 0usize;

    let to_display_lines = |items: Vec<MatchedItem>, winwidth: usize, icon: Icon| {
        printer::decorate_lines(items, winwidth, icon)
    };

    let now = std::time::Instant::now();
    while let Some(searcher_message) = receiver.recv().await {
        if stop_signal.load(Ordering::SeqCst) {
            return;
        }

        match searcher_message {
            SearcherMessage::Match(matched_item) => {
                total_matched += 1;
                total_processed += 1;

                if best_items.items.len() <= best_items.max_capacity {
                    best_items.items.push(matched_item);
                    best_items.sort();

                    let now = Instant::now();
                    if now > best_items.past + UPDATE_INTERVAL {
                        let display_lines =
                            to_display_lines(best_items.items.clone(), winwidth, icon);
                        progressor.update_all(&display_lines, total_matched, total_processed);
                        best_items.last_lines = display_lines.lines;
                        best_items.past = now;
                    }
                } else {
                    let last = best_items
                        .items
                        .last_mut()
                        .expect("Max capacity is non-zero; qed");

                    let new = matched_item;
                    if new.score > last.score {
                        *last = new;
                        best_items.sort();
                    }

                    if total_matched % 16 == 0 || total_processed % 16 == 0 {
                        let now = Instant::now();
                        if now > best_items.past + UPDATE_INTERVAL {
                            let display_lines =
                                to_display_lines(best_items.items.clone(), winwidth, icon);

                            let visible_highlights = display_lines
                                .indices
                                .iter()
                                .map(|line_highlights| {
                                    line_highlights
                                        .iter()
                                        .copied()
                                        .filter(|&x| x <= winwidth)
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>();

                            if best_items.last_lines != display_lines.lines.as_slice()
                                || best_items.last_visible_highlights != visible_highlights
                            {
                                progressor.update_all(
                                    &display_lines,
                                    total_matched,
                                    total_processed,
                                );
                                best_items.last_lines = display_lines.lines;
                                best_items.last_visible_highlights = visible_highlights;
                            } else {
                                progressor.update_brief(total_matched, total_processed)
                            }

                            best_items.past = now;
                        }
                    }
                }
            }
            SearcherMessage::ProcessedOne => {
                total_processed += 1;
            }
        }
    }

    tracing::debug!("Elapsed: {:?}ms", now.elapsed().as_millis());

    let display_lines = to_display_lines(best_items.items, winwidth, icon);

    progressor.on_finished(display_lines, total_matched, total_processed);

    tracing::debug!(
        "Searching is done, total_matched: {total_matched:?}, total_processed: {total_processed}",
    );
}
