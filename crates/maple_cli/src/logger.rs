use anyhow::Result;
use log::{debug, LevelFilter};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use std::path::Path;

pub fn init<P: AsRef<Path>>(log_path: P) -> Result<()> {
    let encoder = PatternEncoder::new(
        "{date(%Y-%m-%d %H:%M:%S)} {level} {thread} {file}:{line} {message}{n}",
    );

    // Logging to log file.
    let log_file = FileAppender::builder()
        // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
        .encoder(Box::new(encoder))
        .append(false)
        .build(log_path.as_ref())?;

    // Log Trace level output to file where trace is the default level
    // and the programmatically specified level to stderr.
    let config = Config::builder()
        .appender(Appender::builder().build("vim-clap", Box::new(log_file)))
        .build(
            Root::builder()
                .appender("vim-clap")
                .build(LevelFilter::Debug),
        )?;

    // Use this to change log levels at runtime.
    // This means you can change the default log level to trace
    // if you are trying to debug an issue and need more logs on then turn it off
    // once you are done.
    let _handle = log4rs::init_config(config)?;
    debug!(
        "Initialize the logging system, log file: {}",
        log_path.as_ref().display()
    );
    Ok(())
}
