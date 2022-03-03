use std::collections::HashMap;
use std::slice::IterMut;

use types::FilteredItem;

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
) -> Option<(String, Vec<usize>)> {
    use crate::trimmer::v1::trim_text;

    if line.is_empty() || indices.is_empty() {
        return None;
    }

    if let Some(skipped) = skipped {
        let container_width = winwidth - skipped;
        let text = line.chars().skip(skipped).collect::<String>();
        indices.iter_mut().for_each(|x| *x -= 2);
        // TODO: tabstop is not always 4, `:h vim9-differences`
        trim_text(&text, indices, container_width, 4).map(|(text, mut indices)| {
            (
                format!("{}{}", line.chars().take(skipped).collect::<String>(), text),
                {
                    indices.iter_mut().for_each(|x| *x += 2);
                    indices
                },
            )
        })
    } else {
        trim_text(line, indices, winwidth, 4)
    }
}

const MAX_LINE_LEN: usize = 500;

/// Long matched lines can cause the matched items invisible.
///
/// # Arguments
///
/// - winwidth: width of the display window.
/// - skipped: number of skipped chars, used when need to skip the leading icons.
pub fn truncate_long_matched_lines<T>(
    items: IterMut<FilteredItem<T>>,
    winwidth: usize,
    skipped: Option<usize>,
) -> LinesTruncatedMap {
    let mut truncated_map = HashMap::new();
    let winwidth = winwidth - WINWIDTH_OFFSET;
    items.enumerate().for_each(|(lnum, mut filtered_item)| {
        let origin_display_text = filtered_item.source_item.display_text();

        // Truncate the text simply if it's too long.
        if origin_display_text.len() > MAX_LINE_LEN {
            let display_text: String = origin_display_text.chars().take(1000).collect();
            filtered_item.display_text = Some(display_text);
            filtered_item.match_indices = filtered_item
                .match_indices
                .iter()
                .filter(|x| **x < 1000)
                .copied()
                .collect();
        } else if let Some((truncated, truncated_indices)) = truncate_line_v1(
            origin_display_text,
            &mut filtered_item.match_indices,
            winwidth,
            skipped,
        ) {
            truncated_map.insert(lnum + 1, origin_display_text.to_string());

            filtered_item.display_text = Some(truncated);
            filtered_item.match_indices = truncated_indices;
        }
    });
    truncated_map
}

pub fn truncate_long_matched_lines_v0<T>(
    items: IterMut<FilteredItem<T>>,
    winwidth: usize,
    skipped: Option<usize>,
) -> LinesTruncatedMap {
    let mut truncated_map = HashMap::new();
    let winwidth = winwidth - WINWIDTH_OFFSET;
    items.enumerate().for_each(|(lnum, filtered_item)| {
        let line = filtered_item.source_item_display_text();

        if let Some((truncated, truncated_indices)) =
            crate::trimmer::v0::trim_text(line, &filtered_item.match_indices, winwidth, skipped)
        {
            truncated_map.insert(lnum + 1, line.to_string());

            filtered_item.display_text = Some(truncated);
            filtered_item.match_indices = truncated_indices;
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

            if let Some((truncated, truncated_indices)) =
                truncate_line_v1(&line, &mut indices, winwidth, skipped)
            {
                truncated_map.insert(lnum, line);
                (truncated, truncated_indices)
            } else {
                (line, indices)
            }
        })
        .unzip();
    (lines, indices, truncated_map)
}
