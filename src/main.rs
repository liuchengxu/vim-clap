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

use crate::icon::{prepend_grep_icon, prepend_icon, DEFAULT_ICONIZED};

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

    /// Prepend an icon for item of grep provider, valid only when --number is used.
    #[structopt(long = "grep-enable-icon")]
    grep_enable_icon: bool,

    /// Prepend an icon for item of files provider, valid only when --number is used.
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

    fn try_prepend_icon<'a>(&self, top_n: impl std::iter::Iterator<Item = &'a str>) -> Vec<String> {
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

    fn execute_impl(&self, cmd: &mut Command, args: &[String]) -> Result<()> {
        let cmd_output = cmd.output()?;

        if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
            let error = format!("{}", String::from_utf8_lossy(&cmd_output.stderr));
            println_json!(error);
            std::process::exit(1);
        }

        let total = bytecount::count(&cmd_output.stdout, b'\n');

        if let Some(number) = self.number {
            // TODO: do not have to into String for whole stdout, find the nth index of newline.
            // &cmd_output.stdout[..nth_newline_index]
            let stdout_str = String::from_utf8_lossy(&cmd_output.stdout);
            let lines = self.try_prepend_icon(stdout_str.split('\n').take(number));
            println_json!(total, lines);
            return Ok(());
        }

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

    fn prepare_grep_and_args(&self, cmd_str: &str) -> (Command, Vec<String>) {
        let args = cmd_str
            .split_whitespace()
            .map(Into::into)
            .collect::<Vec<String>>();
        let mut cmd = Command::new(args[0].clone());
        self.set_cmd_dir(&mut cmd);
        (cmd, args)
    }

    pub fn try_exec_grep(&self) -> Result<()> {
        if let Some(ref grep_cmd) = self.grep_cmd {
            let (mut cmd, mut args) = self.prepare_grep_and_args(grep_cmd);

            // We split out the grep opts and query in case of the possible escape issue of clap.
            if let Some(grep_query) = self.grep_query.clone() {
                args.push(grep_query);
            }

            // currently vim-clap only supports rg.
            // Ref https://github.com/liuchengxu/vim-clap/pull/60
            if cfg!(windows) {
                args.push(".".into());
            }

            cmd.args(&args[1..]);

            self.execute_impl(&mut cmd, &args)?;

            return Ok(());
        }
        Err(anyhow::Error::new(DummyError).context("No grep cmd specified"))
    }

    // This can work with the piped command, e.g., git ls-files | uniq.
    fn prepare_cmd(&self, cmd_str: &str) -> Command {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(&["/C", cmd_str]);
            cmd
        } else {
            let mut cmd = Command::new("bash");
            cmd.arg("-c").arg(cmd_str);
            cmd
        };
        self.set_cmd_dir(&mut cmd);
        cmd
    }

    pub fn try_exec_cmd(&self) -> Result<()> {
        if let Some(ref cmd_str) = self.cmd {
            let mut cmd = self.prepare_cmd(cmd_str);

            self.execute_impl(
                &mut cmd,
                &cmd_str
                    .split_whitespace()
                    .map(Into::into)
                    .collect::<Vec<_>>(),
            )?;

            return Ok(());
        }
        Err(anyhow::Error::new(DummyError).context("No cmd specified"))
    }

    fn apply_fuzzy_filter_and_rank(&self) -> Result<Vec<(String, f64, Vec<usize>)>> {
        let query = &*self.query;
        let algo = self.algo.as_ref().unwrap_or(&Algo::Fzy);

        let scorer = |line: &str| match algo {
            Algo::Skim => {
                fuzzy_indices(line, query).map(|(score, indices)| (score as f64, indices))
            }
            Algo::Fzy => match_and_score_with_positions(query, line),
        };

        // Result<Option<T>> => T
        let mut ranked = if let Some(input) = &self.input {
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

    pub fn do_filter(&self) -> Result<()> {
        let ranked = self.apply_fuzzy_filter_and_rank()?;

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

        Ok(())
    }
}

pub fn main() -> Result<()> {
    let maple = Maple::from_args();

    if maple.try_exec_cmd().is_ok() || maple.try_exec_grep().is_ok() {
        return Ok(());
    }

    maple.do_filter()?;

    Ok(())
}
