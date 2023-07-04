use std::io::IsTerminal;

use crate::app::Args;
use anyhow::{anyhow, Result};
use clap::Parser;

/// Starts a RPC service using stdio.
#[derive(Parser, Debug, Clone)]
pub struct Rpc;

impl Rpc {
    pub async fn run(&self, args: Args) -> Result<()> {
        let (config, config_err) =
            maple_core::config::load_config_on_startup(args.config_file.clone());

        let maybe_log = if let Some(log_path) = args.log {
            Some(log_path)
        } else if let Ok(log_path) =
            std::env::var("VIM_CLAP_LOG_PATH").map(std::path::PathBuf::from)
        {
            Some(log_path)
        } else {
            config.log.log_file.as_ref().map(std::path::PathBuf::from)
        };

        if let Some(log_path) = maybe_log {
            if let Ok(metadata) = std::fs::metadata(&log_path) {
                if log_path.is_file() && metadata.len() > 8 * 1024 * 1024 {
                    std::fs::remove_file(&log_path)?;
                }
            }

            let file_name = log_path
                .file_name()
                .ok_or_else(|| anyhow!("no file name in {log_path:?}"))?;

            let directory = log_path
                .parent()
                .ok_or_else(|| anyhow!("{log_path:?} has no parent"))?;

            let file_appender = tracing_appender::rolling::never(directory, file_name);
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            let max_level = config
                .log
                .max_level
                .parse()
                .unwrap_or(tracing::Level::DEBUG);

            let subscriber = tracing_subscriber::FmtSubscriber::builder()
                .with_max_level(max_level)
                .with_line_number(true)
                .with_writer(non_blocking)
                .with_ansi(std::io::stdout().is_terminal())
                .finish();

            tracing::subscriber::set_global_default(subscriber)?;

            maple_core::stdio_server::start(config_err).await;
        } else {
            maple_core::stdio_server::start(config_err).await;
        }

        Ok(())
    }
}
