//! Frecency-based item tracking.
//!
//! Frecency is a combination of frequency and recency, used to rank items
//! based on how often and how recently they were accessed.
//!
//! See: <https://en.wikipedia.org/wiki/Frecency>

use chrono::prelude::*;
use serde::{Deserialize, Serialize};

/// UTC timestamp type alias.
pub type UtcTime = DateTime<Utc>;

// Time constants in seconds
const HOUR: i64 = 3600;
const DAY: i64 = HOUR * 24;
const WEEK: i64 = DAY * 7;
const MONTH: i64 = DAY * 30;

/// Default maximum number of entries.
pub const DEFAULT_MAX_ENTRIES: usize = 10_000;

/// A single entry in the frecency tracker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrecentEntry<T> {
    /// The tracked item.
    pub item: T,
    /// Time of last access.
    pub last_access: UtcTime,
    /// Number of total accesses.
    pub access_count: u64,
    /// Score based on frecency algorithm.
    pub frecent_score: u64,
}

impl<T: PartialEq> PartialEq for FrecentEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item
    }
}

impl<T: Eq> Eq for FrecentEntry<T> {}

impl<T> FrecentEntry<T> {
    /// Creates a new entry with the given item.
    pub fn new(item: T) -> Self {
        Self {
            item,
            last_access: Utc::now(),
            access_count: 1,
            frecent_score: 1,
        }
    }

    /// Creates a new entry with custom initial values.
    pub fn with_values(item: T, last_access: UtcTime, access_count: u64) -> Self {
        let mut entry = Self {
            item,
            last_access,
            access_count,
            frecent_score: 1,
        };
        entry.update_frecent_score(Some(Utc::now()));
        entry
    }

    /// Records a new access, updating the timestamp and score.
    pub fn record_access(&mut self) {
        let now = Utc::now();
        self.last_access = now;
        self.access_count += 1;
        self.update_frecent_score(Some(now));
    }

    /// Updates the frecent score based on time since last access.
    ///
    /// The score decays over time:
    /// - Within 1 hour: visits * 4
    /// - Within 1 day: visits * 2
    /// - Within 1 week: visits * 1.5
    /// - Within 1 month: visits * 0.5
    /// - Older: visits * 0.25
    pub fn update_frecent_score(&mut self, at: Option<UtcTime>) {
        let now = at.unwrap_or_else(Utc::now);
        let duration = now.signed_duration_since(self.last_access).num_seconds();

        self.frecent_score = if duration < HOUR {
            self.access_count * 4
        } else if duration < DAY {
            self.access_count * 2
        } else if duration < WEEK {
            self.access_count * 3 / 2
        } else if duration < MONTH {
            self.access_count / 2
        } else {
            self.access_count / 4
        };

        // Ensure minimum score of 1 if there are any accesses
        if self.frecent_score == 0 && self.access_count > 0 {
            self.frecent_score = 1;
        }
    }
}

/// A collection of frecent entries with automatic sorting and size limiting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrecentItems<T> {
    /// Maximum number of entries to keep.
    pub max_entries: usize,
    /// Sorted list of entries (highest frecency first).
    pub entries: Vec<FrecentEntry<T>>,
}

impl<T> Default for FrecentItems<T> {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES,
            entries: Vec::new(),
        }
    }
}

impl<T: Clone + PartialEq> FrecentItems<T> {
    /// Creates a new empty collection with default max entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new collection with the specified max entries.
    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            max_entries,
            entries: Vec::new(),
        }
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Updates or inserts an item, maintaining sorted order.
    ///
    /// If the item already exists, its access count and timestamp are updated.
    /// Otherwise, a new entry is created.
    pub fn upsert(&mut self, item: T) {
        match self.entries.iter().position(|e| e.item == item) {
            Some(pos) => {
                self.entries[pos].record_access();
            }
            None => {
                self.entries.push(FrecentEntry::new(item));
            }
        }

        self.sort_and_truncate();
    }

    /// Removes an item from the collection.
    pub fn remove(&mut self, item: &T) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| &e.item == item) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// Returns the top N items by frecency.
    pub fn top_n(&self, n: usize) -> Vec<&T> {
        self.entries.iter().take(n).map(|e| &e.item).collect()
    }

    /// Returns all items in frecency order.
    pub fn items(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().map(|e| &e.item)
    }

    /// Updates all frecent scores based on current time.
    ///
    /// Call this periodically or after loading from disk to ensure
    /// scores reflect current recency.
    pub fn refresh_scores(&mut self) {
        let now = Utc::now();
        for entry in &mut self.entries {
            entry.update_frecent_score(Some(now));
        }
        self.sort_and_truncate();
    }

    /// Sorts entries by frecency (higher score first), then by recency (more recent first).
    fn sort_and_truncate(&mut self) {
        self.entries.sort_unstable_by(|a, b| {
            b.frecent_score
                .cmp(&a.frecent_score)
                .then_with(|| b.last_access.cmp(&a.last_access))
        });
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }
}

