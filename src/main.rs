mod icon;

use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

use anyhow::Result;
use extracted_fzy::match_and_score_with_positions;
use fuzzy_matcher::skim::fuzzy_indices;
use rayon::prelude::*;
use serde_json::json;
use structopt::clap::arg_enum;
use structopt::StructOpt;

use icon::prepend_icon;

arg_enum! {
    #[derive(Debug)]
    enum Algo {
        Skim,
        Fzy,
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "maple")]
struct Maple {
    /// Initial query string
    #[structopt(index = 1, short, long)]
    query: String,

    /// Filter algorithm
    #[structopt(short, long, possible_values = &Algo::variants(), case_insensitive = true)]
    algo: Option<Algo>,

    /// Read input from a file instead of stdin, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    /// Print the top NUM of filtered items.
    ///
    /// The returned JSON has three fields:
    ///   - total: total number of initial filtered result set.
    ///   - lines: text lines used for displaying directly.
    ///   - indices: the indices of matched elements per line, used for the highlight purpose.
    #[structopt(short = "n", long = "number", name = "NUM")]
    number: Option<usize>,

    /// Specify the output file path when the output of command exceeds the threshold.
    #[structopt(long = "output")]
    output: Option<String>,

    /// Specify the threshold for writing the output of command to a tempfile.
    #[structopt(long = "output-threshold", default_value = "100000")]
    output_threshold: usize,

    /// Specify the system command to run.
    #[structopt(long = "cmd", name = "CMD")]
    cmd: Option<String>,

    /// Specify the grep command to run, normally rg will be used.
    ///
    /// Incase of clap can not reconginize such option: --cmd "rg --vimgrep ... "fn ul"".
    ///                                                       |-----------------|
    ///                                                   this can be seen as an option by mistake.
    #[structopt(long = "grep-cmd", name = "GREP_CMD")]
    grep_cmd: Option<String>,

    /// Specify the query string for GREP_CMD.
    #[structopt(long = "grep-query")]
    grep_query: Option<String>,

    /// Prepend an icon for item of files and grep provider.
    #[structopt(long = "enable-icon")]
    enable_icon: bool,

    /// Specify the working directory of CMD
    #[structopt(long = "cmd-dir", parse(from_os_str))]
    cmd_dir: Option<PathBuf>,
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
        if last_line.is_empty() {
            lines.remove(lines.len() - 1);
        }
    }
}

impl Maple {
    pub fn set_cmd_dir(&self, cmd: &mut Command) {
        if let Some(cmd_dir) = self.cmd_dir.clone() {
            if cmd_dir.is_dir() {
                cmd.current_dir(cmd_dir);
            } else {
                let mut cmd_dir = cmd_dir;
                cmd_dir.pop();
                cmd.current_dir(cmd_dir);
            }
        }
    }

    fn execute(&self, cmd: &mut Command, args: &[String]) -> Result<()> {
        let cmd_output = cmd.output()?;

        let line_count = bytecount::count(&cmd_output.stdout, b'\n');

        if let Some(number) = self.number {
            // TODO: do not have to into String for whole stdout, find the nth index of newline.
            // &cmd_output.stdout[..nth_newline_index]
            let stdout_str = String::from_utf8_lossy(&cmd_output.stdout);
            let mut lines = stdout_str
                .split('\n')
                .take(number)
                .map(Into::into)
                .collect::<Vec<_>>();
            trim_trailing(&mut lines);
            println!(
                "{}",
                json!({
                  "total": line_count,
                  "lines": lines
                })
            );
            return Ok(());
        }

        // Write the output to a tempfile if the lines are too many.
        let (stdout_str, tempfile): (String, Option<PathBuf>) =
            if line_count > self.output_threshold {
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
                File::create(tempfile.clone())?.write_all(&cmd_output.stdout)?;
                // FIXME find the nth newline index of stdout.
                let _end = std::cmp::min(cmd_output.stdout.len(), 500);
                (
                    // lines used for displaying directly.
                    // &cmd_output.stdout[..nth_newline_index]
                    String::from_utf8_lossy(&cmd_output.stdout).into(),
                    Some(tempfile),
                )
            } else {
                (String::from_utf8_lossy(&cmd_output.stdout).into(), None)
            };

        let mut lines = if self.enable_icon {
            stdout_str.split('\n').map(prepend_icon).collect::<Vec<_>>()
        } else {
            stdout_str.split('\n').map(Into::into).collect::<Vec<_>>()
        };

        // The last element could be a empty string.
        trim_trailing(&mut lines);

        if let Some(tempfile) = tempfile {
            println!(
                "{}",
                json!({"total": line_count, "lines": lines, "tempfile": tempfile})
            );
        } else {
            println!("{}", json!({"total": line_count, "lines": lines}));
        }

        Ok(())
    }

