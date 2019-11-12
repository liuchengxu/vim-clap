use std::io::{self, BufRead};

use fuzzy_matcher::skim::fuzzy_indices;
use rayon::prelude::*;
use rff::match_and_score_with_positions;
use serde_json::json;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "maple")]
struct Opt {
    /// Initial query string
    #[structopt(index = 1, short, long)]
    query: String,

    /// Filter algorithm
    #[structopt(short, long, default_value = "fzy")]
    algo: String,
}

pub fn main() {
    let opt = Opt::from_args();

    let query = &*opt.query;
    let algo = &*opt.algo;

    let scorer = |line: &str| match algo.into() {
        "skim" => fuzzy_indices(line, query).map(|(score, indices)| (score as f64, indices)),
        "fzy" => {
            match_and_score_with_positions(query, line).map(|(_, score, indices)| (score, indices))
        }
        _ => unreachable!(),
    };

    // Result<Option<T>> => T
    let mut ranked = io::stdin()
        .lock()
        .lines()
        .filter_map(|lines_iter| {
            lines_iter
                .ok()
                .and_then(|line| scorer(&line).map(|(score, indices)| (line, score, indices)))
        })
        .collect::<Vec<_>>();

    ranked.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

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
