use super::{walk_parallel, WalkConfig};
use crate::searcher::SearchContext;
use crate::stdio_server::VimProgressor;
use filter::{BestItems, MatchedItem};
use ignore::{DirEntry, WalkState};
use matcher::Matcher;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use types::ProgressUpdate;

fn search_files(
    search_root: PathBuf,
    hidden: bool,
    matcher: Matcher,
    stop_signal: Arc<AtomicBool>,
    sender: UnboundedSender<Option<MatchedItem>>,
) {
    let walk_config = WalkConfig {
        hidden,
        ..Default::default()
    };

    walk_parallel(search_root.clone(), walk_config).run(|| {
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

            let path = if let Ok(p) = entry.path().strip_prefix(&search_root) {
                p.to_string_lossy().to_string()
            } else {
                entry.path().to_string_lossy().to_string()
            };

            // TODO: Add match_file_path() in matcher to avoid allocation each time.
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

pub async fn search(hidden: bool, matcher: Matcher, search_context: SearchContext) {
    let SearchContext {
        cwd,
        vim,
        icon,
        winwidth,
        stop_signal,
        item_pool_size,
    } = search_context;

    let number = item_pool_size;
    let progressor = VimProgressor::new(vim, stop_signal.clone());
    let search_root = cwd;

    let (sender, mut receiver) = unbounded_channel();

    std::thread::Builder::new()
        .name("files-worker".into())
        .spawn({
            let stop_signal = stop_signal.clone();
            move || search_files(search_root, hidden, matcher, stop_signal, sender)
        })
        .expect("Failed to spawn blines worker thread");

    let mut total_matched = 0usize;
    let mut total_processed = 0usize;

    let mut best_items = BestItems::new(icon, winwidth, number, progressor);

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
