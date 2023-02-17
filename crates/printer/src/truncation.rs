use crate::trimmer::v1::{trim_text as trim_text_v1, TrimmedText};
use std::collections::HashMap;
use std::slice::IterMut;
use types::MatchedItem;

/// Line number of Vim is 1-based.
pub type VimLineNumber = usize;

/// Map of truncated line number to original full line.
///
/// Can't use HashMap<String, String> since we can't tell the original lines in the following case:
///
/// //  ..{ version = "1.0", features = ["derive"] }
/// //  ..{ version = "1.0", features = ["derive"] }
/// //  ..{ version = "1.0", features = ["derive"] }
/// //  ..{ version = "1.0", features = ["derive"] }
///
pub type LinesTruncatedMap = HashMap<VimLineNumber, String>;

/// sign column width 2
#[cfg(not(test))]
const WINWIDTH_OFFSET: usize = 4;

#[cfg(test)]
const WINWIDTH_OFFSET: usize = 0;

fn truncate_line_v1(
    line: &str,
    indices: &mut [usize],
    winwidth: usize,
    skipped: Option<usize>,
) -> Option<TrimmedText> {
    if line.is_empty() || indices.is_empty() {
        return None;
    }

    if let Some(skipped) = skipped {
        let container_width = winwidth - skipped;
        let text = line.chars().skip(skipped).collect::<String>();
        indices.iter_mut().for_each(|x| *x -= 2);
        // TODO: tabstop is not always 4, `:h vim9-differences`
        trim_text_v1(&text, indices, container_width, 4).map(|trimmed_text| {
            let TrimmedText { text, mut indices } = trimmed_text;
            let truncated_text = text;
            let mut text = String::with_capacity(skipped + truncated_text.len());
            line.chars().take(skipped).for_each(|c| text.push(c));
            text.push_str(&truncated_text);
            indices.iter_mut().for_each(|x| *x += 2);
            TrimmedText::new(text, indices)
        })
    } else {
        trim_text_v1(line, indices, winwidth, 4)
    }
}

const MAX_LINE_LEN: usize = 500;

/// Truncate the output text of item if it's too long.
///
/// # Arguments
///
/// - `winwidth`: width of the display window.
/// - `skipped`: number of skipped chars, used when need to skip the leading icons.
pub(super) fn truncate_item_output_text(
    items: IterMut<MatchedItem>,
    winwidth: usize,
    skipped: Option<usize>,
) -> LinesTruncatedMap {
    let mut truncated_map = HashMap::new();
    let winwidth = winwidth - WINWIDTH_OFFSET;
    items.enumerate().for_each(|(lnum, mut matched_item)| {
        let output_text = matched_item.output_text().to_string();

        // Truncate the text simply if it's too long.
        if output_text.len() > MAX_LINE_LEN {
            let truncated_output_text: String = output_text.chars().take(1000).collect();
            matched_item.display_text = Some(truncated_output_text);
            matched_item.indices.retain(|&x| x < 1000);
        } else if let Some(TrimmedText { text, indices }) =
            truncate_line_v1(&output_text, &mut matched_item.indices, winwidth, skipped)
        {
            truncated_map.insert(lnum + 1, output_text);

            matched_item.display_text = Some(text);
            matched_item.indices = indices;
        } else {
            // Use the origin `output_text` as the final `display_text`.
            matched_item.display_text.replace(output_text);
        }
    });
    truncated_map
}

pub fn truncate_item_output_text_v0(
    items: IterMut<MatchedItem>,
    winwidth: usize,
    skipped: Option<usize>,
) -> LinesTruncatedMap {
    let mut truncated_map = HashMap::new();
    let winwidth = winwidth - WINWIDTH_OFFSET;
    items.enumerate().for_each(|(lnum, matched_item)| {
        let output_text = matched_item.item.output_text();

        if let Some((truncated_output_text, truncated_indices)) =
            crate::trimmer::v0::trim_text(&output_text, &matched_item.indices, winwidth, skipped)
        {
            truncated_map.insert(lnum + 1, output_text.to_string());

            matched_item.display_text = Some(truncated_output_text);
            matched_item.indices = truncated_indices;
        }
    });
    truncated_map
}

pub fn truncate_grep_lines(
    lines: impl IntoIterator<Item = String>,
    indices: impl IntoIterator<Item = Vec<usize>>,
    winwidth: usize,
    skipped: Option<usize>,
) -> (Vec<String>, Vec<Vec<usize>>, LinesTruncatedMap) {
    let mut truncated_map = HashMap::new();
    let mut lnum = 0usize;
    let winwidth = winwidth - WINWIDTH_OFFSET;
    let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = lines
        .into_iter()
        .zip(indices.into_iter())
        .map(|(line, mut indices)| {
            lnum += 1;

            if let Some(trimmed_text) = truncate_line_v1(&line, &mut indices, winwidth, skipped) {
                truncated_map.insert(lnum, line);
                (trimmed_text.text, trimmed_text.indices)
            } else {
                (line, indices)
            }
        })
        .unzip();
    (lines, indices, truncated_map)
}