    pub fn try_exec_grep(&self) -> Result<()> {
        if let Some(ref grep_cmd) = self.grep_cmd {
            let mut args = grep_cmd
                .split_whitespace()
                .map(Into::into)
                .collect::<Vec<String>>();
            let mut cmd = Command::new(args[0].clone());
            self.set_cmd_dir(&mut cmd);
            // TODO windows needs to append . for rg
            if let Some(grep_query) = self.grep_query.clone() {
                args.push(grep_query);
            }
            // currently vim-clap only supports rg.
            // Ref https://github.com/liuchengxu/vim-clap/pull/60
            if cfg!(windows) {
                args.push(".".into());
            }
            cmd.args(&args[1..]);
            self.execute(&mut cmd, &args)?;
            return Ok(());
        }
        Err(anyhow::Error::new(DummyError).context("No grep cmd specified"))
    }

    pub fn try_exec_cmd(&self) -> Result<()> {
        if let Some(ref cmd) = self.cmd {
            // TODO: translate piped command?
            let args = cmd
                .split_whitespace()
                .map(Into::into)
                .collect::<Vec<String>>();
            let mut cmd = Command::new(args[0].clone());
            self.set_cmd_dir(&mut cmd);
            cmd.args(&args[1..]);
            self.execute(&mut cmd, &args)?;
            return Ok(());
        }
        Err(anyhow::Error::new(DummyError).context("No cmd specified"))
    }
}

pub fn main() -> Result<()> {
    let opt = Maple::from_args();

    if opt.try_exec_cmd().is_ok() || opt.try_exec_grep().is_ok() {
        return Ok(());
    }

    let query = &*opt.query;
    let algo = opt.algo.unwrap_or(Algo::Fzy);

    let scorer = |line: &str| match algo {
        Algo::Skim => fuzzy_indices(line, query).map(|(score, indices)| (score as f64, indices)),
        Algo::Fzy => match_and_score_with_positions(query, line),
    };

    // Result<Option<T>> => T
    let mut ranked = if let Some(input) = opt.input {
        std::fs::read_to_string(input)?
            .par_lines()
            .filter_map(|line| scorer(&line).map(|(score, indices)| (line.into(), score, indices)))
            .collect::<Vec<_>>()
    } else {
        io::stdin()
            .lock()
            .lines()
            .filter_map(|lines_iter| {
                lines_iter
                    .ok()
                    .and_then(|line| scorer(&line).map(|(score, indices)| (line, score, indices)))
            })
            .collect::<Vec<_>>()
    };

    ranked.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

    if let Some(number) = opt.number {
        let total = ranked.len();
        let payload = ranked.into_iter().take(number);
        let mut lines = Vec::with_capacity(number);
        let mut indices = Vec::with_capacity(number);
        for (text, _, idxs) in payload {
            if opt.enable_icon {
                lines.push(prepend_icon(&text));
            } else {
                lines.push(text);
            }
            indices.push(idxs);
        }
        println!(
            "{}",
            json!({"total": total, "lines": lines, "indices": indices})
        );
    } else {
        for (text, _, indices) in ranked.iter() {
            println!(
                "{}",
                json!({
                "text": text,
                "indices": indices,
                })
            );
        }
    }

    Ok(())
}
