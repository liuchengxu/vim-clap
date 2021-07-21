use std::cmp::Ordering;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use chrono::prelude::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::utils::UtcTime;

const HOUR: i64 = 3600;
const DAY: i64 = HOUR * 24;
const WEEK: i64 = DAY * 7;

const MAX_ENTRIES: u64 = 10_000;

const JSON_FILENAME: &str = "recent_files.json";

pub static RECENT_FILES_JSON_PATH: Lazy<Option<PathBuf>> =
    Lazy::new(|| crate::utils::generate_data_file_path(JSON_FILENAME).ok());

pub static RECENT_FILES_IN_MEMORY: Lazy<Mutex<SortedRecentFiles>> = Lazy::new(|| {
    let maybe_persistent = crate::utils::load_json(RECENT_FILES_JSON_PATH.as_deref())
        .map(|f: SortedRecentFiles| f.remove_invalid_entries())
        .unwrap_or_default();
    Mutex::new(maybe_persistent)
});

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

#[derive(Clone, Debug, Eq, Ord, Serialize, Deserialize)]
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

impl PartialOrd for FrecentEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some((self.frecent_score, self.visits, self.last_visit).cmp(&(
            other.frecent_score,
            other.visits,
            other.last_visit,
        )))
    }
}

impl FrecentEntry {
    pub fn new(fpath: String) -> Self {
        Self {
            fpath,
            last_visit: Utc::now(),
            visits: 1u64,
            frecent_score: 1u64,
        }
    }

    pub fn refresh_now(&mut self) {
        let now = Utc::now();
        self.last_visit = now;
        self.visits += 1;
        self.update_frecent(Some(now));
    }

    pub fn update_frecent(&mut self, at: Option<UtcTime>) {
        let now = at.unwrap_or_else(Utc::now);

        let duration = now.signed_duration_since(self.last_visit).num_seconds();

        self.frecent_score = if duration < HOUR {
            self.visits * 4
        } else if duration < DAY {
            self.visits * 2
        } else if duration < WEEK {
            self.visits / 2
        } else {
            self.visits / 4
        };
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
    pub fn remove_invalid_entries(self) -> Self {
        Self {
            entries: self
                .entries
                .into_iter()
                .filter(|entry| std::path::Path::new(&entry.fpath).exists())
                .collect(),
            ..self
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn filter_on_query(&self, query: &str) -> Vec<filter::FilteredItem> {
        // .map(|entry| {
        // let fpath = &entry.fpath;
        // let user_dirs = directories::UserDirs::new().expect("User dirs");
        // let home_dir = user_dirs.home_dir();
        // if let Ok(stripped) = std::path::Path::new(fpath).strip_prefix(home_dir) {
        // format!("~/{}", stripped.to_string_lossy().to_string())
        // } else {
        // fpath.to_string()
        // }
        // })
        filter::simple_run(self.entries.iter().map(|entry| entry.fpath.as_str()), query)
    }

    /// Update or insert a new entry in a sorted way.
    pub fn upsert(&mut self, file: String) {
        match self
            .entries
            .iter()
            .position(|ref entry| entry.fpath.as_str() == file.as_str())
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

        if let Err(e) = self.write_to_disk() {
            log::error!("Failed to write the recent files to the disk: {:?}", e);
        }
    }

    fn write_to_disk(&self) -> Result<()> {
        if let Some(recent_files_json) = RECENT_FILES_JSON_PATH.as_deref() {
            utility::create_or_overwrite(
                recent_files_json,
                serde_json::to_string(self)?.as_bytes(),
            )?;
        }
        Ok(())
    }
}
