use std::path::PathBuf;

use fuzzy_filter::Algo;
use structopt::clap::AppSettings;
use structopt::StructOpt;

pub mod blines;
pub mod cache;
pub mod exec;
pub mod filter;
pub mod grep;
pub mod helptags;
pub mod rpc;

#[derive(StructOpt, Debug)]
pub enum Cmd {
    /// Display the current version
    #[structopt(name = "version")]
    Version,
    /// Fuzzy filter the input
    #[structopt(name = "filter")]
    Filter {
        /// Initial query string
        #[structopt(index = 1, short, long)]
        query: String,

        /// Filter algorithm
        #[structopt(short, long, possible_values = &Algo::variants(), case_insensitive = true)]
        algo: Option<Algo>,

        /// Shell command to produce the whole dataset that query is applied on.
        #[structopt(short, long)]
        cmd: Option<String>,

        /// Working directory of shell command.
        #[structopt(short, long)]
        cmd_dir: Option<String>,

        /// Synchronous filtering, returns after the input stream is complete.
        #[structopt(short, long)]
        sync: bool,

        /// Read input from a file instead of stdin, only absolute file path is supported.
        #[structopt(long = "input", parse(from_os_str))]
        input: Option<PathBuf>,
    },
    /// Execute the grep command to avoid the escape issue
    #[structopt(name = "grep")]
    Grep {
        /// Specify the grep command to run, normally rg will be used.
        ///
        /// Incase of clap can not reconginize such option: --cmd "rg --vimgrep ... "fn ul"".
        ///                                                       |-----------------|
        ///                                                   this can be seen as an option by mistake.
        #[structopt(index = 1, short, long)]
        grep_cmd: String,

        /// Specify the query string for GREP_CMD.
        #[structopt(index = 2, short, long)]
        grep_query: String,

        /// Delegate to -g option of rg
        #[structopt(short = "g", long = "glob")]
        glob: Option<String>,

        /// Specify the working directory of CMD
        #[structopt(long = "cmd-dir", parse(from_os_str))]
        cmd_dir: Option<PathBuf>,

        /// Synchronous filtering, returns after the input stream is complete.
        #[structopt(short, long)]
        sync: bool,

        /// Read input from a cached grep tempfile, only absolute file path is supported.
        #[structopt(long = "input", parse(from_os_str))]
        input: Option<PathBuf>,
    },
    #[structopt(name = "rpc")]
    RPC,
    #[structopt(name = "ripgrep-forerunner")]
    RipgrepForerunner {
        /// Specify the working directory of CMD
        #[structopt(long = "cmd-dir", parse(from_os_str))]
        cmd_dir: Option<PathBuf>,
    },
    /// Execute the command
    #[structopt(name = "exec")]
    Exec(crate::cmd::exec::Exec),
    #[structopt(name = "blines")]
    Blines(crate::cmd::blines::Blines),
    #[structopt(name = "helptags")]
    Helptags(crate::cmd::helptags::Helptags),
    #[structopt(name = "cache")]
    Cache(crate::cmd::cache::Cache),
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
    #[structopt(long = "enable-icon")]
    pub enable_icon: bool,

    /// Do not use the cached file for exec subcommand.
    #[structopt(long = "no-cache")]
    pub no_cache: bool,

    #[structopt(subcommand)]
    pub command: Cmd,
}
