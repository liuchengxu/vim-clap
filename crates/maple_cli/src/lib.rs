mod app;
mod dumb_analyzer;
mod logger;
mod process;
mod stdio_server;
mod tools;

pub mod command;
/// Re-exports.
pub use {
    anyhow::{Context, Result},
    app::{Cmd, Maple},
    filter::{subprocess, Source},
    icon::IconPainter,
    structopt::StructOpt,
};
