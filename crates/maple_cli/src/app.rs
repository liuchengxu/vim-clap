use anyhow::Result;
use structopt::{clap::AppSettings, StructOpt};

use filter::FilterContext;
use icon::IconPainter;

#[derive(StructOpt, Debug)]
pub enum Cmd {
    /// Display the current version
    #[structopt(name = "version")]
    Version,
    /// Start the stdio-based service, currently there is only filer support.
    #[structopt(name = "rpc")]
    Rpc,
    /// Execute the grep command to avoid the escape issue
    #[structopt(name = "grep")]
    Grep(crate::cmd::grep::Grep),
    /// Execute the shell command.
    #[structopt(name = "exec")]
    Exec(crate::cmd::exec::Exec),
    /// Dumb jump.
    #[structopt(name = "dumb-jump")]
    DumbJump(crate::cmd::dumb_jump::DumbJump),
    /// Generate the project-wide tags using ctags.
    #[structopt(name = "tags")]
    Tags(crate::cmd::tags::Tags),
    /// Interact with the cache info.
    #[structopt(name = "cache")]
    Cache(crate::cmd::cache::Cache),
    /// Fuzzy filter the input.
    #[structopt(name = "filter")]
    Filter(crate::cmd::filter::Filter),
    /// Filter against current Vim buffer.
    #[structopt(name = "blines")]
    Blines(crate::cmd::blines::Blines),
    /// Generate vim help tags.
    #[structopt(name = "helptags")]
    Helptags(crate::cmd::helptags::Helptags),
    /// Start the forerunner job of grep.
    #[structopt(name = "ripgrep-forerunner")]
    RipGrepForerunner(crate::cmd::grep::RipGrepForerunner),
    /// Retrive the latest remote release info.
    #[structopt(name = "upgrade")]
    Upgrade(upgrade::Upgrade),
}

#[derive(StructOpt, Debug)]
#[structopt(
  name = "maple",
  no_version,
  global_settings = &[AppSettings::DisableVersion]
)]
pub struct Maple {
    #[structopt(flatten)]
    pub params: Params,

    /// Enable the logging system.
    #[structopt(long = "log", parse(from_os_str))]
    pub log: Option<std::path::PathBuf>,

    #[structopt(subcommand)]
    pub command: Cmd,
}

#[derive(StructOpt, Debug)]
pub struct Params {
    /// Print the top NUM of filtered items.
    ///
    /// The returned JSON has three fields:
    ///   - total: total number of initial filtered result set.
    ///   - lines: text lines used for displaying directly.
    ///   - indices: the indices of matched elements per line, used for the highlight purpose.
    #[structopt(long = "number", name = "NUM")]
    pub number: Option<usize>,

    /// Width of clap window.
    #[structopt(long = "winwidth")]
    pub winwidth: Option<usize>,

    /// Prepend an icon for item of files and grep provider, valid only when --number is used.
    #[structopt(long, possible_values = &IconPainter::variants(), case_insensitive = true)]
    pub icon_painter: Option<IconPainter>,

    /// Do not use the cached file for exec subcommand.
    #[structopt(long = "no-cache")]
    pub no_cache: bool,
}

impl Params {
    pub fn into_filter_context(self) -> FilterContext {
        FilterContext::default()
            .number(self.number)
            .winwidth(self.winwidth)
            .icon_painter(self.icon_painter)
    }
}

impl Maple {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Cmd::Version | Cmd::Upgrade(_) => unreachable!("Version and Upgrade are unusable"),
            Cmd::Exec(exec) => exec.run(self.params)?,
            Cmd::Grep(grep) => grep.run(self.params)?,
            Cmd::Tags(tags) => tags.run(self.params)?,
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
            }
        };
        Ok(())
    }
}
