use std::path::PathBuf;

use anyhow::Result;
use chrono::prelude::*;

use crate::datastore::CACHE_INFO_IN_MEMORY;
use crate::process::BaseCommand;
use crate::utils::UtcTime;

pub const MAX_DIGESTS: usize = 100;

/// Digest of a cached command execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Digest {
    /// Base command.
    #[serde(flatten)]
    pub base: BaseCommand,
    /// Time of last execution.
    pub execution_time: UtcTime,
    /// Time of last visit.
    pub last_visit: UtcTime,
    /// Number of times the base command was visited.
    pub total_visits: usize,
    /// Number of times the command was executed so far.
    pub total_executions: usize,
    /// Number of results from last execution.
    pub total: usize,
    /// File persistent on the disk for caching the results.
    pub cached_path: PathBuf,
}

impl Digest {
    /// Creates an instance of [`Digest`].
    pub fn new(base: BaseCommand, total: usize, cached_path: PathBuf) -> Self {
        let now = Utc::now();
        Self {
            base,
            total,
            cached_path,
            last_visit: now,
            total_visits: 1,
            total_executions: 1,
            execution_time: now,
        }
    }

    /// Returns the score of being stale.
    ///
    /// The item with higher stale score should be removed first.
    pub fn stale_score(&self) -> i64 {
        let now = Utc::now();
        let execution_diff = now - self.execution_time;
        let visit_diff = now - self.last_visit;

        let stale_duration = execution_diff + visit_diff;

        stale_duration.num_seconds()
    }

    pub fn is_usable(&self) -> bool {
        let now = Utc::now();

        const EXECUTION_EXPIRATION_DAYS: i64 = 3;

        if now.signed_duration_since(self.execution_time).num_days() > EXECUTION_EXPIRATION_DAYS {
            return false;
        }

        // TODO: when the preview content mismatches the line, the cache is outdated and should be updated.

        self.cached_path.exists()
    }
}

/// List of cache digests.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheInfo {
    digests: Vec<Digest>,
}

impl Default for CacheInfo {
    fn default() -> Self {
        Self {
            digests: Vec::new(),
        }
    }
}

impl CacheInfo {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            digests: Vec::with_capacity(capacity),
        }
    }

    /// Finds the digest given `base_cmd`.
    fn find_digest(&self, base_cmd: &BaseCommand) -> Option<usize> {
        self.digests.iter().position(|d| &d.base == base_cmd)
    }

    /// Finds the usable digest given `base_cmd`.
    pub fn find_digest_usable(&mut self, base_cmd: &BaseCommand) -> Option<Digest> {
        match self.find_digest(base_cmd) {
            Some(index) => {
                let mut d = &mut self.digests[index];
                if d.is_usable() {
                    d.total_visits += 1;
                    d.last_visit = Utc::now();
                    // FIXME: save the latest state?
                    Some(d.clone())
                } else {
                    if let Err(err) = self.prune_stale(index) {
                        tracing::error!(?err, "Failed to prune the stale cache digest");
                    }
                    None
                }
            }
            _ => None,
        }
    }

    /// Pushes `digest` to the digests queue with max capacity constraint.
    ///
    /// Also writes the memory cached info back to the disk.
    pub fn limited_push(&mut self, digest: Digest) -> Result<()> {
        // The digest already exists.
        if let Some(index) = self.find_digest(&digest.base) {
            let old_executions = self.digests[index].total_executions;
            let mut new_digest = digest;
            new_digest.total_executions += old_executions;
            self.digests[index] = new_digest;
        } else {
            self.digests.push(digest);

            if self.digests.len() > MAX_DIGESTS {
                self.digests.sort_unstable_by_key(|k| k.stale_score());
                self.digests.pop();
            }
        }

        crate::datastore::store_cache_info(self)?;

        Ok(())
    }

    /// Prunes the stale digest at index of `stale_index`.
    pub fn prune_stale(&mut self, stale_index: usize) -> Result<()> {
        self.digests.swap_remove(stale_index);

        crate::datastore::store_cache_info(self)?;

        Ok(())
    }
}

/// Pushes the digest of the results of new fresh run to [`CACHE_INFO_IN_MEMORY`].
pub fn push_cache_digest(digest: Digest) -> Result<()> {
    let cache_info = CACHE_INFO_IN_MEMORY.lock();

    let mut cache_info_cloned = cache_info.clone();
    tokio::spawn(async move {
        if let Err(e) = cache_info_cloned.limited_push(digest) {
            tracing::error!(?e, "Failed to push the cache digest");
        }
    });

    Ok(())
}
