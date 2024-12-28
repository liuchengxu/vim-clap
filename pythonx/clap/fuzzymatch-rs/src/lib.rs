use std::collections::HashMap;
use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

use matcher::{Bonus, MatchResult, MatcherBuilder};
use printer::truncate_item_output_text_v0;
use types::{ClapItem, MatchScope, SourceItem};

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
    match_scope: MatchScope,
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

        let match_scope = ctx
            .get("match_scope")
            .map(Into::into)
            .unwrap_or(MatchScope::Full);

        let bonus_type = ctx.get("bonus_type").map(Into::into).unwrap_or(Bonus::None);

        let mut bonuses = vec![bonus_type];
        if let Some(language) = ctx.get("language") {
            bonuses.push(Bonus::Language(language.into()));
        }

        Self {
            winwidth,
            enable_icon,
            match_scope,
            bonuses,
        }
    }
}

#[derive(Debug)]
struct LineWithIcon(String);

impl ClapItem for LineWithIcon {
    fn raw_text(&self) -> &str {
        self.0.as_str()
    }

    fn match_text(&self) -> &str {
        &self.0[4..]
    }

    fn match_result_callback(&self, match_result: MatchResult) -> MatchResult {
        let mut match_result = match_result;
        match_result.indices.iter_mut().for_each(|x| {
            *x += 4;
        });
        match_result
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
        match_scope,
        mut bonuses,
    } = context.into();

    bonuses.push(Bonus::RecentFiles(recent_files.into()));

    let matcher = MatcherBuilder::new()
        .bonuses(bonuses)
        .match_scope(match_scope)
        .build(query.into());

    let mut ranked = candidates
        .into_iter()
        .filter_map(|line: String| {
            let item: Arc<dyn ClapItem> = if enable_icon {
                Arc::new(LineWithIcon(line))
            } else {
                Arc::new(SourceItem::from(line))
            };
            matcher.match_item(item)
        })
        .collect::<Vec<_>>();

    ranked.sort_unstable_by(|v1, v2| v2.cmp(v1));

    // " " is 4 bytes, but the offset of highlight is 2.
    // 2 = chars(icon)
    let skipped = if enable_icon { Some(2) } else { None };
    let truncated_map = truncate_item_output_text_v0(ranked.iter_mut(), winwidth, skipped);

    let (lines, indices): (Vec<_>, Vec<_>) = ranked
        .into_iter()
        .map(|matched_item| {
            (
                matched_item.display_text().to_string(),
                matched_item.indices,
            )
        })
        .unzip();

    Ok((
        indices,
        lines,
        truncated_map
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    ))
}

/// This module is a python module implemented in Rust.
#[pymodule]
fn fuzzymatch_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(fuzzy_match, m)?)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py_and_rs_subscore_should_work() {
        use matcher::substring::substr_indices as substr_scorer;
        use pyo3::ffi::c_str;
        use pyo3::prelude::*;
        use pyo3::types::PyModule;
        use types::CaseMatching;

        Python::with_gil(|py| {
            let py_scorer = PyModule::from_code(
                py,
                c_str!(include_str!("../../scorer.py")),
                c_str!("scorer.py"),
                c_str!("scorer"),
            )
            .unwrap();

            let test_cases = vec![
                ("su ou", "substr_scorer_should_work"),
                ("su ork", "substr_scorer_should_work"),
            ];

            for (needle, haystack) in test_cases.into_iter() {
                let py_result: (i32, Vec<usize>) = py_scorer
                    .getattr("substr_scorer")
                    .unwrap()
                    .call1((needle, haystack))
                    .unwrap()
                    .extract()
                    .map(|(score, positions): (f64, Vec<usize>)| (score as i32, positions))
                    .unwrap();
                let rs_result = substr_scorer(haystack, needle, CaseMatching::Smart).unwrap();
                assert_eq!(py_result, rs_result);
            }
        });
    }

    #[test]
    fn test_skip_icon() {
        let lines = vec![" .dependabot/config.yml".into(), " .editorconfig".into()];
        let query = "con";

        let context: HashMap<String, String> = vec![
            ("winwidth", "62"),
            ("enable_icon", "True"),
            ("match_scope", "Full"),
            ("bonus_type", "FileName"),
        ]
        .into_iter()
        .map(|(x, y)| (x.into(), y.into()))
        .collect();

        println!("ret: {:#?}", fuzzy_match(query, lines, vec![], context));
    }
}
