//! Recent files tracking using frecency algorithm.

use frecency::{FrecentEntry, FrecentItems};
use matcher::{Bonus, MatcherBuilder};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

// Re-export core frecency types for external use.
pub use frecency::DEFAULT_MAX_ENTRIES;

/// Preference for sorting the recent files.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub enum SortPreference {
    /// Sort by the visit time.
    #[default]
    Frequency,
    /// Sort by the number of visits.
    Recency,
    /// Sort by both `Frecency` and `Recency`.
    Frecency,
}

/// In memory version of sorted recent files.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SortedRecentFiles {
    /// Maximum number of entries.
    #[serde(default = "default_max_entries")]
    pub max_entries: u64,
    /// Sort preference of entries.
    #[serde(default)]
    pub sort_preference: SortPreference,
    /// Inner frecent items storage.
    #[serde(flatten)]
    inner: FrecentItems<String>,
}

fn default_max_entries() -> u64 {
    DEFAULT_MAX_ENTRIES as u64
}

impl Default for SortedRecentFiles {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES as u64,
            sort_preference: Default::default(),
            inner: FrecentItems::with_max_entries(DEFAULT_MAX_ENTRIES),
        }
    }
}

impl SortedRecentFiles {
    /// Deletes the invalid ones from current entries.
    ///
    /// Used when loading from the disk.
    pub fn remove_invalid_entries(mut self) -> Self {
        let mut paths = HashSet::new();
        self.inner.retain(|entry| {
            let Ok(path) = std::fs::canonicalize(&entry.item) else {
                return false;
            };
            if paths.contains(&path) {
                return false;
            }
            let is_valid_entry = path.exists() && path.is_file();
            paths.insert(path);
            is_valid_entry
        });
        self
    }

    /// Returns the size of entries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a reference to the entries.
    pub fn entries(&self) -> &[FrecentEntry<String>] {
        &self.inner.entries
    }

    /// Sort the entries by adding a bonus score given `cwd`.
    pub fn sort_by_cwd(&mut self, cwd: &str) {
        let cwd_owned = cwd.to_string();
        self.inner.entries.sort_unstable_by(|a, b| {
            let a_score = if a.item.starts_with(&cwd_owned) {
                a.frecent_score * 2
            } else {
                a.frecent_score
            };
            let b_score = if b.item.starts_with(&cwd_owned) {
                b.frecent_score * 2
            } else {
                b.frecent_score
            };
            b_score
                .cmp(&a_score)
                .then_with(|| b.last_access.cmp(&a.last_access))
        });
    }

    pub fn recent_n_files(&self, n: usize) -> Vec<String> {
        self.inner.top_n(n).into_iter().cloned().collect()
    }

    pub fn filter_on_query(&self, query: &str, cwd: String) -> Vec<filter::MatchedItem> {
        let mut cwd_with_separator = cwd.clone();
        cwd_with_separator.push(std::path::MAIN_SEPARATOR);

        let matcher = MatcherBuilder::new()
            .bonuses(vec![Bonus::Cwd(cwd.into()), Bonus::FileName])
            .build(query.into());

        let source_items = self.inner.entries.par_iter().map(|entry| {
            Arc::new(types::SourceItem::new(
                entry.item.replacen(&cwd_with_separator, "", 1),
                None,
                None,
            )) as Arc<dyn types::ClapItem>
        });

        filter::par_filter(source_items, &matcher)
    }

    /// Updates or inserts a new entry in a sorted way.
    pub fn upsert(&mut self, file: String) {
        self.inner.upsert(file);

        // Write back to the disk.
        if let Err(e) = crate::datastore::store_recent_files(self) {
            tracing::error!(?e, "Failed to write the recent files to the disk");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_by_cwd() {
        let mut sorted_recent_files = SortedRecentFiles::default();

        let entries = [
            "/usr/local/share/test1.txt",
            "/home/xlc/.vimrc",
            "/home/xlc/test.txt",
        ];

        for entry in entries.iter() {
            sorted_recent_files.upsert(entry.to_string());
        }

        sorted_recent_files.sort_by_cwd("/usr/local/share");

        assert_eq!(
            sorted_recent_files
                .inner
                .entries
                .iter()
                .map(|entry| entry.item.as_str())
                .collect::<Vec<_>>(),
            vec![
                "/usr/local/share/test1.txt",
                "/home/xlc/test.txt",
                "/home/xlc/.vimrc",
            ]
        );
    }
}
