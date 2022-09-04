mod forerunner;

pub use self::forerunner::RipGrepForerunner;

use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;
use filter::ParSource;
use rayon::prelude::*;
use subprocess::Exec;

use filter::Source;
use icon::Icon;
use matcher::MatchScope;

use crate::app::Params;
use crate::cache::Digest;
use crate::process::shell_command;
use crate::process::{CacheableCommand, ShellCommand};
use crate::tools::ripgrep::Match;

const RG_ARGS: &[&str] = &[
    "rg",
    "--column",
    "--line-number",
    "--no-heading",
    "--color=never",
    "--smart-case",
    "",
    ".",
];

// Ref https://github.com/liuchengxu/vim-clap/issues/533
// Now `.` is pushed to the end for all platforms due to https://github.com/liuchengxu/vim-clap/issues/711.
pub const RG_EXEC_CMD: &str = "rg --column --line-number --no-heading --color=never --smart-case '' .";

#[derive(Parser, Debug, Clone)]
pub struct Grep {
    /// Specify the query string for GREP_CMD.
    #[clap(index = 1, long)]
    grep_query: String,

    /// Specify the grep command to run, normally rg will be used.
    ///
    /// Incase of clap can not reconginize such option: --cmd "rg --vimgrep ... "fn ul"".
    ///                                                       |-----------------|
    ///                                                   this can be seen as an option by mistake.
    #[clap(long, required_if_eq("sync", "true"))]
    grep_cmd: Option<String>,

    /// Delegate to -g option of rg
    #[clap(long)]
    glob: Option<String>,

    /// Specify the working directory of CMD
    #[clap(long, parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[clap(long, parse(from_os_str))]
    input: Option<PathBuf>,

    /// Specify the directory for running ripgrep.
    #[clap(long, parse(from_os_str))]
    grep_dir: Vec<PathBuf>,

    /// Synchronous filtering, returns after the input stream is complete.
    #[clap(long)]
    sync: bool,

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
            refresh_cache(&dir)?;
            return Ok(());
        }

        if self.sync {
            self.sync_run(params)?;
        } else if self.par_run {
            self.par_run(params)?;
        } else {
            self.dyn_run(params)?;
        }

        Ok(())
    }

    /// Runs grep command and returns until its output stream is completed.
    ///
    /// Write the output to the cache file if neccessary.
    fn sync_run(
        &self,
        Params {
            number,
            winwidth,
            icon,
            ..
        }: Params,
    ) -> Result<()> {
        let mut grep_cmd = self
            .grep_cmd
            .clone()
            .context("--grep-cmd is required when --sync is on")?;

        if let Some(ref g) = self.glob {
            grep_cmd.push_str(" -g ");
            grep_cmd.push_str(g);
        }

        // Force using json format.
        grep_cmd.push_str(" --json ");
        grep_cmd.push_str(&self.grep_query);

        // currently vim-clap only supports rg.
        // Ref https://github.com/liuchengxu/vim-clap/pull/60
        grep_cmd.push_str(" .");

        // Shell command avoids https://github.com/liuchengxu/vim-clap/issues/595
        let mut std_cmd = shell_command(&grep_cmd);

        if let Some(ref dir) = self.cmd_dir {
            std_cmd.current_dir(dir);
        }

        let shell_cmd = ShellCommand::new(grep_cmd, std::env::current_dir()?);
        let execute_info =
            CacheableCommand::new(&mut std_cmd, shell_cmd, number, Default::default(), None)
                .execute()?;

        let enable_icon = !matches!(icon, Icon::Null);

        let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = execute_info
            .lines
            .par_iter()
            .filter_map(|s| {
                Match::try_from(s.as_str())
                    .ok()
                    .map(|mat| mat.build_grep_line(enable_icon))
            })
            .unzip();

        let total = lines.len();

        let (lines, indices, truncated_map) = printer::truncate_grep_lines(
            lines,
            indices,
            winwidth.unwrap_or(80),
            if enable_icon { Some(2) } else { None },
        );

        if truncated_map.is_empty() {
            utility::println_json!(total, lines, indices);
        } else {
            let icon_added = enable_icon;
            utility::println_json!(total, lines, indices, truncated_map, icon_added);
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
                let shell_cmd = rg_shell_command(dir);
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

// Used for creating the cache in async context.
#[derive(Debug, Clone, Hash)]
pub struct RgTokioCommand {
    shell_cmd: ShellCommand,
}

impl RgTokioCommand {
    pub fn new(dir: PathBuf) -> Self {
        let shell_cmd = ShellCommand::new(RG_EXEC_CMD.into(), dir);
        Self { shell_cmd }
    }

    pub fn cache_digest(&self) -> Option<Digest> {
        self.shell_cmd.cache_digest()
    }

    pub async fn create_cache(self) -> Result<Digest> {
        let cache_file = self.shell_cmd.cache_file_path()?;

        let std_cmd = rg_command(&self.shell_cmd.cwd);
        let mut tokio_cmd = tokio::process::Command::from(std_cmd);
        crate::process::tokio::write_stdout_to_file(&mut tokio_cmd, &cache_file).await?;

        let digest = crate::cache::store_cache_digest(self.shell_cmd.clone(), cache_file)?;

        Ok(digest)
    }
}

pub fn rg_command<P: AsRef<Path>>(dir: P) -> Command {
    // Can not use StdCommand as it joins the args which does not work somehow.
    let mut cmd = Command::new(RG_ARGS[0]);
    // Do not use --vimgrep here.
    cmd.args(&RG_ARGS[1..]).current_dir(dir);
    cmd
}

#[inline]
pub fn rg_shell_command<P: AsRef<Path>>(dir: P) -> ShellCommand {
    ShellCommand::new(RG_EXEC_CMD.into(), PathBuf::from(dir.as_ref()))
}

pub fn refresh_cache(dir: impl AsRef<Path>) -> Result<Digest> {
    let shell_cmd = rg_shell_command(dir.as_ref());
    let cache_file_path = shell_cmd.cache_file_path()?;

    let mut cmd = rg_command(dir.as_ref());
    crate::process::write_stdout_to_file(&mut cmd, &cache_file_path)?;

    let digest = crate::cache::store_cache_digest(shell_cmd, cache_file_path)?;

    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::tokio::TokioCommand;
    use itertools::Itertools;
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
        println!("Elapsed: {:.3?}", elapsed);

        let now = Instant::now();
        let rg_cmd = RgTokioCommand::new(dir);
        let res = rg_cmd.create_cache().await.unwrap();
        println!(
            "Cache creation result(new): {:?}",
            (res.total, res.cached_path)
        );
        let elapsed = now.elapsed();
        println!("Elapsed: {:.3?}", elapsed);
    }
}
