use std::cmp::Ordering;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use chrono::prelude::*;
use once_cell::sync::{Lazy, OnceCell};
use serde::{Deserialize, Serialize};

use crate::stdio_server::Message;

const HOUR: i64 = 3600;
const DAY: i64 = HOUR * 24;
const WEEK: i64 = DAY * 7;

type UtcTime = DateTime<Utc>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SortPreference {
    /// Sort by the visit time.
    Frequency,
    /// Sort by the number of visits.
    Recency,
    ///
    Frecency,
}

impl Default for SortPreference {
    fn default() -> Self {
        Self::Frecency
    }
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize)]
pub struct FrecentEntry {
    /// Absolute file path.
    pub fpath: String,
    /// Time of last visit.
    pub last_visit: UtcTime,
    /// Number of total visits.
    pub visits: u64,
    /// Score based on https://en.wikipedia.org/wiki/Frecency
    pub frecent: u64,
}

impl PartialEq for FrecentEntry {
    fn eq(&self, other: &Self) -> bool {
        self.fpath == other.fpath
    }
}

impl PartialOrd for FrecentEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some((self.frecent, self.visits, self.last_visit).cmp(&(
            other.frecent,
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
    pub fn new(fpath: String) -> Self {
        Self {
            fpath,
            last_visit: Utc::now(),
            visits: 1u64,
            frecent: 1u64,
        }
    }

    pub fn update_frecent(&mut self, at: Option<UtcTime>) {
        let now = at.unwrap_or_else(Utc::now);

        let duration = now.signed_duration_since(self.last_visit).num_seconds();

        self.frecent = if duration < HOUR {
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

// Write to disk in Drop.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SortedRecentFiles {
    pub max_entries: u64,
    pub sort_preference: SortPreference,
    pub entries: Vec<FrecentEntry>,
}

impl Default for SortedRecentFiles {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            sort_preference: Default::default(),
            entries: Default::default(),
        }
    }
}

impl Drop for SortedRecentFiles {
    fn drop(&mut self) {
        log::debug!("------------ calling Drop for SortedRecentFiles");
        if let Err(e) = self.write_to_disk() {
            log::error!("Error when writing MRU back to the disk: {}", e);
        }
    }
}

fn persistent_recent_files_path() -> Result<PathBuf> {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "vim", "Vim Clap") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;
        log::debug!("data_dir: {:?}", data_dir);

        let mut recent_files_json = data_dir.to_path_buf();
        recent_files_json.push("recent_files.json");

        return Ok(recent_files_json);
    }

    Err(anyhow!("Can not fetch the Vim Clap project directory"))
}

impl SortedRecentFiles {
    /// Add new entry in a sorted way.
    pub fn upsert(&mut self, file: String) {
        match self
            .entries
            .iter()
            .position(|ref entry| entry.fpath.as_str() == file.as_str())
        {
            Some(pos) => {
                let entry = &mut self.entries[pos];
                log::debug!("The entry already exists: {:?}", entry);
                entry.last_visit = Utc::now();
                entry.visits += 1;
                entry.update_frecent(None);
            }
            None => {
                let entry = FrecentEntry::new(file);
                log::debug!("Inserting a new entry: {:?}", entry);
                self.entries.push(entry);
            }
        }

        self.entries.sort();

        // Truncate the list
    }

    pub fn update_frecent(&mut self) {
        let now = Utc::now();

        for entry in self.entries.iter_mut() {
            entry.update_frecent(Some(now));
        }
    }

    pub fn write_to_disk(&self) -> Result<()> {
        if let Ok(recent_files_json) = persistent_recent_files_path() {
            // Overwrite it.
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(recent_files_json)?;

            f.write_all(serde_json::to_string(self)?.as_bytes())?;
            f.flush()?;
        }
        Ok(())
    }
}

static RECENT_FILES: Lazy<Mutex<SortedRecentFiles>> =
    Lazy::new(|| Mutex::new(initialize_recent_files()));

fn initialize_recent_files() -> SortedRecentFiles {
    persistent_recent_files_path()
        .map(|recent_files_json| {
            if recent_files_json.exists() {
                // todo!("load from disk")
                Default::default()
            } else {
                Default::default()
            }
        })
        .unwrap_or_default()
}

pub fn note_recent_file(msg: Message) {
    log::debug!("msg: {:?}", msg);
    let file = msg.get_string_unsafe("file");

    if file.is_empty() {
        return;
    }

    let mut recent_files = RECENT_FILES.lock().unwrap();
    recent_files.upsert(file);

    log::debug!("recent_files: {:?}", recent_files);
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_time() {}
}
