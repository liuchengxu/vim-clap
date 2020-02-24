use std::path::PathBuf;

use anyhow::Result;
use fuzzy_filter::{fuzzy_filter_and_rank, Algo};

use crate::icon::prepend_icon;

pub fn run(
    query: String,
    input: Option<PathBuf>,
    algo: Option<Algo>,
    number: Option<usize>,
    enable_icon: bool,
) -> Result<()> {
    let ranked = fuzzy_filter_and_rank(&query, input, algo.unwrap_or(Algo::Fzy))?;

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
