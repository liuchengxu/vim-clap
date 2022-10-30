use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use subprocess::Exec;

use filter::{ParSource, Source};
use matcher::MatchScope;

use crate::app::Params;
use crate::command::grep::RG_EXEC_CMD;
use crate::process::ShellCommand;

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

    /// Specify the directory for running ripgrep.
    #[clap(long, parse(from_os_str))]
    grep_dir: Vec<PathBuf>,

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

    /// Runs grep using the dyn filter.
    ///
    /// Firstly try using the cache.
    fn dyn_run(&self, params: Params) -> Result<()> {
        let no_cache = params.no_cache;

        let do_dyn_filter = |source: Source<std::iter::Empty<_>>| {
            filter::dyn_run(
                &self.grep_query,
                params
                    .into_filter_context()
                    .match_scope(MatchScope::GrepLine),
                source,
            )
        };

        let source: Source<std::iter::Empty<_>> = if let Some(ref tempfile) = self.input {
            Source::File(tempfile.clone())
        } else if let Some(ref dir) = self.cmd_dir {
            if !no_cache {
                let shell_cmd = super::rg_shell_command(dir);
                if let Some(digest) = shell_cmd.cache_digest() {
                    return do_dyn_filter(Source::File(digest.cached_path));
                }
            }
            Exec::shell(RG_EXEC_CMD).cwd(dir).into()
        } else {
            Exec::shell(RG_EXEC_CMD).into()
        };

        do_dyn_filter(source)
    }

    fn par_run(&self, params: Params) -> Result<()> {
        let no_cache = params.no_cache;

        let par_dyn_dun = |par_source: ParSource| {
            filter::par_dyn_run(
                &self.grep_query,
                params
                    .into_filter_context()
                    .match_scope(MatchScope::GrepLine),
                par_source,
            )
        };

        let par_source = if let Some(ref tempfile) = self.input {
            ParSource::File(tempfile.clone())
        } else if let Some(ref dir) = self.cmd_dir {
            if !no_cache {
                let shell_cmd = ShellCommand::new(RG_EXEC_CMD.into(), dir.clone());
                if let Some(digest) = shell_cmd.cache_digest() {
                    return par_dyn_dun(ParSource::File(digest.cached_path));
                }
            }
            ParSource::Exec(Box::new(Exec::shell(RG_EXEC_CMD).cwd(dir)))
        } else {
            ParSource::Exec(Box::new(Exec::shell(RG_EXEC_CMD)))
        };

        // TODO: Improve the responsiveness of ripgrep as it can emit the items after some time.
        // When running the command below, a few seconds before showing the progress, might be
        // mitigated by using the libripgrep instead of using the rg executable.
        // time /home/xlc/.vim/plugged/vim-clap/target/release/maple --icon=Grep --no-cache --number 136 --winwidth 122 --case-matching smart grep srlss --cmd-dir /home/xlc/src/github.com/subspace/subspace --par-run
        par_dyn_dun(par_source)
    }
}
