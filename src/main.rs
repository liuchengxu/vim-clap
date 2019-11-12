use std::io::{self, BufRead};

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
