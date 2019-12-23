use std::io::{self, BufRead};
use std::path::PathBuf;

use fuzzy_matcher::skim::fuzzy_indices;
use rayon::prelude::*;
use rff::match_and_score_with_positions;
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
struct Opt {
    /// Initial query string
    #[structopt(index = 1, short, long)]
    query: String,

    /// Filter algorithm
    #[structopt(short, long, possible_values = &Algo::variants(), case_insensitive = true)]
    algo: Option<Algo>,

    /// Read input from a file instead of stdin, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    /// Print the top number of filtered items.
    ///
    /// The returned JSON has three fields:
    ///   - total: total number of initial filtered result set.
    ///   - lines: text lines used for displaying directly.
    ///   - indices: the indices of matched elements per line, used for the highlight purpose.
    #[structopt(short = "n", long = "number")]
    number: Option<usize>,
}

pub fn main() {
    let opt = Opt::from_args();

    let query = &*opt.query;
    let algo = opt.algo.unwrap_or(Algo::Fzy);

    let scorer = |line: &str| match algo {
        Algo::Skim => fuzzy_indices(line, query).map(|(score, indices)| (score as f64, indices)),
        Algo::Fzy => {
            match_and_score_with_positions(query, line).map(|(_, score, indices)| (score, indices))
        }
    };

    // Result<Option<T>> => T
    let mut ranked = if let Some(input) = opt.input {
        std::fs::read_to_string(input)
            .expect("Input file does not exist")
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
        let payload = ranked.into_iter().take(number).collect::<Vec<_>>();
        let mut lines = Vec::with_capacity(number);
        let mut indices = Vec::with_capacity(number);
        for (text, _, idxs) in payload.iter() {
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
}
