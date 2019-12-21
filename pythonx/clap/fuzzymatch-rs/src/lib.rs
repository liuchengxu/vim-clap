#![feature(pattern)]

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use rff::match_and_score_with_positions;

use std::str::pattern::Pattern;

#[inline]
fn find_start_at<'a, P: Pattern<'a>>(slice: &'a str, at: usize, pat: P) -> Option<usize> {
    slice[at..].find(pat).map(|i| at + i)
}

fn substr_scorer(niddle: &str, haystack: &str) -> Option<(f64, Vec<usize>)> {
    let niddle = niddle.to_lowercase();
    let haystack = haystack.to_lowercase();
    let indices: Vec<usize> = (0..haystack.len()).collect();
    let haystack = haystack.as_str();

    let mut offset = 0;
    let mut positions = Vec::new();
    for sub_niddle in niddle.split_whitespace() {
        match find_start_at(haystack, offset, sub_niddle) {
            Some(idx) => {
                offset = idx;
                let niddle_len = sub_niddle.len();
                positions.extend_from_slice(&indices[offset..offset + niddle_len]);
                offset += niddle_len;
            }
            None => return None,
        }
    }

    if positions.is_empty() {
        return Some((0f64, positions));
    }

    let last_pos = positions.last().unwrap();
    let match_len = (last_pos + 1 - positions[0]) as f64;

    Some((
        ((2f64 / (positions[0] + 1) as f64) + 1f64 / (last_pos + 1) as f64 - match_len),
        positions,
    ))
}

#[pyfunction]
/// Filter the candidates given query using the fzy algorithm
fn fuzzy_match(query: &str, candidates: Vec<String>) -> PyResult<(Vec<Vec<usize>>, Vec<String>)> {
    let scorer: Box<dyn Fn(&str) -> Option<(f64, Vec<usize>)>> = if query.contains(" ") {
        Box::new(|line: &str| substr_scorer(query, line))
    } else {
        Box::new(|line: &str| {
            match_and_score_with_positions(query, line).map(|(_, score, indices)| (score, indices))
        })
    };

    let mut ranked = candidates
        .into_iter()
        .filter_map(|line| scorer(&line).map(|(score, indices)| (line, score, indices)))
        .collect::<Vec<_>>();

    ranked.sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(v1).unwrap());

    let mut indices = Vec::with_capacity(ranked.len());
    let mut filtered = Vec::with_capacity(ranked.len());
    for (text, _, ids) in ranked.into_iter() {
        indices.push(ids);
        filtered.push(text);
    }

    Ok((indices, filtered))
}

/// This module is a python module implemented in Rust.
#[pymodule]
fn fuzzymatch_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(fuzzy_match))?;

    Ok(())
}

#[test]
fn substr_scorer_should_work() {
    let niddle = "su ork";
    let haystack = "substr_scorer_should_work";
    println!("{:?}", substr_scorer(niddle, haystack));
    let niddle = "su ou";
    let haystack = "substr_scorer_should_work";
    println!("{:?}", substr_scorer(niddle, haystack));

    let niddle = "su  ou";
    let haystack = "substr_scorer_should_work";
    println!("{:?}", fuzzy_match(niddle, vec![haystack.into()]));
}
