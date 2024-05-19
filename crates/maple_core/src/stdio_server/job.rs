//! This module ensures the process of same command won't be spawned multiple times simultaneously.

use futures::Future;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;

static JOBS: Lazy<Arc<Mutex<HashSet<u64>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::default())));

/// Spawn a new task to run the job if it's not reserved.
#[allow(unused)]
pub fn try_start(job_future: impl Future<Output = ()> + Send + Sync + 'static, job_id: u64) {
    if reserve(job_id) {
        tokio::spawn(async move {
            job_future.await;
            unreserve(job_id)
        });
    }
}

pub fn reserve(job_id: u64) -> bool {
    let mut jobs = JOBS.lock();
    if jobs.contains(&job_id) {
        false
    } else {
        jobs.insert(job_id);
        true
    }
}

pub fn unreserve(job_id: u64) {
    let mut jobs = JOBS.lock();
    jobs.remove(&job_id);
}

// Define a function to spawn a new thread and run a future
pub fn spawn_on_new_thread<F>(future: F) -> std::thread::JoinHandle<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    std::thread::spawn(move || {
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(32)
            .build()
            .unwrap();
        tokio_runtime.block_on(future);
    })
}
