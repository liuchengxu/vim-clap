mod stoppable_searcher;

pub use self::stoppable_searcher::search;
use self::stoppable_searcher::{FileResult, StoppableSearchImpl, UPDATE_INTERVAL};
use matcher::Matcher;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::unbounded_channel;

use super::SearchInfo;

#[derive(Debug)]
pub struct SearchResult {
    pub matches: Vec<FileResult>,
    pub total_matched: u64,
    pub total_processed: u64,
}

pub async fn cli_search(paths: Vec<PathBuf>, matcher: Matcher) -> SearchResult {
    let (sender, mut receiver) = unbounded_channel();

    let stop_signal = Arc::new(AtomicBool::new(false));

    let search_info = SearchInfo::new();

    {
        let search_info = search_info.clone();
        std::thread::Builder::new()
            .name("searcher-worker".into())
            .spawn(move || {
                StoppableSearchImpl::new(paths, matcher, sender, stop_signal, usize::MAX)
                    .run(search_info)
            })
            .expect("Failed to spawn searcher worker thread");
    }

    let mut matches = Vec::new();
    let mut total_matched = 0;

    let mut past = Instant::now();

    while let Some(file_result) = receiver.recv().await {
        matches.push(file_result);
        total_matched += 1;
        let total_processed = search_info.total_processed.load(Ordering::Relaxed);

        if total_matched % 16 == 0 || total_processed.is_multiple_of(16) {
            let now = Instant::now();
            if now > past + UPDATE_INTERVAL {
                println!("total_matched: {total_matched:?}, total_processed: {total_processed:?}");
                past = now;
            }
        }
    }

    let total_processed = search_info.total_processed.load(Ordering::SeqCst) as u64;

    SearchResult {
        matches,
        total_matched,
        total_processed,
    }
}
