use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use structopt::StructOpt;

use filter::{
    matcher::{Bonus, MatchType},
    subprocess::Exec,
    FilterContext, Source,
};
use icon::IconPainter;
use utility::is_git_repo;

use crate::app::Params;
use crate::process::{
    light::{set_current_dir, LightCommand},
    BaseCommand,
};
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

fn prepare_sync_grep_cmd<P: AsRef<Path>>(
    cmd_str: &str,
    cmd_dir: Option<P>,
) -> (Command, Vec<&str>) {
    let args = cmd_str
        .split_whitespace()
        // If cmd_str contains a quoted option, that's problematic.
        //
        // Ref https://github.com/liuchengxu/vim-clap/issues/595
        .map(|s| {
            if s.len() > 2 {
                if s.starts_with('"') && s.chars().nth_back(0).unwrap() == '"' {
                    &s[1..s.len() - 1]
                } else {
                    s
                }
            } else {
                s
            }
        })
        .chain(std::iter::once("--json")) // Force using json format.
        .collect::<Vec<&str>>();

    let mut cmd = Command::new(args[0]);

    set_current_dir(&mut cmd, cmd_dir);

    (cmd, args)
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
            icon_painter,
            ..
        }: Params,
    ) -> Result<()> {
        let grep_cmd = self
            .grep_cmd
            .clone()
            .context("--grep-cmd is required when --sync is on")?;
        let (mut cmd, mut args) = prepare_sync_grep_cmd(&grep_cmd, self.cmd_dir.as_ref());

        // We split out the grep opts and query in case of the possible escape issue of clap.
        args.push(&self.grep_query);

        if let Some(ref g) = self.glob {
            args.push("-g");
            args.push(g);
        }

        // currently vim-clap only supports rg.
        // Ref https://github.com/liuchengxu/vim-clap/pull/60
        if cfg!(windows) {
            args.push(".");
        }

        cmd.args(&args[1..]);

        let mut light_cmd = LightCommand::new_grep(&mut cmd, None, number, None, None);

        let base_cmd = BaseCommand::new(grep_cmd, std::env::current_dir()?);
        let execute_info = light_cmd.execute(base_cmd)?;

        let enable_icon = icon_painter.is_some();

        let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = execute_info
            .lines
            .iter()
            .filter_map(|s| Match::try_from(s.as_str()).ok())
            .map(|mat| mat.build_grep_line(enable_icon))
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
    fn dyn_run(
        &self,
        Params {
            number,
            winwidth,
            icon_painter,
            no_cache,
        }: Params,
    ) -> Result<()> {
        let do_dyn_filter = |source: Source<std::iter::Empty<_>>| {
            filter::dyn_run(
                &self.grep_query,
                source,
                FilterContext::new(
                    None,
                    number,
                    winwidth,
                    icon_painter,
                    MatchType::IgnoreFilePath,
                ),
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

#[derive(StructOpt, Debug, Clone)]
pub struct RipGrepForerunner {
    /// Specify the working directory of CMD
    #[structopt(long = "cmd-dir", parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Specify the threshold for writing the output of command to a tempfile.
    #[structopt(long = "output-threshold", default_value = "30000")]
    output_threshold: usize,
}

impl RipGrepForerunner {
    /// Skip the forerunner job if `cmd_dir` is not a git repo.
    ///
    /// Only spawn the forerunner job for git repo for now.
    fn should_skip(&self) -> bool {
        if let Some(ref dir) = self.cmd_dir {
            if !is_git_repo(dir) {
                return true;
            }
        } else if let Ok(dir) = std::env::current_dir() {
            if !is_git_repo(&dir) {
                return true;
            }
        }
        false
    }

    pub fn run(
        self,
        Params {
            number,
            icon_painter,
            no_cache,
            ..
        }: Params,
    ) -> Result<()> {
        if !no_cache {
            if let Some(ref dir) = self.cmd_dir {
                let base_cmd = BaseCommand::new(RG_EXEC_CMD.into(), dir.clone());
                if let Some((total, cache)) = base_cmd.cached_info() {
                    send_response_from_cache(
                        &cache,
                        total as usize,
                        SendResponse::Json,
                        Some(IconPainter::Grep),
                    );
                    return Ok(());
                }
            }
        }

        if self.should_skip() {
            return Ok(());
        }

        let mut cmd = Command::new(RG_ARGS[0]);
        // Do not use --vimgrep here.
        cmd.args(&RG_ARGS[1..]);

        set_current_dir(&mut cmd, self.cmd_dir.clone());

        let mut light_cmd = LightCommand::new_grep(
            &mut cmd,
            self.cmd_dir.clone(),
            number,
            icon_painter,
            Some(self.output_threshold),
        );

        let cwd = match self.cmd_dir {
            Some(d) => d,
            None => std::env::current_dir()?,
        };
        let base_cmd = BaseCommand::new(RG_EXEC_CMD.into(), cwd);

        light_cmd.execute(base_cmd)?.print();

        Ok(())
    }
}
