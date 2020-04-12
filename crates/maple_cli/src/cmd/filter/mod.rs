pub mod dynamic;
mod scoring_line;

pub use dynamic::dyn_fuzzy_filter_and_rank as dyn_run;
pub use scoring_line::ContentFiltering;

use std::collections::HashMap;

use anyhow::Result;
use fuzzy_filter::{fuzzy_filter_and_rank, truncate_long_matched_lines, Algo, Source};

use icon::{IconPainter, ICON_LEN};

/// Returns the info of the truncated top items ranked by the filtering score.
fn process_top_items<T>(
    top_size: usize,
    top_list: impl IntoIterator<Item = (String, T, Vec<usize>)>,
    winwidth: usize,
    icon_painter: Option<IconPainter>,
) -> (Vec<String>, Vec<Vec<usize>>, HashMap<String, String>) {
    let (truncated_lines, truncated_map) = truncate_long_matched_lines(top_list, winwidth, None);
    let mut lines = Vec::with_capacity(top_size);
    let mut indices = Vec::with_capacity(top_size);
    if let Some(painter) = icon_painter {
        for (text, _, idxs) in truncated_lines {
            let iconized = if let Some(origin_text) = truncated_map.get(&text) {
                format!("{} {}", painter.get_icon(origin_text), text)
            } else {
                painter.paint(&text)
            };
            lines.push(iconized);
            indices.push(idxs.into_iter().map(|x| x + ICON_LEN).collect());
        }
    } else {
        for (text, _, idxs) in truncated_lines {
            lines.push(text);
            indices.push(idxs);
        }
    }
    (lines, indices, truncated_map)
}

pub fn run<I: Iterator<Item = String>>(
    query: &str,
    source: Source<I>,
    algo: Option<Algo>,
    number: Option<usize>,
    icon_painter: Option<IconPainter>,
    winwidth: Option<usize>,
) -> Result<()> {
    let ranked = fuzzy_filter_and_rank(query, source, algo.unwrap_or(Algo::Fzy))?;

    if let Some(number) = number {
        let total = ranked.len();
        let (lines, indices, truncated_map) = process_top_items(
            number,
            ranked.into_iter().take(number),
            winwidth.unwrap_or(62),
            icon_painter,
        );
        if truncated_map.is_empty() {
            println_json!(total, lines, indices);
        } else {
            println_json!(total, lines, indices, truncated_map);
        }
    } else {
        for (text, _, indices) in ranked.iter() {
            println_json!(text, indices);
        }
    }

    Ok(())
}
