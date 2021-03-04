use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;
use structopt::StructOpt;

use filter::{
    matcher::{Bonus, MatchType},
    subprocess::Exec,
    RunContext, Source,
};
use icon::IconPainter;
use utility::is_git_repo;

use crate::cmd::cache::{cache_exists, send_response_from_cache, SendResponse};
use crate::light_command::{set_current_dir, LightCommand};

const RG_ARGS: [&str; 7] = [
    "rg",
    "--column",
    "--line-number",
    "--no-heading",
    "--color=never",
    "--smart-case",
    "",
];

// Ref https://github.com/liuchengxu/vim-clap/issues/533
#[cfg(windows)]
const RG_EXEC_CMD: &str = "rg --column --line-number --no-heading --color=never --smart-case '' .";
#[cfg(not(windows))]
const RG_EXEC_CMD: &str = "rg --column --line-number --no-heading --color=never --smart-case ''";

#[derive(StructOpt, Debug, Clone)]
pub struct Grep {
    /// Specify the query string for GREP_CMD.
    #[structopt(index = 1, short, long)]
    grep_query: String,

    /// Specify the grep command to run, normally rg will be used.
    ///
    /// Incase of clap can not reconginize such option: --cmd "rg --vimgrep ... "fn ul"".
    ///                                                       |-----------------|
    ///                                                   this can be seen as an option by mistake.
    #[structopt(short, long, required_if("sync", "true"))]
    grep_cmd: Option<String>,

    /// Delegate to -g option of rg
    #[structopt(short = "g", long = "glob")]
    glob: Option<String>,

    /// Specify the working directory of CMD
    #[structopt(long = "cmd-dir", parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    /// Synchronous filtering, returns after the input stream is complete.
    #[structopt(short, long)]
    sync: bool,
}

fn prepare_grep_and_args(cmd_str: &str, cmd_dir: Option<PathBuf>) -> (Command, Vec<&str>) {
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
        .chain(std::iter::once("--json"))
        .collect::<Vec<&str>>();

    let mut cmd = Command::new(args[0]);

    set_current_dir(&mut cmd, cmd_dir);

    (cmd, args)
}

/// This struct represents the line content of rg's --json.
#[derive(Deserialize, Debug)]
pub struct JsonLine {
    #[serde(rename = "type")]
    pub ty: String,
    pub data: Match,
}

impl JsonLine {
    /// Returns the formatted String like using rg's -vimgrep option.
    pub fn format(&self, enable_icon: bool) -> String {
        let maybe_icon = if enable_icon {
            Some(icon::icon_for(&self.data.path.text))
        } else {
            None
        };
        format!(
            "{} {}:{}:{}:{}",
            maybe_icon.unwrap_or_default(),
            self.data.path.text,
            self.data.line_number.unwrap_or_default(),
            self.data.column(),
            self.data.lines.text.trim_end(),
        )
    }

    pub fn offset(&self, enable_icon: bool) -> usize {
        // filepath:line_number:column:text"
        let fixed_offset = if enable_icon { 3 + 4 } else { 3 };
        self.data.path.text.len()
            + self.data.line_number.unwrap_or_default().to_string().len()
            + self.data.column().to_string().len()
            + fixed_offset
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Text {
    pub text: String,
}

#[derive(Deserialize, Debug)]
pub struct Match {
    pub path: Text,
    pub lines: Text,
    pub line_number: Option<u64>,
    pub absolute_offset: u64,
    pub submatches: Vec<SubMatch>,
}

impl Match {
    pub fn match_indices(&self, offset: usize) -> Vec<usize> {
        self.submatches
            .iter()
            .map(|s| s.match_indices(offset))
            .flatten()
            .collect()
    }

    pub fn column(&self) -> usize {
        self.submatches[0].start
    }
}

#[derive(Deserialize, Debug)]
pub struct SubMatch {
    #[serde(rename = "match")]
    pub m: Text,
    pub start: usize,
    pub end: usize,
}

impl SubMatch {
    pub fn match_indices(&self, offset: usize) -> Vec<usize> {
        (self.start..self.end)
            .into_iter()
            .map(|x| x + offset)
            .collect()
    }
}

impl Grep {
    pub fn run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
        no_cache: bool,
    ) -> Result<()> {
        if self.sync {
            self.sync_run(number, winwidth, icon_painter)?;
        } else {
            self.dyn_run(number, winwidth, icon_painter, no_cache)?;
        }
        Ok(())
    }

    /// Runs grep command and returns until its output stream is completed.
    ///
    /// Write the output to the cache file if neccessary.
    fn sync_run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
    ) -> Result<()> {
        let grep_cmd = self
            .grep_cmd
            .clone()
            .context("--grep-cmd is required when --sync is on")?;
        let (mut cmd, mut args) = prepare_grep_and_args(&grep_cmd, self.cmd_dir.clone());

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

        let execute_info = light_cmd.execute(&args)?;

        let enable_icon = icon_painter.is_some();

        let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = execute_info
            .lines
            .iter()
            .filter_map(|s| serde_json::from_str::<JsonLine>(s).ok())
            .map(|line| {
                let formatted = line.format(enable_icon);
                let indices = line.data.match_indices(line.offset(enable_icon));
                (formatted, indices)
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
    fn dyn_run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
        no_cache: bool,
    ) -> Result<()> {
        let do_dyn_filter = |source: Source<std::iter::Empty<_>>| {
            filter::dyn_run(
                &self.grep_query,
                source,
                RunContext::new(
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
                if let Ok((cached_file, _)) = cache_exists(&RG_ARGS, dir) {
                    return do_dyn_filter(Source::File(cached_file));
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
        number: Option<usize>,
        icon_painter: Option<IconPainter>,
        no_cache: bool,
    ) -> Result<()> {
        if !no_cache {
            if let Some(ref dir) = self.cmd_dir {
                if let Ok((cache, total)) = cache_exists(&RG_ARGS, dir) {
                    send_response_from_cache(
                        &cache,
                        total,
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
            self.cmd_dir,
            number,
            icon_painter,
            Some(self.output_threshold),
        );

        light_cmd.execute(&RG_ARGS)?.print();

        Ok(())
    }
}
