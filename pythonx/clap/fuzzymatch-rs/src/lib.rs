#![feature(pattern)]

use extracted_fzy::match_and_score_with_positions;
use fuzzy_filter::truncate_long_matched_lines;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::collections::HashMap;

use std::str::pattern::Pattern;

#[inline]
fn find_start_at<'a, P: Pattern<'a>>(slice: &'a str, at: usize, pat: P) -> Option<usize> {
    slice[at..].find(pat).map(|i| at + i)
}

fn substr_scorer(niddle: &str, haystack: &str) -> Option<(f64, Vec<usize>)> {
    let haystack = haystack.to_lowercase();
    let haystack = haystack.as_str();

    let mut offset = 0;
    let mut positions = Vec::new();
    for sub_niddle in niddle.split_whitespace() {
        let sub_niddle = sub_niddle.to_lowercase();

        match find_start_at(haystack, offset, &sub_niddle) {
            Some(idx) => {
                offset = idx + sub_niddle.len();
                // For build without overflow checks this could be written as
                // `let mut pos = idx - 1;` with `|| { pos += 1; pos }` closure.
                let mut pos = idx;
                positions.resize_with(
                    positions.len() + sub_niddle.len(),
                    // Simple endless iterator for `idx..` range. Even though it's endless,
                    // it will iterate only `sub_niddle.len()` times.
                    || {
                        pos += 1;
                        pos - 1
                    },
                );
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
fn fuzzy_match(
    query: &str,
    candidates: Vec<String>,
    winwidth: usize,
) -> PyResult<(Vec<Vec<usize>>, Vec<String>, HashMap<String, String>)> {
    let scorer: Box<dyn Fn(&str) -> Option<(f64, Vec<usize>)>> = if query.contains(" ") {
        Box::new(|line: &str| substr_scorer(query, line))
    } else {
        Box::new(|line: &str| match_and_score_with_positions(query, line))
    };

    let mut ranked = candidates
        .into_iter()
        .filter_map(|line| scorer(&line).map(|(score, indices)| (line, score, indices)))
        .collect::<Vec<_>>();

    ranked.sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(v1).unwrap());

    // println!("ranked: {:?}", ranked);
    let (justify_ranked, mut justified_map) =
        truncate_long_matched_lines(ranked, winwidth, Some(4));

    let mut indices = Vec::with_capacity(justify_ranked.len());
    let mut filtered = Vec::with_capacity(justify_ranked.len());
    for (text, _, ids) in justify_ranked.into_iter() {
        /*
          if winwidth > 0 && justified_map.contains_key(&text) {
              let raw_line = justified_map.get(&text).unwrap().clone();
              let icon: String = raw_line.chars().take(2).collect();
              indices.push(ids.into_iter().map(|x| x + 4).collect::<Vec<_>>());
              let icon_truncated = format!("{}{}", icon, text);
              filtered.push(icon_truncated.clone());
              justified_map.insert(icon_truncated, raw_line);
          } else {
        */
        indices.push(ids);
        filtered.push(text);
        // }
    }

    Ok((indices, filtered, justified_map))
}

/// This module is a python module implemented in Rust.
#[pymodule]
fn fuzzymatch_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(fuzzy_match))?;

    Ok(())
}

#[test]
fn py_and_rs_subscore_should_work() {
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
        let py_result: (f64, Vec<usize>) = py_scorer
            .call1("substr_scorer", (niddle, haystack))
            .unwrap()
            .extract()
            .unwrap();
        let rs_result = substr_scorer(niddle, haystack).unwrap();
        assert_eq!(py_result, rs_result);
    }
}

#[test]
fn truncate_long_matched_lines_should_work() {
    use termion::style::{Invert, Reset};

    fn wrap_matches(line: &str, indices: &[usize]) -> String {
        let mut ret = String::new();
        let mut peekable = indices.iter().peekable();
        for (idx, ch) in line.chars().enumerate() {
            let next_id = **peekable.peek().unwrap_or(&&line.len());
            if next_id == idx {
                ret.push_str(format!("{}{}{}", Invert, ch, Reset).as_str());
                peekable.next();
            } else {
                ret.push(ch);
            }
        }

        ret
    }

    // let source = vec!["fuzzy-filter/target/debug/build/memchr-c1c0eb0055864ad6/build_script_build-c1c0eb0055864ad6.dSYM/Contents/Resources/DWARF/build_script_build-c1c0eb0055864ad6".into()];
    // let query = "srcsrsr";

    // let source = vec![
    // " fuzzy-filter/target/debug/deps/librustversion-b273394e6c9c64f6.dylib.dSYM/Contents/Resources/DWARF/librustversion-b273394e6c9c64f6.dylib".into(),
    // " fuzzy-filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib".into(),
    // " target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib".into(),
    // " target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib".into(),
    // ];
    // let query = "srlisrsr";

    let source = vec![" target/debug/deps/librustversion-903f7f9b8fc1cc96.dylib.dSYM/Contents/Resources/DWARF/librustversion-903f7f9b8fc1cc96.dylib".into()];
    let query = "srlis";

    let winwidth = 62;
    let (truncated_indices, truncated_lines, truncated_map) =
        fuzzy_match(query, source, winwidth).unwrap();

    println!("justified_map: {:#?}", truncated_map);
    for (idx, indices) in truncated_indices.into_iter().enumerate() {
        println!("truncated: {}", "-".repeat(winwidth as usize));

        println!(
            "truncated: {}",
            wrap_matches(&truncated_lines[idx], &indices[..])
        );
    }
}
