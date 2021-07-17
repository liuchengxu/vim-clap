use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use chrono::prelude::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

const HOUR: i64 = 3600;
const DAY: i64 = HOUR * 24;
const WEEK: i64 = DAY * 7;

const JSON_FILENAME: &str = "recent_files.json";

const MAX_ENTRIES: u64 = 10_000;

fn persistent_recent_files_path() -> Result<PathBuf> {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "vim", "Vim Clap") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

        let mut recent_files_json = data_dir.to_path_buf();
        recent_files_json.push(JSON_FILENAME);

        return Ok(recent_files_json);
    }

    Err(anyhow!("Couldn't create the Vim Clap project directory"))
}

pub static JSON_PATH: Lazy<Option<PathBuf>> = Lazy::new(|| persistent_recent_files_path().ok());

fn read_recent_files_from_file<P: AsRef<Path>>(path: P) -> Result<SortedRecentFiles> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let recent_files = serde_json::from_reader(reader)?;
    Ok(recent_files)
}

pub static RECENT_FILES_IN_MEMORY: Lazy<Mutex<SortedRecentFiles>> =
    Lazy::new(|| Mutex::new(initialize_recent_files()));

fn initialize_recent_files() -> SortedRecentFiles {
    JSON_PATH
        .as_deref()
        .and_then(|recent_files_json| {
            if recent_files_json.exists() {
                read_recent_files_from_file(recent_files_json).ok()
            } else {
                None
            }
        })
        .unwrap_or_default()
}

type UtcTime = DateTime<Utc>;

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
        if let Some(recent_files_json) = JSON_PATH.as_deref() {
            // Overwrite it.
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(recent_files_json)?;

            f.write_all(serde_json::to_string(self)?.as_bytes())?;
            f.flush()?;
        }
        Ok(())
    }
}
