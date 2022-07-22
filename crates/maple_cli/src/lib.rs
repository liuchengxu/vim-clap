mod app;
mod cache;
// mod config;
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
/// Re-exports.
pub use {
    anyhow::{Context, Result},
    app::{Cmd, Maple},
    clap::Parser,
    datastore::CACHE_INFO_IN_MEMORY,
};
