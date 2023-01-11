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
