use crate::app::Params;
use crate::command::grep::RG_EXEC_CMD;
use anyhow::Result;
use clap::Parser;
use filter::{ParallelSource, SequentialSource};
use matcher::MatchScope;
use std::path::PathBuf;
use subprocess::Exec;

#[derive(Parser, Debug, Clone)]
pub struct Grep {
    /// Specify the query string for GREP_CMD.
    #[clap(index = 1, long)]
    grep_query: String,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[clap(long, parse(from_os_str))]
    input: Option<PathBuf>,

    /// Specify the working directory of CMD
    #[clap(long, parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Recreate the cache, only intended for the test purpose.
    #[clap(long)]
    refresh_cache: bool,

    #[clap(long)]
    par_run: bool,
}

impl Grep {
    pub fn run(&self, params: Params) -> Result<()> {
        if self.refresh_cache {
            let dir = match self.cmd_dir {
                Some(ref dir) => dir.clone(),
                None => std::env::current_dir()?,
            };
            println!("Recreating the grep cache for {}", dir.display());
            super::refresh_cache(&dir)?;
            return Ok(());
        }

        if self.par_run {
            self.par_run(params)?;
        } else {
            self.dyn_run(params)?;
        }

        Ok(())
    }

    fn usable_cache(&self, params: &Params) -> Option<PathBuf> {
        if !params.no_cache {
            if let Some(ref dir) = self.cmd_dir {
                let shell_cmd = super::rg_shell_command(dir);
                if let Some(digest) = shell_cmd.cache_digest() {
                    return Some(digest.cached_path);
                }
            }
        }
        None
    }

    /// Runs grep using the dyn filter.
    ///
    /// Firstly try using the cache.
    fn dyn_run(&self, params: Params) -> Result<()> {
        let maybe_usable_cache = self.usable_cache(&params);

        let filter_context = params
            .into_filter_context()
            .match_scope(MatchScope::GrepLine);

        let source: SequentialSource<std::iter::Empty<_>> = if let Some(cache) = maybe_usable_cache
        {
            SequentialSource::File(cache)
        } else if let Some(ref tempfile) = self.input {
            SequentialSource::File(tempfile.clone())
        } else if let Some(ref dir) = self.cmd_dir {
            Exec::shell(RG_EXEC_CMD).cwd(dir).into()
        } else {
            Exec::shell(RG_EXEC_CMD).into()
        };

        filter::dyn_run(&self.grep_query, filter_context, source)
    }

    fn par_run(&self, params: Params) -> Result<()> {
        let maybe_usable_cache = self.usable_cache(&params);

        let filter_context = params
            .into_filter_context()
            .match_scope(MatchScope::GrepLine);

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
        filter::par_dyn_run(&self.grep_query, filter_context, par_source)
    }
}
