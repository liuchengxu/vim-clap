#![allow(unused)]

mod app;
mod cache;
mod datastore;
mod dumb_analyzer;
mod logger;
mod paths;
mod previewer;
mod process;
mod recent_files;
mod stdio_server;
mod tools;
mod utils;

pub mod command;
/// Re-exports.
pub use {
    anyhow::{Context, Result},
    app::{Cmd, Maple},
    filter::{subprocess, Source},
    icon::{Icon, IconKind},
    structopt::StructOpt,
};
