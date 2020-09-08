use anyhow::Result;
use icon::IconPainter;
use structopt::clap::AppSettings;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum Cmd {
    /// Display the current version
    #[structopt(name = "version")]
    Version,
    /// Start the stdio-based service, currently there is only filer support.
    #[structopt(name = "rpc")]
    RPC,
    /// Execute the grep command to avoid the escape issue
    #[structopt(name = "grep")]
    Grep(crate::cmd::grep::Grep),
    /// Execute the shell command.
    #[structopt(name = "exec")]
    Exec(crate::cmd::exec::Exec),
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
    /// Print the top NUM of filtered items.
    ///
    /// The returned JSON has three fields:
    ///   - total: total number of initial filtered result set.
    ///   - lines: text lines used for displaying directly.
    ///   - indices: the indices of matched elements per line, used for the highlight purpose.
    #[structopt(short = "n", long = "number", name = "NUM")]
    pub number: Option<usize>,

    /// Width of clap window.
    #[structopt(short = "w", long = "winwidth")]
    pub winwidth: Option<usize>,

    /// Prepend an icon for item of files and grep provider, valid only when --number is used.
    #[structopt(short, long, possible_values = &IconPainter::variants(), case_insensitive = true)]
    pub icon_painter: Option<IconPainter>,

    /// Do not use the cached file for exec subcommand.
    #[structopt(long = "no-cache")]
    pub no_cache: bool,

    /// Enable the logging system.
    #[structopt(long = "log", parse(from_os_str))]
    pub log: Option<std::path::PathBuf>,

    #[structopt(subcommand)]
    pub command: Cmd,
}

impl Maple {
    pub fn run(self) -> Result<()> {
        if let Some(ref log_path) = self.log {
            crate::logger::init(log_path)?;
        } else if let Ok(log_path) = std::env::var("VIM_CLAP_LOG_PATH") {
            crate::logger::init(log_path)?;
        }
        match self.command {
            Cmd::Version | Cmd::Upgrade(_) => unreachable!(),
            Cmd::Helptags(helptags) => helptags.run()?,
            Cmd::Tags(tags) => tags.run(self.no_cache, self.icon_painter)?,
            Cmd::RPC => {
                stdio_server::run_forever(std::io::BufReader::new(std::io::stdin()));
            }
            Cmd::Blines(blines) => {
                blines.run(self.number, self.winwidth)?;
            }
            Cmd::RipGrepForerunner(rip_grep_forerunner) => {
                rip_grep_forerunner.run(self.number, self.icon_painter, self.no_cache)?
            }
            Cmd::Cache(cache) => cache.run()?,
            Cmd::Filter(filter) => {
                filter.run(self.number, self.winwidth, self.icon_painter)?;
            }
            Cmd::Exec(exec) => {
                exec.run(self.number, self.icon_painter, self.no_cache)?;
            }
            Cmd::Grep(grep) => {
                grep.run(self.number, self.winwidth, self.icon_painter, self.no_cache)?;
            }
        }
        Ok(())
    }
}
