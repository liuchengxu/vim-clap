mod forerunner;
mod live_grep;

use crate::app::Args;
use anyhow::Result;
use clap::Parser;
use filter::{ParallelSource, SequentialSource};
use maple_core::tools::rg::{refresh_cache, rg_shell_command};
use matcher::MatchScope;
use std::path::PathBuf;
use subprocess::Exec;

pub use self::forerunner::RipGrepForerunner;
pub use self::live_grep::LiveGrep;

// Ref https://github.com/liuchengxu/vim-clap/issues/533
// Now `.` is pushed to the end for all platforms due to https://github.com/liuchengxu/vim-clap/issues/711.
pub const RG_EXEC_CMD: &str =
    "rg --column --line-number --no-heading --color=never --smart-case '' .";

#[derive(Parser, Debug, Clone)]
pub struct Grep {
    /// Specify the query string for GREP_CMD.
    #[clap(index = 1)]
    grep_query: String,

    /// Read input from a cached grep tempfile.
    ///
    /// Only absolute file path is supported.
    #[clap(long, value_parser)]
    input: Option<PathBuf>,

    /// Specify the working directory of CMD.
    #[clap(long, value_parser)]
    cmd_dir: Option<PathBuf>,

    /// Recreate the grep cache.
    ///
    /// Only intended for the test purpose.
    #[clap(long)]
    refresh_cache: bool,

    /// Run the filter in parallel.
    ///
    /// Deprecated.
    #[clap(long)]
    par_run: bool,

    /// Use the builtin searching implementation on top of libripgrep instead of the rg executable.
    #[clap(long)]
    lib_ripgrep: bool,
}

impl Grep {
    pub async fn run(&self, args: Args) -> Result<()> {
        if self.refresh_cache {
            let dir = match self.cmd_dir {
                Some(ref dir) => dir.clone(),
                None => std::env::current_dir()?,
            };
            println!("Recreating the grep cache for {}", dir.display());
            refresh_cache(&dir)?;
            return Ok(());
        }

        if self.lib_ripgrep {
            let dir = match self.cmd_dir {
                Some(ref dir) => dir.clone(),
                None => std::env::current_dir()?,
            };

            let clap_matcher = matcher::MatcherBuilder::new().build(self.grep_query.clone().into());

            let search_result =
                maple_core::searcher::grep::cli_search(vec![dir], clap_matcher).await;

            println!(
                "total_matched: {:?}, total_processed: {:?}",
                search_result.total_matched, search_result.total_processed
            );

            return Ok(());
        }

        let maybe_usable_cache = self.usable_cache(&args);

        let filter_context = args.into_filter_context().match_scope(MatchScope::GrepLine);

        if self.par_run {
            let par_source = if let Some(cache) = maybe_usable_cache {
                ParallelSource::File(cache)
            } else if let Some(ref tempfile) = self.input {
                ParallelSource::File(tempfile.clone())
            } else if let Some(ref dir) = self.cmd_dir {
                ParallelSource::Exec(Box::new(Exec::shell(RG_EXEC_CMD).cwd(dir)))
            } else {
                ParallelSource::Exec(Box::new(Exec::shell(RG_EXEC_CMD)))
            };

            // TODO: Improve the responsiveness of ripgrep as it can emit the items after some time.
            // When running the command below, a few seconds before showing the progress, might be
            // mitigated by using the libripgrep instead of using the rg executable.
            // time /home/xlc/.vim/plugged/vim-clap/target/release/maple --icon=Grep --no-cache --number 136 --winwidth 122 --case-matching smart grep srlss --cmd-dir /home/xlc/src/github.com/subspace/subspace --par-run
            filter::par_dyn_run(&self.grep_query, filter_context, par_source)?;
        } else {
            let source: SequentialSource<std::iter::Empty<_>> =
                if let Some(cache) = maybe_usable_cache {
                    SequentialSource::File(cache)
                } else if let Some(ref tempfile) = self.input {
                    SequentialSource::File(tempfile.clone())
                } else if let Some(ref dir) = self.cmd_dir {
                    Exec::shell(RG_EXEC_CMD).cwd(dir).into()
                } else {
                    Exec::shell(RG_EXEC_CMD).into()
                };

            filter::dyn_run(&self.grep_query, filter_context, source)?;
        }

        Ok(())
    }

    fn usable_cache(&self, args: &Args) -> Option<PathBuf> {
        if !args.no_cache {
            if let Some(digest) = self
                .cmd_dir
                .as_ref()
                .map(rg_shell_command)
                .and_then(|shell_cmd| shell_cmd.cache_digest())
            {
                return Some(digest.cached_path);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;
    use maple_core::process::tokio::TokioCommand;
    use maple_core::process::ShellCommand;
    use maple_core::tools::rg::RgTokioCommand;
    use std::path::Path;
    use std::time::Instant;

    // 3X faster than the deprecated version.
    async fn create_cache_deprecated(dir: &Path) -> (usize, PathBuf) {
        let inner = ShellCommand::new(RG_EXEC_CMD.into(), dir.to_path_buf());

        let lines = TokioCommand::new(RG_EXEC_CMD)
            .current_dir(dir)
            .lines()
            .await
            .unwrap();

        let total = lines.len();
        let lines = lines.into_iter().join("\n");
        let cache_path = inner.write_cache(total, lines.as_bytes()).unwrap();

        (total, cache_path)
    }

    #[tokio::test]
    async fn test_create_grep_cache_async() {
        let dir = std::env::current_dir().unwrap();

        let now = Instant::now();
        let res = create_cache_deprecated(&dir).await;
        println!("Cache creation result(old): {res:?}");
        let elapsed = now.elapsed();
        println!("Elapsed: {elapsed:.3?}");

        let now = Instant::now();
        let rg_cmd = RgTokioCommand::new(dir);
        let res = rg_cmd.create_cache().await.unwrap();
        println!(
            "Cache creation result(new): {:?}",
            (res.total, res.cached_path)
        );
        let elapsed = now.elapsed();
        println!("Elapsed: {elapsed:.3?}");
    }
}
