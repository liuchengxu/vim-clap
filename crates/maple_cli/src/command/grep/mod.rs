mod forerunner;

pub use self::forerunner::RipGrepForerunner;

use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use itertools::Itertools;
use rayon::prelude::*;
use structopt::StructOpt;

use filter::{
    matcher::{Bonus, MatchType},
    subprocess::Exec,
    Source,
};
use icon::Icon;
use utility::is_git_repo;

use crate::app::Params;
use crate::process::tokio::TokioCommand;
use crate::process::{light::LightCommand, rstd::StdCommand, BaseCommand};
use crate::tools::ripgrep::Match;
use crate::utils::{send_response_from_cache, SendResponse};

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
const RG_EXEC_CMD: &str = "rg --column --line-number --no-heading --color=never --smart-case '' .";

#[derive(StructOpt, Debug, Clone)]
pub struct Grep {
    /// Specify the query string for GREP_CMD.
    #[structopt(index = 1, long)]
    grep_query: String,

    /// Specify the grep command to run, normally rg will be used.
    ///
    /// Incase of clap can not reconginize such option: --cmd "rg --vimgrep ... "fn ul"".
    ///                                                       |-----------------|
    ///                                                   this can be seen as an option by mistake.
    #[structopt(long, required_if("sync", "true"))]
    grep_cmd: Option<String>,

    /// Delegate to -g option of rg
    #[structopt(long)]
    glob: Option<String>,

    /// Specify the working directory of CMD
    #[structopt(long, parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[structopt(long, parse(from_os_str))]
    input: Option<PathBuf>,

    /// Synchronous filtering, returns after the input stream is complete.
    #[structopt(long)]
    sync: bool,
}

impl Grep {
    pub fn run(&self, params: Params) -> Result<()> {
        if self.sync {
            self.sync_run(params)?;
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
        let mut std_cmd = StdCommand::new(&grep_cmd);

        if let Some(ref dir) = self.cmd_dir {
            std_cmd.current_dir(dir);
        }

        let mut cmd = std_cmd.into_inner();

        let mut light_cmd =
            LightCommand::new_grep(&mut cmd, None, number, Default::default(), None);

        let base_cmd = BaseCommand::new(grep_cmd, std::env::current_dir()?);
        let execute_info = light_cmd.execute(base_cmd)?;

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
            utility::println_json!(total, lines, indices, truncated_map);
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
                source,
                params
                    .into_filter_context()
                    .match_type(MatchType::IgnoreFilePath),
                vec![Bonus::None],
            )
        };

        let source: Source<std::iter::Empty<_>> = if let Some(ref tempfile) = self.input {
            Source::File(tempfile.clone())
        } else if let Some(ref dir) = self.cmd_dir {
            if !no_cache {
                let base_cmd = BaseCommand::new(RG_EXEC_CMD.into(), dir.clone());
                if let Some(cache_file) = base_cmd.cache_file() {
                    return do_dyn_filter(Source::File(cache_file));
                }
            }
            Exec::shell(RG_EXEC_CMD).cwd(dir).into()
        } else {
            Exec::shell(RG_EXEC_CMD).into()
        };

        do_dyn_filter(source)
    }
}

#[derive(Debug, Clone)]
pub struct RgBaseCommand {
    pub inner: BaseCommand,
}

impl RgBaseCommand {
    pub fn new(dir: PathBuf) -> Self {
        let inner = BaseCommand::new(RG_EXEC_CMD.into(), dir);
        Self { inner }
    }

    pub fn cache_info(&self) -> Option<(usize, PathBuf)> {
        self.inner.cache_info()
    }

    pub async fn create_cache(self) -> Result<(usize, PathBuf)> {
        let lines = TokioCommand::new(&self.inner.command)
            .current_dir(&self.inner.cwd)
            .lines()
            .await?;

        let total = lines.len();
        let lines = lines.into_iter().join("\n");

        let cache_path = self.inner.create_cache(total, lines.as_bytes())?;

        Ok((total, cache_path))
    }
}

pub fn refresh_cache(dir: impl AsRef<Path>) -> Result<usize> {
    let mut cmd = Command::new(RG_ARGS[0]);
    // Do not use --vimgrep here.
    cmd.args(&RG_ARGS[1..]).current_dir(dir.as_ref());

    let stdout = crate::process::rstd::collect_stdout(&mut cmd)?;

    let total = bytecount::count(&stdout, b'\n');

    let base_cmd = BaseCommand::new(RG_EXEC_CMD.into(), PathBuf::from(dir.as_ref()));

    base_cmd.create_cache(total, &stdout)?;

    Ok(total)
}
