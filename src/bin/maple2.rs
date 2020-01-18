use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::SystemTime;

use anyhow::Result;
use extracted_fzy::match_and_score_with_positions;
use fuzzy_matcher::skim::fuzzy_indices;
use rayon::prelude::*;
use serde_json::json;
use structopt::StructOpt;

use maple::cmd::{Algo, Cmd};
use maple::icon::{prepend_grep_icon, prepend_icon, DEFAULT_ICONIZED};

#[derive(StructOpt, Debug)]
#[structopt(name = "maple")]
struct Maple {
    /// Print the top NUM of filtered items.
    ///
    /// The returned JSON has three fields:
    ///   - total: total number of initial filtered result set.
    ///   - lines: text lines used for displaying directly.
    ///   - indices: the indices of matched elements per line, used for the highlight purpose.
    #[structopt(short = "n", long = "number", name = "NUM")]
    number: Option<usize>,

    /// Prepend an icon for item of files and grep provider, valid only when --number is used.
    #[structopt(long = "enable-icon")]
    enable_icon: bool,

    #[structopt(subcommand)]
    command: Cmd,
}

#[derive(Debug)]
struct DummyError;

impl std::fmt::Display for DummyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DummyError is here!")
    }
}

impl std::error::Error for DummyError {
    fn description(&self) -> &str {
        "DummyError used for anyhow"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        None
    }
}

/// Remove the last element if it's empty string.
#[inline]
fn trim_trailing(lines: &mut Vec<String>) {
    if let Some(last_line) = lines.last() {
        if last_line.is_empty() || last_line == DEFAULT_ICONIZED {
            lines.remove(lines.len() - 1);
        }
    }
}

/// Combine json and println macro.
macro_rules! println_json {
  ( $( $field:expr ),+ ) => {
    {
      println!("{}", json!({ $(stringify!($field): $field,)* }))
    }
  }
}

fn set_current_dir(cmd: &mut Command, cmd_dir: Option<PathBuf>) {
    if let Some(cmd_dir) = cmd_dir {
        // If cmd_dir is not a directory, use its parent as current dir.
        if cmd_dir.is_dir() {
            cmd.current_dir(cmd_dir);
        } else {
            let mut cmd_dir = cmd_dir;
            cmd_dir.pop();
            cmd.current_dir(cmd_dir);
        }
    }
}

fn prepare_grep_and_args(cmd_str: &str, cmd_dir: Option<PathBuf>) -> (Command, Vec<String>) {
    let args = cmd_str
        .split_whitespace()
        .map(Into::into)
        .collect::<Vec<String>>();

    let mut cmd = Command::new(args[0].clone());

    set_current_dir(&mut cmd, cmd_dir);

    (cmd, args)
}

// This can work with the piped command, e.g., git ls-files | uniq.
fn prepare_exec_cmd(cmd_str: &str, cmd_dir: Option<PathBuf>) -> Command {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(&["/C", cmd_str]);
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(cmd_str);
        cmd
    };

    set_current_dir(&mut cmd, cmd_dir);

    cmd
}

#[derive(Debug)]
struct LightCommand<'a> {
    cmd: &'a mut Command,
    total: usize,
    number: Option<usize>,
    output: Option<String>,
    enable_icon: bool,
    grep_enable_icon: bool,
    output_threshold: usize,
}

impl<'a> LightCommand<'a> {
    fn new(
        cmd: &'a mut Command,
        number: Option<usize>,
        output: Option<String>,
        enable_icon: bool,
        grep_enable_icon: bool,
        output_threshold: usize,
    ) -> Self {
        let total = 0;
        Self {
            cmd,
            number,
            total,
            output,
            enable_icon,
            grep_enable_icon,
            output_threshold,
        }
    }

    fn output(&mut self) -> Result<Output> {
        let cmd_output = self.cmd.output()?;

        // vim-clap does not handle the stderr stream, we just pass the error info via stdout.
        if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
            let error = format!("{}", String::from_utf8_lossy(&cmd_output.stderr));
            println_json!(error);
            std::process::exit(1);
        }

