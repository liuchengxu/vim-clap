mod app;
mod cache;
mod config;
mod datastore;
mod find_usages;
mod paths;
mod previewer;
mod process;
mod recent_files;
mod stdio_server;
mod tools;
mod utils;

/// For benchmarks.
pub mod command;

/// For benchmarks.
pub use self::cache::find_largest_cache_digest;
pub use self::utils::count_lines;

/// Re-exports.
pub use app::{Params, RunCmd};
