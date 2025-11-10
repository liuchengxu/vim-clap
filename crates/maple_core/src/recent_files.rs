use crate::UtcTime;
use chrono::prelude::*;
use matcher::{Bonus, MatcherBuilder};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::Arc;

// 3600 seconds
const HOUR: i64 = 3600;
const DAY: i64 = HOUR * 24;
const WEEK: i64 = DAY * 7;
const MONTH: i64 = DAY * 30;

/// Maximum number of recent files.
const MAX_ENTRIES: u64 = 10_000;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrecentEntry {
    /// Absolute file path.
    pub fpath: String,
    /// Time of last visit.
    pub last_visit: UtcTime,
    /// Number of total visits.
    pub visits: u64,
    /// Score based on https://en.wikipedia.org/wiki/Frecency
    pub frecent_score: u64,
}

impl PartialEq for FrecentEntry {
    fn eq(&self, other: &Self) -> bool {
        self.fpath == other.fpath
    }
}

impl Eq for FrecentEntry {}

impl PartialOrd for FrecentEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FrecentEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.frecent_score.cmp(&other.frecent_score) {
            Ordering::Equal => other.last_visit.cmp(&self.last_visit),
            other => other,
        }
    }
}

impl FrecentEntry {
    /// Creates a new instance of [`FrecentEntry`].
    pub fn new(fpath: String) -> Self {
        Self {
            fpath,
            last_visit: Utc::now(),
            visits: 1u64,
            frecent_score: 1u64,
        }
    }

    /// Updates an existing entry.
    pub fn refresh_now(&mut self) {
        let now = Utc::now();
        self.last_visit = now;
        self.visits += 1;
        self.update_frecent(Some(now));
    }

    /// Updates the frecent score.
    pub fn update_frecent(&mut self, at: Option<UtcTime>) {
        let now = at.unwrap_or_else(Utc::now);

        let duration = now.signed_duration_since(self.last_visit).num_seconds();

        self.frecent_score = if duration < HOUR {
            self.visits * 4
        } else if duration < DAY {
            self.visits * 2
        } else if duration < WEEK {
            self.visits * 3 / 2
        } else if duration < MONTH {
            self.visits / 2
        } else {
            self.visits / 4
        };
    }

    /// Add a bonus score based on cwd.
    pub fn cwd_preferred_score(&self, cwd: &str) -> u64 {
        if self.fpath.starts_with(cwd) {
            self.frecent_score * 2
        } else {
            self.frecent_score
        }
    }
}

/// In memory version of sorted recent files.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SortedRecentFiles {
    /// Maximum number of entries.
    pub max_entries: u64,
    /// Sort preference of entries.
    pub sort_preference: SortPreference,
    /// An ordered list of [`FrecentEntry`].
    pub entries: Vec<FrecentEntry>,
}

impl Default for SortedRecentFiles {
    fn default() -> Self {
        Self {
            max_entries: MAX_ENTRIES,
            sort_preference: Default::default(),
            entries: Default::default(),
        }
    }
}

impl SortedRecentFiles {
    /// Deletes the invalid ones from current entries.
    ///
    /// Used when loading from the disk.
    pub fn remove_invalid_entries(self) -> Self {
        let mut paths = HashSet::new();
        Self {
            entries: self
                .entries
                .into_iter()
                .filter_map(|entry| {
                    let path = std::fs::canonicalize(&entry.fpath).ok()?;
                    if paths.contains(&path) {
                        None
                    } else {
                        let is_valid_entry = path.exists() && path.is_file();
                        paths.insert(path);
                        is_valid_entry.then_some(entry)
                    }
                })
                .collect(),
            ..self
        }
    }

    /// Returns the size of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Sort the entries by adding a bonus score given `cwd`.
    pub fn sort_by_cwd(&mut self, cwd: &str) {
        self.entries.sort_unstable_by(|a, b| {
            b.cwd_preferred_score(cwd)
                .cmp(&a.cwd_preferred_score(cwd))
                .then_with(|| b.last_visit.cmp(&a.last_visit))
        });
    }

    pub fn recent_n_files(&self, n: usize) -> Vec<String> {
        self.entries
            .iter()
            .take(n)
            .map(|entry| entry.fpath.clone())
            .collect()
    }

    pub fn filter_on_query(&self, query: &str, cwd: String) -> Vec<filter::MatchedItem> {
        let mut cwd_with_separator = cwd.clone();
        cwd_with_separator.push(std::path::MAIN_SEPARATOR);

        let matcher = MatcherBuilder::new()
            .bonuses(vec![Bonus::Cwd(cwd.into()), Bonus::FileName])
            .build(query.into());

        let source_items = self.entries.par_iter().map(|entry| {
            Arc::new(types::SourceItem::new(
                entry.fpath.replacen(&cwd_with_separator, "", 1),
                None,
                None,
            )) as Arc<dyn types::ClapItem>
        });

        filter::par_filter(source_items, &matcher)
    }

    /// Updates or inserts a new entry in a sorted way.
    pub fn upsert(&mut self, file: String) {
        match self
            .entries
            .iter()
            .position(|entry| entry.fpath.as_str() == file.as_str())
        {
            Some(pos) => FrecentEntry::refresh_now(&mut self.entries[pos]),
            None => {
                let entry = FrecentEntry::new(file);
                self.entries.push(entry);
            }
        }

        self.entries
            .sort_unstable_by(|a, b| b.partial_cmp(a).unwrap());

        if self.entries.len() > self.max_entries as usize {
            self.entries.truncate(self.max_entries as usize);
        }

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
                .entries
                .iter()
                .map(|entry| entry.fpath.as_str())
                .collect::<Vec<_>>(),
            vec![
                "/usr/local/share/test1.txt",
                "/home/xlc/test.txt",
                "/home/xlc/.vimrc",
            ]
        );
    }
}
