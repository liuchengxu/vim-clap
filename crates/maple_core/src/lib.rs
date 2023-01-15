mod cache;
pub mod config;
pub mod datastore;
pub mod dirs;
pub mod find_usages;
pub mod helptags;
pub mod paths;
mod previewer;
pub mod process;
mod recent_files;
pub mod searcher;
pub mod stdio_server;
pub mod tools;
pub mod utils;

/// For benchmarks.
pub use self::cache::find_largest_cache_digest;

use chrono::{DateTime, Utc};

pub type UtcTime = DateTime<Utc>;
