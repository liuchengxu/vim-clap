use anyhow::Result;
use clap::Parser;

use filter::FilterContext;
use icon::Icon;
use types::CaseMatching;

use crate::command;

#[derive(Parser, Debug)]
pub enum RunCmd {
    /// Start the stdio-based service, currently there is only filer support.
    #[clap(name = "rpc")]
    Rpc(command::rpc::Rpc),
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

    /// Specify the number of threads used in the rayon global thread pool.
    ///
    /// By default, the number of physical cores will be used if the environment variable
    /// `RAYON_NUM_THREADS` also does not exist.
    #[clap(long)]
    pub rayon_num_threads: Option<usize>,

    /// Enable the logging system.
    #[clap(long, parse(from_os_str))]
    pub log: Option<std::path::PathBuf>,

    /// Specify the path of the config file.
    #[clap(long, parse(from_os_str))]
    pub config_file: Option<std::path::PathBuf>,
}

impl Params {
    pub fn into_filter_context(self) -> FilterContext {
        FilterContext::default()
            .icon(self.icon)
            .number(self.number)
            .winwidth(self.winwidth)
    }
}

impl RunCmd {
    pub async fn run(self, params: Params) -> Result<()> {
        // Set the global thread pool to use the number of physical cores if `RAYON_NUM_THREADS`
        // does not exist.
        //
        // > By default, Rayon uses the same number of threads as the number of CPUs available.
        // > Note that on systems with hyperthreading enabled this equals the number of logical cores
        // > and not the physical ones.
        //
        // It's preferred to just use the physical cores instead of the logical cores based on
        // the personal experience, observed by the performance regression (up to 20%) after enabling
        // the virtualization on my AMD 5900x which uses the logical cores instead of the physical ones.
        let num_threads = params.rayon_num_threads.unwrap_or_else(|| {
            std::env::var("RAYON_NUM_THREADS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(num_cpus::get_physical)
        });
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build_global()
            .expect("Failed to configure the rayon global thread pool");

        match self {
            Self::Blines(blines) => blines.run(params),
            Self::Cache(cache) => cache.run(),
            Self::Ctags(ctags) => ctags.run(params),
            Self::DumbJump(dumb_jump) => dumb_jump.run(),
            Self::Exec(exec) => exec.run(params),
            Self::Filter(filter) => filter.run(params),
            Self::Grep(grep) => grep.run(params),
            Self::Gtags(gtags) => gtags.run(params),
            Self::Helptags(helptags) => helptags.run(),
            Self::RipGrepForerunner(rip_grep_forerunner) => rip_grep_forerunner.run(params),
            Self::Rpc(rpc) => rpc.run(params).await,
        }
    }
}
