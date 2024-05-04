use crate::app::Args;
use anyhow::{anyhow, Result};
use clap::Parser;
use maple_core::stdio_server::ConfigError;
use std::io::IsTerminal;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

/// Starts a RPC service using stdio.
#[derive(Parser, Debug, Clone)]
pub struct Rpc;

impl Rpc {
    pub async fn run(&self, args: Args) -> Result<()> {
        let (config, maybe_toml_err) =
            maple_config::load_config_on_startup(args.config_file.clone());

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
                // Remove the old log file automatically if its size exceeds 8MiB.
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

            let mut env_filter = EnvFilter::from_default_env();
            let mut log_target_err = String::new();
            let log_target = &config.log.log_target;
            if !log_target.is_empty() {
                // `maple_core::stdio_server=debug,rpc=trace`
                for target in log_target.split(',') {
                    match target.trim().parse() {
                        Ok(directive) => {
                            env_filter = env_filter.add_directive(directive);
                        }
                        Err(err) => {
                            log_target_err
                                .push_str(&format!("Bad invalid log-target `{target}`: {err:?}.",));
                        }
                    }
                }
            }

            let subscriber = tracing_subscriber::FmtSubscriber::builder()
                .with_span_events(FmtSpan::FULL)
                .with_max_level(max_level)
                .with_env_filter(env_filter)
                .with_line_number(true)
                .with_writer(non_blocking)
                .with_ansi(std::io::stdout().is_terminal())
                .finish();

            tracing::subscriber::set_global_default(subscriber)?;

            maple_core::stdio_server::start(ConfigError {
                maybe_toml_err,
                maybe_log_target_err: if log_target_err.is_empty() {
                    None
                } else {
                    Some(log_target_err)
                },
            })
            .await;
        } else {
            maple_core::stdio_server::start(ConfigError {
                maybe_toml_err,
                maybe_log_target_err: None,
            })
            .await;
        }

        Ok(())
    }
}