        Ok(cmd_output)
    }

    fn try_truncate(&self, stdout: &[u8]) -> Result<()> {
        if let Some(number) = self.number {
            // TODO: do not have to into String for whole stdout, find the nth index of newline.
            // &cmd_output.stdout[..nth_newline_index]
            let stdout_str = String::from_utf8_lossy(&stdout);
            let lines = self.try_prepend_icon(stdout_str.split('\n').take(number));
            let total = self.total;
            println_json!(total, lines);
            return Ok(());
        }
        Err(anyhow::Error::new(DummyError).context("No truncation"))
    }

    fn try_prepend_icon<'b>(&self, top_n: impl std::iter::Iterator<Item = &'b str>) -> Vec<String> {
        let mut lines = if self.grep_enable_icon {
            top_n.map(prepend_grep_icon).collect::<Vec<_>>()
        } else if self.enable_icon {
            top_n.map(prepend_icon).collect::<Vec<_>>()
        } else {
            top_n.map(Into::into).collect::<Vec<_>>()
        };
        trim_trailing(&mut lines);
        lines
    }

    fn to_string_and_cache_if_threshold_exceeded(
        &self,
        total: usize,
        cmd_stdout: &[u8],
        args: &[String],
    ) -> Result<(String, Option<PathBuf>)> {
        if total > self.output_threshold {
            let tempfile = if let Some(ref output) = self.output {
                output.into()
            } else {
                let mut dir = std::env::temp_dir();
                dir.push(format!(
                    "{}_{}",
                    args.join("_"),
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)?
                        .as_secs()
                ));
                dir
            };
            File::create(tempfile.clone())?.write_all(cmd_stdout)?;
            // FIXME find the nth newline index of stdout.
            let _end = std::cmp::min(cmd_stdout.len(), 500);
            Ok((
                // lines used for displaying directly.
                // &cmd_output.stdout[..nth_newline_index]
                String::from_utf8_lossy(cmd_stdout).into(),
                Some(tempfile),
            ))
        } else {
            Ok((String::from_utf8_lossy(cmd_stdout).into(), None))
        }
    }

    fn execute(&mut self, args: &[String]) -> Result<()> {
        let cmd_output = self.output()?;
        let cmd_stdout = &cmd_output.stdout;

        self.total = bytecount::count(cmd_stdout, b'\n');

        if self.try_truncate(cmd_stdout).is_ok() {
            return Ok(());
        }

        let total = self.total;
        // Write the output to a tempfile if the lines are too many.
        let (stdout_str, tempfile) =
            self.to_string_and_cache_if_threshold_exceeded(total, &cmd_output.stdout, args)?;
        let lines = self.try_prepend_icon(stdout_str.split('\n'));
        if let Some(tempfile) = tempfile {
            println_json!(total, lines, tempfile);
        } else {
            println_json!(total, lines);
        }

        Ok(())
    }
}

impl Maple {
    fn apply_fuzzy_filter_and_rank(
        &self,
        query: &str,
        input: &Option<PathBuf>,
        algo: &Option<Algo>,
    ) -> Result<Vec<(String, f64, Vec<usize>)>> {
        let algo = algo.as_ref().unwrap_or(&Algo::Fzy);

        let scorer = |line: &str| match algo {
            Algo::Skim => {
                fuzzy_indices(line, &query).map(|(score, indices)| (score as f64, indices))
            }
            Algo::Fzy => match_and_score_with_positions(&query, line),
        };

        // Result<Option<T>> => T
        let mut ranked = if let Some(input) = input {
            std::fs::read_to_string(input)?
                .par_lines()
                .filter_map(|line| {
                    scorer(&line).map(|(score, indices)| (line.into(), score, indices))
                })
                .collect::<Vec<_>>()
        } else {
            io::stdin()
                .lock()
                .lines()
                .filter_map(|lines_iter| {
                    lines_iter.ok().and_then(|line| {
                        scorer(&line).map(|(score, indices)| (line, score, indices))
                    })
                })
                .collect::<Vec<_>>()
        };

        ranked.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

        Ok(ranked)
    }

    fn run(&self) -> Result<()> {
        match &self.command {
            Cmd::Filter { query, input, algo } => {
                let ranked = self.apply_fuzzy_filter_and_rank(query, input, algo)?;

                if let Some(number) = self.number {
                    let total = ranked.len();
                    let payload = ranked.into_iter().take(number);
                    let mut lines = Vec::with_capacity(number);
                    let mut indices = Vec::with_capacity(number);
                    if self.enable_icon {
                        for (text, _, idxs) in payload {
                            lines.push(prepend_icon(&text));
                            indices.push(idxs);
                        }
                    } else {
                        for (text, _, idxs) in payload {
                            lines.push(text);
                            indices.push(idxs);
                        }
                    }
                    println_json!(total, lines, indices);
                } else {
                    for (text, _, indices) in ranked.iter() {
                        println_json!(text, indices);
                    }
                }
            }
            Cmd::Exec {
                cmd,
                output,
                cmd_dir,
                output_threshold,
            } => {
                let mut exec_cmd = prepare_exec_cmd(cmd, cmd_dir.clone());

                let mut light_cmd = LightCommand::new(
                    &mut exec_cmd,
                    self.number,
                    output.clone(),
                    self.enable_icon,
                    false,
                    *output_threshold,
                );

                light_cmd.execute(&cmd.split_whitespace().map(Into::into).collect::<Vec<_>>())?;
            }
            Cmd::Grep {
                grep_cmd,
                grep_query,
                cmd_dir,
            } => {
                let (mut cmd, mut args) = prepare_grep_and_args(grep_cmd, cmd_dir.clone());

                // We split out the grep opts and query in case of the possible escape issue of clap.
                args.push(grep_query.clone());

                // currently vim-clap only supports rg.
                // Ref https://github.com/liuchengxu/vim-clap/pull/60
                if cfg!(windows) {
                    args.push(".".into());
                }

                cmd.args(&args[1..]);

                let mut light_cmd =
                    LightCommand::new(&mut cmd, self.number, None, false, self.enable_icon, 0usize);

                light_cmd.execute(&args)?;
            }
        }
        Ok(())
    }
}

pub fn main() -> Result<()> {
    let maple = Maple::from_args();

    maple.run()?;

    Ok(())
}
