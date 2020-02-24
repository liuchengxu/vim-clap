use std::io::{self, BufRead};
use std::path::PathBuf;

use anyhow::Result;
use extracted_fzy::match_and_score_with_positions;
use fuzzy_matcher::skim::fuzzy_indices;
use rayon::prelude::*;

use crate::cmd::Algo;
use crate::icon::prepend_icon;

pub fn apply_fuzzy_filter_and_rank(
    query: &str,
    input: Option<PathBuf>,
    algo: Option<Algo>,
) -> Result<Vec<(String, f64, Vec<usize>)>> {
    let algo = algo.unwrap_or(Algo::Fzy);

    let scorer = |line: &str| match algo {
        Algo::Skim => fuzzy_indices(line, &query).map(|(score, indices)| (score as f64, indices)),
        Algo::Fzy => match_and_score_with_positions(&query, line),
    };

    // Result<Option<T>> => T
    let mut ranked = if let Some(input) = input {
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

    Ok(ranked)
}

pub fn run(
    query: String,
    input: Option<PathBuf>,
    algo: Option<Algo>,
    number: Option<usize>,
    enable_icon: bool,
) -> Result<()> {
    let ranked = apply_fuzzy_filter_and_rank(&query, input, algo)?;

    if let Some(number) = number {
        let total = ranked.len();
        let payload = ranked.into_iter().take(number);
        let mut lines = Vec::with_capacity(number);
        let mut indices = Vec::with_capacity(number);
        if enable_icon {
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
