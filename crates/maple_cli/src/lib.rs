mod app;
mod cache;
mod datastore;
mod dumb_analyzer;
mod logger;
mod paths;
mod process;
mod recent_files;
mod stdio_server;
mod tools;
mod utils;
mod previewer;

pub mod command;
/// Re-exports.
pub use {
    anyhow::{Context, Result},
    app::{Cmd, Maple},
    filter::{subprocess, Source},
    icon::IconPainter,
    structopt::StructOpt,
};
