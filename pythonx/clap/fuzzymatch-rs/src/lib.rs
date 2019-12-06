use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use rff::match_and_score_with_positions;

#[pyfunction]
/// Filter the candidates given query using the fzy algorithm
fn fuzzy_match(query: &str, candidates: Vec<String>) -> PyResult<(Vec<Vec<usize>>, Vec<String>)> {
    let scorer = |line: &str| {
        match_and_score_with_positions(&query, line).map(|(_, score, indices)| (score, indices))
    };

    let mut ranked = candidates
        .into_iter()
        .filter_map(|line| scorer(&line).map(|(score, indices)| (line, score, indices)))
        .collect::<Vec<_>>();

    ranked.sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(v1).unwrap());

    let mut indices = Vec::new();
    let mut filtered = Vec::new();
    for (text, _, ids) in ranked.iter() {
        indices.push(ids.clone());
        filtered.push(text.clone());
    }

    Ok((indices, filtered))
}

/// This module is a python module implemented in Rust.
#[pymodule]
fn fuzzymatch_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(fuzzy_match))?;

    Ok(())
}
