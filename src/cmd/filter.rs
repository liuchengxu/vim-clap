use std::path::PathBuf;

use anyhow::Result;
use fuzzy_filter::{fuzzy_filter_and_rank, truncate_long_matched_lines, Algo};

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
        let (justified_payload, mut justified_map) = truncate_long_matched_lines(payload, 62, None);
        let mut lines = Vec::with_capacity(number);
        let mut indices = Vec::with_capacity(number);
        if enable_icon {
            for (text, _, idxs) in justified_payload {
                let iconized = prepend_icon(&text);
                // if let Some(x) = justified_map.get(&text) {
                // println!("iconized: {}", iconized);
                // justified_map.insert(iconized.clone(), x.clone());
                // }

                lines.push(iconized);
                indices.push(idxs);
            }
        } else {
            for (text, _, idxs) in justified_payload {
                lines.push(text);
                indices.push(idxs);
            }
        }
        // let justified_map = justified_map
        // .into_iter()
        // .map(|(a, b)| (b, a))
        // .collect::<std::collections::HashMap<_, _>>();
        if justified_map.is_empty() {
            println_json!(total, lines, indices);
        } else {
            println_json!(total, lines, indices, justified_map);
        }
    } else {
        for (text, _, indices) in ranked.iter() {
            println_json!(text, indices);
        }
    }

    Ok(())
}
