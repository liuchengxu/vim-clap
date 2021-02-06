use std::collections::HashMap;

use pyo3::{prelude::*, wrap_pyfunction};

use filter::matcher::{Algo, Bonus, MatchType, Matcher};
use printer::truncate_long_matched_lines;

/// Pass a Vector of lines to Vim for setting them in Vim with one single API call.
type LinesInBatch = Vec<String>;

/// Each line's matched indices of LinesInBatch.
type MatchedIndicesInBatch = Vec<Vec<usize>>;

/// NOTE: TruncatedMap is ought to be HashMap<usize, String>,
/// but there is an issue when converting to call result to Vim Dict in python dynamic call,
/// therefore hereby has to use HashMap<String, String> instead.
type TruncatedMapInfo = HashMap<String, String>;

const DEFAULT_WINWIDTH: usize = 80;

#[derive(Debug, Clone)]
struct MatchContext {
    winwidth: usize,
    enable_icon: bool,
    match_type: MatchType,
    bonus_type: Bonus,
}

impl From<HashMap<String, String>> for MatchContext {
    fn from(ctx: HashMap<String, String>) -> Self {
        let winwidth = ctx
            .get("winwidth")
            .map(|x| x.parse::<usize>().unwrap_or(DEFAULT_WINWIDTH))
            .unwrap_or(DEFAULT_WINWIDTH);

        let enable_icon = ctx
            .get("enable_icon")
            .map(|x| x.to_lowercase() == "true")
            .unwrap_or(false);

        let match_type = ctx
            .get("match_type")
            .map(Into::into)
            .unwrap_or(MatchType::Full);

        let bonus_type = ctx.get("bonus_type").map(Into::into).unwrap_or(Bonus::None);

        Self {
            winwidth,
            enable_icon,
            match_type,
            bonus_type,
        }
    }
}

/// Filter the candidates synchorously given `query` and `candidates`.
///
/// `recent_files` and `context` are the full context for matching each item.
#[pyfunction]
fn fuzzy_match(
    query: &str,
    candidates: Vec<String>,
    recent_files: Vec<String>,
    context: HashMap<String, String>,
) -> PyResult<(MatchedIndicesInBatch, LinesInBatch, TruncatedMapInfo)> {
    let MatchContext {
        winwidth,
        enable_icon,
        match_type,
        bonus_type,
    } = context.into();

    let matcher = Matcher::new_with_bonuses(
        if query.contains(' ') {
            Algo::SubString
        } else {
            Algo::Fzy
        },
        match_type,
        vec![bonus_type, Bonus::RecentFiles(recent_files)],
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

    let (filtered, indices): (Vec<_>, Vec<_>) =
        lines.into_iter().map(|(text, _, ids)| (text, ids)).unzip();

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

    let context: HashMap<String, String> = vec![
        ("winwidth", "62"),
        ("enable_icon", "True"),
        ("match_type", "Full"),
        ("bonus_type", "FileName"),
    ]
    .into_iter()
    .map(|(x, y)| (x.into(), y.into()))
    .collect();

    println!("ret: {:#?}", fuzzy_match(query, lines, vec![], context));
}
