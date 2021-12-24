use std::cmp::Ordering;
use std::path::Path;

use chrono::prelude::*;
use filter::SourceItem;
use matcher::{Bonus, FuzzyAlgorithm, MatchType};
use serde::{Deserialize, Serialize};

use crate::utils::UtcTime;

// 3600 seconds
const HOUR: i64 = 3600;
const DAY: i64 = HOUR * 24;
const WEEK: i64 = DAY * 7;
const MONTH: i64 = DAY * 30;

const MAX_ENTRIES: u64 = 10_000;

/// Preference for sorting the recent files.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SortPreference {
    /// Sort by the visit time.
    Frequency,
    /// Sort by the number of visits.
    Recency,
    /// Sort by both `Frecency` and `Recency`.
    Frecency,
}

impl Default for SortPreference {
    fn default() -> Self {
        Self::Frecency
    }
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
        Some((self.frecent_score, self.visits, self.last_visit).cmp(&(
            other.frecent_score,
            other.visits,
            other.last_visit,
        )))
    }
}

impl Ord for FrecentEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
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
    pub fn adjusted_score(&self, cwd: &str) -> u64 {
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
        Self {
            entries: self
                .entries
                .into_iter()
                .filter(|entry| Path::new(&entry.fpath).exists())
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
            b.adjusted_score(cwd)
                .partial_cmp(&a.adjusted_score(cwd))
                .unwrap()
        });
    }

    pub fn filter_on_query(&self, query: &str, cwd: String) -> Vec<filter::FilteredItem> {
        let source_items: Vec<SourceItem> = self
            .entries
            .iter()
            .map(|entry| entry.fpath.as_str().into())
            .collect();

        let matcher = matcher::Matcher::with_bonuses(
            FuzzyAlgorithm::Fzy,
            MatchType::Full,
            vec![Bonus::cwd(cwd), Bonus::FileName],
        );

        filter::par_filter(query, source_items, &matcher)
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

        let entries = vec![
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
