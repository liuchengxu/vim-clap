use anyhow::Result;
use clap::{AppSettings, Parser};

use filter::FilterContext;
use icon::Icon;
use types::CaseMatching;

use crate::command;

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
    #[clap(name = "cache")]
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
    Upgrade(upgrade::Upgrade),
}

#[derive(Parser, Debug)]
#[clap(name = "maple")]
#[clap(global_setting(AppSettings::DisableVersionFlag))]
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
    #[clap(long, parse(from_str), default_value = "smartcase")]
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
                if let Some(ref log_path) = self.log {
                    crate::logger::init(log_path)?;
                } else if let Ok(log_path) = std::env::var("VIM_CLAP_LOG_PATH") {
                    crate::logger::init(log_path)?;
                }

                crate::stdio_server::run_forever(std::io::BufReader::new(std::io::stdin()));
                // crate::stdio_server::start()?;
            }
        };
        Ok(())
    }
}
