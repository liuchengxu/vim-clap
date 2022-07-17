use anyhow::Result;
use clap::{AppSettings, Parser};

use filter::FilterContext;
use icon::Icon;
use types::CaseMatching;

use crate::command;

/// This command is only invoked when user uses the prebuilt binary, more specifically, exe in
/// vim-clap/bin/maple.
#[derive(Parser, Debug, Clone)]
pub struct Upgrade {
    /// Download if the local version mismatches the latest remote version.
    #[clap(long)]
    download: bool,
    /// Disable the downloading progress_bar
    #[clap(long)]
    no_progress_bar: bool,
}

impl Upgrade {
    pub async fn run(self, local_tag: &str) -> Result<()> {
        upgrade::Upgrade::new(self.download, self.no_progress_bar)
            .run(local_tag)
            .await
            .map_err(Into::into)
    }
}

#[derive(Parser, Debug)]
pub enum Cmd {
    /// Display the current version
    #[clap(name = "version")]
    Version,
    /// Start the stdio-based service, currently there is only filer support.
    #[clap(name = "rpc")]
    Rpc,
    /// Execute the grep command to avoid the escape issue
    #[clap(name = "grep")]
    Grep(command::grep::Grep),
    #[clap(name = "gtags")]
    Gtags(command::gtags::Gtags),
    /// Execute the shell command.
    #[clap(name = "exec")]
    Exec(command::exec::Exec),
    /// Dumb jump.
    #[clap(name = "dumb-jump")]
    DumbJump(command::dumb_jump::DumbJump),
    /// Generate the project-wide tags using ctags.
    #[clap(name = "ctags", subcommand)]
    Ctags(command::ctags::Ctags),
    /// Interact with the cache info.
    #[clap(name = "cache", subcommand)]
    Cache(command::cache::Cache),
    /// Fuzzy filter the input.
    #[clap(name = "filter")]
    Filter(command::filter::Filter),
    /// Filter against current Vim buffer.
    #[clap(name = "blines")]
    Blines(command::blines::Blines),
    /// Generate vim help tags.
    #[clap(name = "helptags")]
    Helptags(command::helptags::Helptags),
    /// Start the forerunner job of grep.
    #[clap(name = "ripgrep-forerunner")]
    RipGrepForerunner(command::grep::RipGrepForerunner),
    /// Retrive the latest remote release info.
    #[clap(name = "upgrade")]
    Upgrade(Upgrade),
}

#[derive(Parser, Debug)]
#[clap(name = "maple")]
#[clap(global_setting(AppSettings::NoAutoVersion))]
pub struct Maple {
    #[clap(flatten)]
    pub params: Params,

    /// Enable the logging system.
    #[clap(long, parse(from_os_str))]
    pub log: Option<std::path::PathBuf>,

    #[clap(subcommand)]
    pub command: Cmd,
}

#[derive(Parser, Debug)]
pub struct Params {
    /// Print the top NUM of filtered items.
    ///
    /// The returned JSON has three fields:
    ///   - total: total number of initial filtered result set.
    ///   - lines: text lines used for displaying directly.
    ///   - indices: the indices of matched elements per line, used for the highlight purpose.
    #[clap(long, name = "NUM")]
    pub number: Option<usize>,

    /// Width of clap window.
    #[clap(long)]
    pub winwidth: Option<usize>,

    /// Prepend an icon for item of files and grep provider, valid only when --number is used.
    #[clap(long, parse(from_str), default_value = "unknown")]
    pub icon: Icon,

    /// Case matching strategy.
    #[clap(long, parse(from_str), default_value = "smart")]
    pub case_matching: CaseMatching,

    /// Do not use the cached file for exec subcommand.
    #[clap(long)]
    pub no_cache: bool,
}

impl Params {
    pub fn into_filter_context(self) -> FilterContext {
        FilterContext::default()
            .icon(self.icon)
            .number(self.number)
            .winwidth(self.winwidth)
    }
}

impl Maple {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Cmd::Version | Cmd::Upgrade(_) => unreachable!("Version and Upgrade are unusable"),
            Cmd::Exec(exec) => exec.run(self.params)?,
            Cmd::Grep(grep) => grep.run(self.params)?,
            Cmd::Ctags(ctags) => ctags.run(self.params)?,
            Cmd::Gtags(gtags) => gtags.run(self.params)?,
            Cmd::Cache(cache) => cache.run()?,
            Cmd::Blines(blines) => blines.run(self.params)?,
            Cmd::Filter(filter) => filter.run(self.params)?,
            Cmd::Helptags(helptags) => helptags.run()?,
            Cmd::DumbJump(dumb_jump) => dumb_jump.run().await?,
            Cmd::RipGrepForerunner(rip_grep_forerunner) => rip_grep_forerunner.run(self.params)?,
            Cmd::Rpc => {
                let maybe_log = if let Some(log_path) = self.log {
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

                    crate::stdio_server::run_forever(std::io::BufReader::new(std::io::stdin()));
                } else {
                    crate::stdio_server::run_forever(std::io::BufReader::new(std::io::stdin()));
                }
            }
        };
        Ok(())
    }
}
