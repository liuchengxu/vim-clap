use icon::IconPainter;
use structopt::clap::AppSettings;
use structopt::StructOpt;

pub mod blines;
pub mod cache;
pub mod exec;
pub mod filter;
pub mod grep;
pub mod helptags;
pub mod rpc;
pub mod tags;

#[derive(StructOpt, Debug)]
pub enum Cmd {
    /// Display the current version
    #[structopt(name = "version")]
    Version,
    /// Fuzzy filter the input
    #[structopt(name = "filter")]
    Filter(crate::cmd::filter::Filter),
    /// Execute the grep command to avoid the escape issue
    #[structopt(name = "grep")]
    Grep(crate::cmd::grep::Grep),
    /// Start the stdio-based service, currently there is only filer support.
    #[structopt(name = "rpc")]
    RPC,
    /// Start the forerunner job of grep.
    #[structopt(name = "ripgrep-forerunner")]
    RipGrepForerunner(crate::cmd::grep::RipGrepForerunner),
    /// Execute the command
    #[structopt(name = "exec")]
    Exec(crate::cmd::exec::Exec),
    #[structopt(name = "blines")]
    Blines(crate::cmd::blines::Blines),
    #[structopt(name = "helptags")]
    Helptags(crate::cmd::helptags::Helptags),
    #[structopt(name = "cache")]
    Cache(crate::cmd::cache::Cache),
    #[structopt(name = "tags")]
    Tags(crate::cmd::tags::Tags),
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

    /// Do not use the cached file for exec subcommand.
    #[structopt(long = "no-cache")]
    pub no_cache: bool,

    /// Prepend an icon for item of files and grep provider, valid only when --number is used.
    #[structopt(short, long, possible_values = &IconPainter::variants(), case_insensitive = true)]
    pub icon_painter: Option<IconPainter>,

    #[structopt(subcommand)]
    pub command: Cmd,
}
