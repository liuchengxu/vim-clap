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

#[inline]
pub fn scorer(algo: &str, query: &str, line: &str) -> Option<(f64, Vec<usize>)> {
    match algo.into() {
        "skim" => fuzzy_indices(line, query).map(|(score, indices)| (score as f64, indices)),
        "fzy" => {
            match_and_score_with_positions(query, line).map(|(_, score, indices)| (score, indices))
        }
        _ => unreachable!(),
    }
}

pub fn main() {
    let opt = Opt::from_args();

    let query = &*opt.query;
    let algo = &*opt.algo;

    let mut ranked = vec![];

    for line in io::stdin().lock().lines() {
        if let Ok(line) = line {
            if let Some((score, indices)) = scorer(algo, query, &line) {
                ranked.push((line, score, indices));
            }
        }
    }

    ranked.par_sort_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

    for (text, _, indices) in ranked.iter() {
        let matched = json!({
          "text": text,
          "indices": indices,
        });
        println!("{}", matched.to_string());
    }
}
