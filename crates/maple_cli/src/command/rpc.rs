use anyhow::Result;
use clap::Parser;

use crate::app::Params;

/// Starts a RPC service using stdio.
#[derive(Parser, Debug, Clone)]
pub struct Rpc;

impl Rpc {
    pub async fn run(&self, params: Params) -> Result<()> {
        let maybe_log = if let Some(log_path) = params.log {
            Some(log_path)
        } else if let Ok(log_path) =
            std::env::var("VIM_CLAP_LOG_PATH").map(std::path::PathBuf::from)
        {
            Some(log_path)
        } else {
            None
        };

        if let Some(log_path) = maybe_log {
            if let Ok(metadata) = std::fs::metadata(&log_path) {
                if log_path.is_file() && metadata.len() > 8 * 1024 * 1024 {
                    std::fs::remove_file(&log_path)?;
                }
            }

            let file_name = log_path.file_name().expect("Invalid file name");
            let directory = log_path.parent().expect("A file must have a parent");

            let file_appender = tracing_appender::rolling::never(directory, file_name);
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            let subscriber = tracing_subscriber::FmtSubscriber::builder()
                .with_max_level(tracing::Level::TRACE)
                .with_line_number(true)
                .with_writer(non_blocking)
                .finish();

            tracing::subscriber::set_global_default(subscriber)?;

            crate::stdio_server::run_forever(std::io::BufReader::new(std::io::stdin())).await;
        } else {
            crate::stdio_server::run_forever(std::io::BufReader::new(std::io::stdin())).await;
        }

        Ok(())
    }
}
