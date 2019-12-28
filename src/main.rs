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

impl Maple {
    pub fn try_exec_cmd(&self) -> Result<()> {
        if let Some(ref cmd) = self.cmd {
            // TODO: translate piped command?
            let args = cmd
                .split_whitespace()
                .map(Into::into)
                .collect::<Vec<String>>();

            let mut cmd = Command::new(args[0].clone());
            if let Some(cmd_dir) = self.cmd_dir.clone() {
                if cmd_dir.is_dir() {
                    cmd.current_dir(cmd_dir);
                } else {
                    let mut cmd_dir = cmd_dir;
                    cmd_dir.pop();
                    cmd.current_dir(cmd_dir);
                }
            }
            cmd.args(&args[1..]);

            let cmd_output = cmd.output()?;

            let line_count = bytecount::count(&cmd_output.stdout, b'\n');

            // Write the output to a tempfile if the lines are too many.
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
                let end = std::cmp::min(cmd_output.stdout.len(), 500);
                println!(
                    "{}",
                    json!({
                      "total": line_count,
                      // lines used for displaying directly.
                      "lines": String::from_utf8_lossy(&cmd_output.stdout[..end]).split("\n").collect::<Vec<_>>(),
                      "tempfile": tempfile,
                    })
                );
            } else {
                println!(
                    "{}",
                    json!({
                      "total": line_count,
                      "lines": String::from_utf8_lossy(&cmd_output.stdout).split("\n").collect::<Vec<_>>(),
                    })
                );
            }
            return Ok(());
        }
        Err(anyhow::Error::new(DummyError).context("No cmd specified"))
    }
}

pub fn main() -> Result<()> {
    let opt = Maple::from_args();

    if opt.try_exec_cmd().is_ok() {
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
            lines.push(text);
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
