use std::collections::HashMap;
use std::slice::IterMut;

use types::FilteredItem;

pub const DOTS: &str = "..";

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
    let last_idx = indices.last()?;
    if *last_idx > winwidth {
        let mut start = *last_idx - winwidth;
        if start >= indices[0] || (indices.len() > 1 && *last_idx - start > winwidth) {
            start = indices[0];
        }
        let line_len = line.len();
        // [--------------------------]
        // [-----------------------------------------------------------------xx--x--]
        for _ in 0..3 {
            if indices[0] - start >= DOTS.len() && line_len - start >= winwidth {
                start += DOTS.len();
            } else {
                break;
            }
        }
        let trailing_dist = line_len - last_idx;
        if trailing_dist < indices[0] - start {
            start += trailing_dist;
        }
        let end = line.len();
        let left_truncated = if let Some(n) = skipped {
            let icon: String = line.chars().take(n).collect();
            format!("{}{}{}", icon, DOTS, utf8_str_slice(line, start, end))
        } else {
            format!("{}{}", DOTS, utf8_str_slice(line, start, end))
        };

        let offset = line_len.saturating_sub(left_truncated.len());

        let left_truncated_len = left_truncated.len();

        let (truncated, max_index) = if left_truncated_len > winwidth {
            if left_truncated_len == winwidth + 1 {
                (
                    format!("{}.", utf8_str_slice(&left_truncated, 0, winwidth - 1)),
                    winwidth - 1,
                )
            } else {
                (
                    format!(
                        "{}{}",
                        utf8_str_slice(&left_truncated, 0, winwidth - 2),
                        DOTS
                    ),
                    winwidth - 2,
                )
            }
        } else {
            (left_truncated, winwidth)
        };

        let truncated_indices = indices
            .iter()
            .map(|x| x - offset)
            .take_while(|x| *x < max_index)
            .collect::<Vec<_>>();

        Some((truncated, truncated_indices))
    } else {
        None
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
