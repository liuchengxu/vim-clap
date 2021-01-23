use std::collections::HashMap;

use pyo3::{prelude::*, wrap_pyfunction};

use filter::matcher::{Algo, Bonus, Matcher};
use printer::truncate_long_matched_lines;

/// Pass a Vector of lines to Vim for setting them in Vim with one single API call.
type LinesInBatch = Vec<String>;

/// Each line's matched indices of LinesInBatch.
type MatchedIndicesInBatch = Vec<Vec<usize>>;

/// NOTE: TruncatedMap is ought to be HashMap<usize, String>,
/// but there is an issue when converting to call result to Vim Dict in python dynamic call,
/// therefore hereby has to use HashMap<String, String> instead.
type TruncatedMapInfo = HashMap<String, String>;

/// Filter the candidates given query using the fzy algorithm
#[pyfunction]
fn fuzzy_match(
    query: &str,
    candidates: Vec<String>,
    winwidth: usize,
    enable_icon: bool,
    match_type: String,
    bonus: String,
    recent_files: Vec<String>,
) -> PyResult<(MatchedIndicesInBatch, LinesInBatch, TruncatedMapInfo)> {
    let matcher = Matcher::new_with_bonuses(
        if query.contains(' ') {
            Algo::SubString
        } else {
            Algo::Fzy
        },
        match_type.into(),
        vec![bonus.into(), Bonus::RecentFiles(recent_files)],
    );
    let do_match = |line: &str| {
        if enable_icon {
            // " " is 4 bytes, but the offset of highlight is 2.
            matcher
                .do_match(&line[4..].into(), query)
                .map(|(score, indices)| (score, indices.into_iter().map(|x| x + 4).collect()))
        } else {
            matcher.do_match(&line.into(), query)
        }
    };

    let mut ranked = candidates
        .into_iter()
        .filter_map(|line| do_match(&line).map(|(score, indices)| (line.into(), score, indices)))
        .collect::<Vec<_>>();

    ranked.sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(v1).unwrap());

    // 2 = chars(icon)
    let skipped = if enable_icon { Some(2) } else { None };
    let (lines, truncated_map) = truncate_long_matched_lines(ranked, winwidth, skipped);

    let mut indices = Vec::with_capacity(lines.len());
    let mut filtered = Vec::with_capacity(lines.len());
    for (text, _, ids) in lines.into_iter() {
        indices.push(ids);
        filtered.push(text);
    }

    Ok((
        indices,
        filtered,
        truncated_map
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    ))
}

/// This module is a python module implemented in Rust.
#[pymodule]
fn fuzzymatch_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(fuzzy_match))?;

    Ok(())
}

#[test]
fn py_and_rs_subscore_should_work() {
    use filter::matcher::substring::substr_indices as substr_scorer;
    use pyo3::{prelude::*, types::PyModule};
    use std::fs;

    let cur_dir = std::env::current_dir().unwrap();
    let py_path = cur_dir.parent().unwrap().join("scorer.py");
    let py_source_code = fs::read_to_string(py_path).unwrap();

    let gil = Python::acquire_gil();
    let py = gil.python();
    let py_scorer = PyModule::from_code(py, &py_source_code, "scorer.py", "scorer").unwrap();

    let test_cases = vec![
        ("su ou", "substr_scorer_should_work"),
        ("su ork", "substr_scorer_should_work"),
    ];

    for (niddle, haystack) in test_cases.into_iter() {
        let py_result: (i64, Vec<usize>) = py_scorer
            .call1("substr_scorer", (niddle, haystack))
            .unwrap()
            .extract()
            .map(|(score, positions): (f64, Vec<usize>)| (score as i64, positions))
            .unwrap();
        let rs_result = substr_scorer(haystack, niddle).unwrap();
        assert_eq!(py_result, rs_result);
    }
}

#[test]
fn test_skip_icon() {
    let lines = vec![" .dependabot/config.yml".into(), " .editorconfig".into()];
    let query = "con";
    println!(
        "ret: {:#?}",
        fuzzy_match(
            query,
            lines,
            62,
            true,
            "Full".into(),
            "FileName".into(),
            vec![]
        )
    );
}