impl<T: Clone + PartialEq> FrecentItems<T>
where
    T: AsRef<str>,
{
    /// Adds a preference bonus for items matching a prefix.
    ///
    /// Useful for boosting items in the current working directory.
    pub fn top_n_with_prefix_boost(&self, n: usize, prefix: &str) -> Vec<&T> {
        let mut scored: Vec<_> = self
            .entries
            .iter()
            .map(|e| {
                let score = if e.item.as_ref().starts_with(prefix) {
                    e.frecent_score * 2
                } else {
                    e.frecent_score
                };
                (score, &e.last_access, &e.item)
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(a.1)));

        scored
            .into_iter()
            .take(n)
            .map(|(_, _, item)| item)
            .collect()
    }
}

/// Filter function for removing invalid entries.
impl<T: Clone + PartialEq> FrecentItems<T> {
    /// Retains only entries that satisfy the predicate.
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&FrecentEntry<T>) -> bool,
    {
        self.entries.retain(f);
    }

    /// Filters entries, keeping only those where the predicate returns true.
    pub fn filter<F>(self, mut predicate: F) -> Self
    where
        F: FnMut(&T) -> bool,
    {
        Self {
            max_entries: self.max_entries,
            entries: self
                .entries
                .into_iter()
                .filter(|e| predicate(&e.item))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_new_item() {
        let mut items: FrecentItems<String> = FrecentItems::new();
        items.upsert("foo".to_string());

        assert_eq!(items.len(), 1);
        assert_eq!(items.entries[0].item, "foo");
        assert_eq!(items.entries[0].access_count, 1);
    }

    #[test]
    fn test_upsert_existing_item() {
        let mut items: FrecentItems<String> = FrecentItems::new();
        items.upsert("foo".to_string());
        items.upsert("foo".to_string());

        assert_eq!(items.len(), 1);
        assert_eq!(items.entries[0].access_count, 2);
    }

    #[test]
    fn test_ordering_by_frecency() {
        let mut items: FrecentItems<String> = FrecentItems::new();

        // Add items with different access counts
        items.upsert("once".to_string());
        items.upsert("twice".to_string());
        items.upsert("twice".to_string());
        items.upsert("thrice".to_string());
        items.upsert("thrice".to_string());
        items.upsert("thrice".to_string());

        let top = items.top_n(3);
        assert_eq!(top[0], "thrice");
        assert_eq!(top[1], "twice");
        assert_eq!(top[2], "once");
    }

    #[test]
    fn test_max_entries() {
        let mut items: FrecentItems<String> = FrecentItems::with_max_entries(2);

        items.upsert("a".to_string());
        items.upsert("b".to_string());
        items.upsert("c".to_string());

        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_remove() {
        let mut items: FrecentItems<String> = FrecentItems::new();
        items.upsert("foo".to_string());
        items.upsert("bar".to_string());

        assert!(items.remove(&"foo".to_string()));
        assert_eq!(items.len(), 1);
        assert!(!items.remove(&"foo".to_string()));
    }

    #[test]
    fn test_prefix_boost() {
        let mut items: FrecentItems<String> = FrecentItems::new();

        // Add items with different access counts
        // "other" has 3 accesses -> score = 3 * 4 = 12
        // "project" has 2 accesses -> score = 2 * 4 = 8
        items.upsert("/home/user/project/file.txt".to_string());
        items.upsert("/home/user/project/file.txt".to_string());
        items.upsert("/home/user/other/file.txt".to_string());
        items.upsert("/home/user/other/file.txt".to_string());
        items.upsert("/home/user/other/file.txt".to_string());

        // Without boost, "other" should be first (more accesses, score 12 vs 8)
        let top = items.top_n(2);
        assert_eq!(top[0], "/home/user/other/file.txt");

        // With boost for project dir:
        // "other": 12 (no match)
        // "project": 8 * 2 = 16 (prefix match)
        // So "project" should be first
        let top_boosted = items.top_n_with_prefix_boost(2, "/home/user/project");
        assert_eq!(top_boosted[0], "/home/user/project/file.txt");
    }
}
