use crate::datastore::CACHE_INFO_IN_MEMORY;
use crate::process::ShellCommand;
use crate::UtcTime;
use chrono::prelude::*;
use std::path::PathBuf;

pub const MAX_DIGESTS: usize = 100;

/// Digest of a cached command execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Digest {
    /// Base command.
    #[serde(flatten)]
    pub shell_cmd: ShellCommand,
    /// Time of last visit.
    pub last_visit: UtcTime,
    /// Time of last execution.
    pub execution_time: UtcTime,
    /// Number of results from last execution.
    pub total: usize,
    /// Number of times the base command was visited.
    pub total_visits: usize,
    /// Number of times the command was executed so far.
    pub total_executions: usize,
    /// File persistent on the disk for caching the results.
    pub cached_path: PathBuf,
}

impl Digest {
    /// Creates an instance of [`Digest`].
    pub fn new(shell_cmd: ShellCommand, total: usize, cached_path: PathBuf) -> Self {
        let now = Utc::now();
        Self {
            shell_cmd,
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
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CacheInfo {
    digests: Vec<Digest>,
}

impl CacheInfo {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            digests: Vec::with_capacity(capacity),
        }
    }

    /// Remove the entries whose `cwd` no longer exists.
    ///
    /// The original directory for the cache can be deleted or moved to another place.
    pub fn remove_invalid_and_old_entries(&mut self) {
        let now = Utc::now();

        const MAX_DAYS: i64 = 30;

        self.digests.retain(|digest| {
            if digest.shell_cmd.cwd.exists()
                && digest.cached_path.exists()
                && now.signed_duration_since(digest.last_visit).num_days() < MAX_DAYS
                // In case the cache was not created completely.
                && std::fs::File::open(&digest.cached_path)
                    .and_then(utils::count_lines)
                    .map(|total| total == digest.total)
                    .unwrap_or(false)
            {
                true
            } else {
                // Remove the cache file accordingly.
                let _ = std::fs::remove_file(&digest.cached_path);
                false
            }
        });
    }

    /// Finds the digest given `shell_cmd`.
    fn find_digest(&self, shell_cmd: &ShellCommand) -> Option<usize> {
        self.digests.iter().position(|d| &d.shell_cmd == shell_cmd)
    }

    /// Finds the usable digest given `shell_cmd`.
    pub fn find_digest_usable(&mut self, shell_cmd: &ShellCommand) -> Option<Digest> {
        match self.find_digest(shell_cmd) {
            Some(index) => {
                let d = &mut self.digests[index];
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
    pub fn limited_push(&mut self, digest: Digest) -> std::io::Result<()> {
        // The digest already exists.
        if let Some(index) = self.find_digest(&digest.shell_cmd) {
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

        crate::datastore::store_cache_info(self)
    }

    /// Prunes the stale digest at index of `stale_index`.
    pub fn prune_stale(&mut self, stale_index: usize) -> std::io::Result<()> {
        self.digests.swap_remove(stale_index);
        crate::datastore::store_cache_info(self)
    }

    pub fn to_digests(&self) -> Vec<Digest> {
        self.digests.clone()
    }
}

/// Pushes the digest of the results of new fresh run to [`CACHE_INFO_IN_MEMORY`].
pub fn push_cache_digest(digest: Digest) {
    let cache_info = CACHE_INFO_IN_MEMORY.clone();

    tokio::spawn(async move {
        let mut cache_info = cache_info.lock();
        if let Err(e) = cache_info.limited_push(digest) {
            tracing::error!(?e, "Failed to push the cache digest");
        }
    });
}

pub fn find_largest_cache_digest() -> Option<Digest> {
    let cache_info = CACHE_INFO_IN_MEMORY.lock();
    let mut digests = cache_info.to_digests();
    digests.sort_unstable_by_key(|digest| digest.total);
    digests.last().cloned()
}

pub fn store_cache_digest(
    shell_cmd: ShellCommand,
    new_created_cache: PathBuf,
) -> std::io::Result<Digest> {
    let total = utils::count_lines(std::fs::File::open(&new_created_cache)?)?;

    let digest = Digest::new(shell_cmd, total, new_created_cache);

    let cache_info = crate::datastore::CACHE_INFO_IN_MEMORY.clone();
    let mut cache_info = cache_info.lock();
    cache_info.limited_push(digest.clone())?;

    Ok(digest)
}
