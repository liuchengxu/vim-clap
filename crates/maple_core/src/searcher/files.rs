use super::{walk_parallel, WalkConfig};
use crate::searcher::SearchContext;
use crate::stdio_server::SearchProgressor;
use filter::{BestItems, MatchedItem};
use ignore::{DirEntry, WalkState};
use matcher::Matcher;
use printer::Printer;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use types::SearchProgressUpdate;

fn search_files(
    paths: Vec<PathBuf>,
    hidden: bool,
    matcher: Matcher,
    stop_signal: Arc<AtomicBool>,
    sender: UnboundedSender<Option<MatchedItem>>,
) {
    let walk_config = WalkConfig {
        hidden,
        ..Default::default()
    };

    let search_root = paths[0].clone();

    walk_parallel(paths, walk_config, "files").run(|| {
        let matcher = matcher.clone();
        let sender = sender.clone();
        let stop_signal = stop_signal.clone();
        let search_root = search_root.clone();
        Box::new(move |entry: Result<DirEntry, ignore::Error>| -> WalkState {
            if stop_signal.load(Ordering::SeqCst) {
                return WalkState::Quit;
            }

            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => return WalkState::Continue,
            };

            // Only search file and skip everything else.
            match entry.file_type() {
                Some(entry) if entry.is_file() => {}
                _ => return WalkState::Continue,
            };

            // TODO: Add match_file_path() in matcher to avoid allocation each time.
            let path = if let Ok(p) = entry.path().strip_prefix(&search_root) {
                p.to_string_lossy().to_string()
            } else {
                entry.path().to_string_lossy().to_string()
            };

            let maybe_matched_item = matcher.match_item(Arc::new(path));

            if let Err(err) = sender.send(maybe_matched_item) {
                tracing::debug!("Sender is dropped: {err:?}");
                WalkState::Quit
            } else {
                WalkState::Continue
            }
        })
    });
}

pub async fn search(query: String, hidden: bool, matcher: Matcher, search_context: SearchContext) {
    let SearchContext {
        paths,
        vim,
        icon,
        line_width,
        stop_signal,
        item_pool_size,
    } = search_context;

    let number = item_pool_size;
    let progressor = SearchProgressor::new(vim, stop_signal.clone());

    let (sender, mut receiver) = unbounded_channel();

    std::thread::Builder::new()
        .name("files-worker".into())
        .spawn({
            let stop_signal = stop_signal.clone();
            move || search_files(paths, hidden, matcher, stop_signal, sender)
        })
        .expect("Failed to spawn blines worker thread");

    let mut total_matched = 0usize;
    let mut total_processed = 0usize;

    let printer = Printer::new(line_width, icon);
    let mut best_items = BestItems::new(printer, number, progressor, Duration::from_millis(200));

    let now = std::time::Instant::now();

    while let Some(maybe_matched_item) = receiver.recv().await {
        if stop_signal.load(Ordering::SeqCst) {
            return;
        }

        match maybe_matched_item {
            Some(matched_item) => {
                total_matched += 1;
                total_processed += 1;

                best_items.on_new_match(matched_item, total_matched, total_processed);
            }
            None => {
                total_processed += 1;
            }
        }
    }

    let elapsed = now.elapsed().as_millis();

    let BestItems {
        items,
        progressor,
        printer,
        ..
    } = best_items;

    let display_lines = printer.to_display_lines(items);

    progressor.on_finished(display_lines, total_matched, total_processed);

    tracing::debug!(
        total_processed,
        total_matched,
        ?query,
        "Searching completed in {elapsed:?}ms"
    );
}
