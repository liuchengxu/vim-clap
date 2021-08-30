use std::collections::HashMap;

use pyo3::{prelude::*, wrap_pyfunction};

use filter::{
    matcher::{Bonus, FuzzyAlgorithm, MatchType, Matcher},
    FilteredItem, Query, SourceItem,
};
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
    bonuses: Vec<Bonus>,
}

impl From<HashMap<String, String>> for MatchContext {
    fn from(ctx: HashMap<String, String>) -> Self {
        let winwidth = ctx
            .get("winwidth")
            .and_then(|x| x.parse::<usize>().ok())
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

        let mut bonuses = vec![bonus_type];
        if let Some(language) = ctx.get("language") {
            bonuses.push(Bonus::Language(language.into()));
        }

        Self {
            winwidth,
            enable_icon,
            match_type,
            bonuses,
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
        mut bonuses,
    } = context.into();

    bonuses.push(Bonus::RecentFiles(recent_files.into()));

    let matcher = Matcher::with_bonuses(FuzzyAlgorithm::Fzy, match_type, bonuses);

    let query: Query = query.into();
    let do_match = |line: &str| {
        if enable_icon {
            // " " is 4 bytes, but the offset of highlight is 2.
            matcher
                .match_query(&SourceItem::from(&line[4..]), &query)
                .map(|(score, indices)| (score, indices.into_iter().map(|x| x + 4).collect()))
        } else {
            matcher.match_query(&SourceItem::from(line), &query)
        }
    };

    let mut ranked = candidates
        .into_iter()
        .filter_map(|line| {
            do_match(&line).map(|(score, indices)| (Into::<SourceItem>::into(line), score, indices))
        })
        .map(Into::<FilteredItem>::into)
        .collect::<Vec<_>>();

    ranked.sort_unstable_by(|v1, v2| v2.score.partial_cmp(&v1.score).unwrap());

    // 2 = chars(icon)
    let skipped = if enable_icon { Some(2) } else { None };
    let truncated_map = truncate_long_matched_lines(ranked.iter_mut(), winwidth, skipped);

    let (filtered, indices): (Vec<_>, Vec<_>) = ranked
        .into_iter()
        .map(|filtered_item| {
            (
                filtered_item.display_text().to_owned(),
                filtered_item.match_indices,
            )
        })
        .unzip();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py_and_rs_subscore_should_work() {
        use filter::matcher::substring::substr_indices as substr_scorer;
        use pyo3::{prelude::*, types::PyModule};
        use std::fs;

        let cur_dir = std::env::current_dir().unwrap();
        let py_path = cur_dir.parent().unwrap().join("scorer.py");
        let py_source_code = fs::read_to_string(py_path).unwrap();

        pyo3::prepare_freethreaded_python();

        let gil = Python::acquire_gil();
        let py = gil.python();
        let py_scorer = PyModule::from_code(py, &py_source_code, "scorer.py", "scorer").unwrap();

        let test_cases = vec![
            ("su ou", "substr_scorer_should_work"),
            ("su ork", "substr_scorer_should_work"),
        ];

        for (niddle, haystack) in test_cases.into_iter() {
            let py_result: (i64, Vec<usize>) = py_scorer
                .getattr("substr_scorer")
                .unwrap()
                .call1((niddle, haystack))
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
}
