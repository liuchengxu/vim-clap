use std::cmp::Ordering;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use std::time::Instant;

use once_cell::sync::{Lazy, OnceCell};

use crate::stdio_server::Message;

const HOUR: u64 = 3600;
const DAY: u64 = HOUR * 24;
const WEEK: u64 = DAY * 7;

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug, Eq)]
pub struct FrecentEntry {
    /// Absolute file path.
    pub fpath: String,
    /// Time of last visit.
    pub last_visit: Instant,
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
            last_visit: Instant::now(),
            visits: 1u64,
            frecent: 1u64,
        }
    }

    pub fn update_frecent(&mut self, at: Option<Instant>) {
        let now = at.unwrap_or_else(Instant::now);

        let duration = (now - self.last_visit).as_secs();

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
#[derive(Clone, Debug)]
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
                entry.last_visit = Instant::now();
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
        let now = Instant::now();

        for entry in self.entries.iter_mut() {
            entry.update_frecent(Some(now));
        }
    }
}

static RECENT_FILES: Lazy<Mutex<SortedRecentFiles>> =
    Lazy::new(|| Mutex::new(initialize_recent_files()));

fn initialize_recent_files() -> SortedRecentFiles {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "vim", "Vim Clap") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir).expect("Failed to create data directory");
        log::debug!("data_dir: {:?}", data_dir);

        let mut recent_files_json = data_dir.to_path_buf();
        recent_files_json.push("recent_files.json");

        if recent_files_json.exists() {
            // todo!("load from disk")
            Default::default()
        } else {
            Default::default()
        }
    }

    Default::default()
}

pub fn note_recent_file(msg: Message) {
    log::debug!("msg: {:?}", msg);
    let file = msg.get_string_unsafe("file");

    if file.is_empty() {
        return;
    }

    let mut recent_files = RECENT_FILES.lock().unwrap();
    recent_files.upsert(file);
    // let mut recent_files = RECENT_FILES::get();
    // recent_files.upsert(file);

    log::debug!("recent_files: {:?}", recent_files);
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_time() {}
}
