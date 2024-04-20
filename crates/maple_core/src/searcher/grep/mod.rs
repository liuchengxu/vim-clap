mod stoppable_searcher;

pub use self::stoppable_searcher::search;
use self::stoppable_searcher::{FileResult, StoppableSearchImpl, UPDATE_INTERVAL};
use matcher::Matcher;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::unbounded_channel;

#[derive(Debug)]
pub struct SearchResult {
    pub matches: Vec<FileResult>,
    pub total_matched: u64,
    pub total_processed: u64,
}

pub async fn cli_search(paths: Vec<PathBuf>, matcher: Matcher) -> SearchResult {
    let (sender, mut receiver) = unbounded_channel();

    let stop_signal = Arc::new(AtomicBool::new(false));

    let total_processed = Arc::new(AtomicUsize::new(0));

    {
        let total_processed = total_processed.clone();
        std::thread::Builder::new()
            .name("searcher-worker".into())
            .spawn(move || {
                StoppableSearchImpl::new(paths, matcher, sender, stop_signal).run(total_processed)
            })
            .expect("Failed to spawn searcher worker thread");
    }

    let mut matches = Vec::new();
    let mut total_matched = 0;

    let mut past = Instant::now();

    while let Some(file_result) = receiver.recv().await {
        matches.push(file_result);
        total_matched += 1;
        let total_processed = total_processed.load(std::sync::atomic::Ordering::Relaxed);

        if total_matched % 16 == 0 || total_processed % 16 == 0 {
            let now = Instant::now();
            if now > past + UPDATE_INTERVAL {
                println!("total_matched: {total_matched:?}, total_processed: {total_processed:?}");
                past = now;
            }
        }
    }

    let total_processed = total_processed.load(std::sync::atomic::Ordering::SeqCst) as u64;

    SearchResult {
        matches,
        total_matched,
        total_processed,
    }
}
