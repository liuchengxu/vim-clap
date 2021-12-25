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

// https://stackoverflow.com/questions/51982999/slice-a-string-containing-unicode-chars
#[inline]
pub fn utf8_str_slice(line: &str, start: usize, end: usize) -> String {
    line.chars().take(end).skip(start).collect()
}

fn truncate_line_impl(
    line: &str,
    indices: &[usize],
    winwidth: usize,
    skipped: Option<usize>,
) -> Option<(String, Vec<usize>)> {
    if line.is_empty() || indices.is_empty() {
        return None;
    }

    if let Some(skipped) = skipped {
        let container_width = winwidth - skipped;

        let text = line.chars().skip(skipped).collect::<String>();
        // let indices = indices.iter().map(|x| x + skipped).collect::<Vec<_>>();

        crate::printer::trim_text(&text, indices, container_width).map(|(text, indices)| {
            (
                format!("{}{}", line.chars().take(skipped).collect::<String>(), text),
                indices,
            )
        })
    } else {
        let container_width = winwidth;
        let text = line;
        crate::printer::trim_text(text, indices, container_width)
    }
}

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
    items.enumerate().for_each(|(lnum, filtered_item)| {
        let line = filtered_item.source_item_display_text();

        if let Some((truncated, truncated_indices)) =
            truncate_line_impl(line, &filtered_item.match_indices, winwidth, skipped)
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
        .map(|(line, indices)| {
            lnum += 1;

            if let Some((truncated, truncated_indices)) =
                truncate_line_impl(&line, &indices, winwidth, skipped)
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
