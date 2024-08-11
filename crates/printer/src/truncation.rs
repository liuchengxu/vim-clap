use crate::trimmer::v1::{trim_text as trim_text_v1, TrimmedText};
use crate::trimmer::UnicodeDots;
use crate::GrepResult;
use std::collections::HashMap;
use std::path::MAIN_SEPARATOR;
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

fn truncate_line_v1(
    line: &str,
    indices: &mut [usize],
    winwidth: usize,
    skipped_chars: Option<usize>,
) -> Option<TrimmedText> {
    if line.is_empty() || indices.is_empty() {
        return None;
    }

    if let Some(skipped) = skipped_chars {
        let byte_idx = line
            .char_indices()
            .nth(skipped)
            .map(|(byte_idx, _char)| byte_idx)?;

        // winwidth is char-sized.
        let container_width = winwidth - skipped;

        // indices from matcher are char-sized.
        indices.iter_mut().for_each(|x| *x -= skipped);

        let (text_skipped, text_to_trim) = line.split_at(byte_idx);

        // TODO: tabstop is not always 4, `:h vim9-differences`
        let maybe_trimmed =
            trim_text_v1(text_to_trim, indices, container_width, 4).map(|mut trimmed| {
                // Rejoin the skipped chars.
                let mut new_text = String::with_capacity(byte_idx + trimmed.trimmed_text.len());
                new_text.push_str(text_skipped);
                new_text.push_str(&trimmed.trimmed_text);
                trimmed.trimmed_text = new_text;

                trimmed.indices.iter_mut().for_each(|x| *x += skipped);

                trimmed
            });

        indices.iter_mut().for_each(|x| *x += skipped);

        maybe_trimmed
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
pub fn truncate_item_output_text(
    items: IterMut<MatchedItem>,
    winwidth: usize,
    skipped_chars: Option<usize>,
) -> LinesTruncatedMap {
    let mut truncated_map = HashMap::new();
    items.enumerate().for_each(|(lnum, matched_item)| {
        let output_text = matched_item.output_text().to_string();
        let truncation_offset = skipped_chars.or(matched_item.item.truncation_offset());

        // Truncate the text simply if it's too long.
        if output_text.len() > MAX_LINE_LEN {
            let truncated_output_text: String = output_text.chars().take(1000).collect();
            matched_item.display_text = Some(truncated_output_text);
            matched_item.indices.retain(|&x| x < 1000);
        } else if let Some(TrimmedText {
            trimmed_text,
            indices,
            ..
        }) = truncate_line_v1(
            &output_text,
            &mut matched_item.indices,
            winwidth,
            truncation_offset,
        ) {
            truncated_map.insert(lnum + 1, output_text);

            matched_item.display_text.replace(trimmed_text);
            matched_item.indices = indices;
        } else {
            // Use the origin `output_text` as the final `display_text`.
            matched_item.display_text.replace(output_text);
        }
    });
    truncated_map
}

/// Truncate the output text of item if it's too long.
///
/// # Arguments
///
/// - `winwidth`: width of the display window.
/// - `skipped`: number of skipped chars, used when need to skip the leading icons.
pub fn truncate_grep_results(
    grep_results: IterMut<GrepResult>,
    winwidth: usize,
    skipped_chars: Option<usize>,
) -> LinesTruncatedMap {
    let mut truncated_map = HashMap::new();
    grep_results.enumerate().for_each(|(lnum, grep_result)| {
        let output_text = grep_result.matched_item.output_text().to_string();

        // Truncate the text simply if it's too long.
        if output_text.len() > MAX_LINE_LEN {
            let truncated_output_text: String = output_text.chars().take(1000).collect();
            grep_result.matched_item.display_text = Some(truncated_output_text);
            grep_result.matched_item.indices.retain(|&x| x < 1000);
        } else if let Some(trimmed) = truncate_line_v1(
            &output_text,
            &mut grep_result.matched_item.indices,
            winwidth,
            skipped_chars,
        ) {
            let TrimmedText {
                trimmed_text,
                indices,
                trim_info,
            } = trimmed;

            truncated_map.insert(lnum + 1, output_text);

            // Adjust the trimmed text further.
            let (better_trimmed_text, indices) = match trim_info.left_trim_start() {
                Some(start) => {
                    match grep_result.path.to_str().and_then(pattern::extract_file_name) {
                        Some((file_name, file_name_start)) if start > file_name_start => {
                            let line_number = grep_result.line_number;
                            let column = grep_result.column;
                            let column_end = grep_result.column_end;

                            // dots + MAIN_SEPARATOR
                            let mut offset = UnicodeDots::CHAR_LEN
                                + 1
                                + file_name.len()
                                + utils::display_width(line_number)
                                + utils::display_width(column)
                                + 2; // : + :

                            // In the middle of file name and column
                            let trimmed_text_with_visible_filename = if start < column_end {
                                let mut trimmed_text_chars = trimmed_text.chars();
                                (0..column_end - start).for_each(|_| { trimmed_text_chars.next(); });

                                let trimmed_pattern = trimmed_text_chars.as_str();
                                offset -= column_end - start;

                                format!("{}{MAIN_SEPARATOR}{file_name}:{line_number}:{column}{trimmed_pattern}", UnicodeDots::DOTS)
                            } else {
                                format!("{}{MAIN_SEPARATOR}{file_name}:{line_number}:{column}{trimmed_text}", UnicodeDots::DOTS)
                            };

                            let mut indices = indices;
                            // let file_name_end = UnicodeDots::CHAR_LEN + 1 + file_name.len();
                            indices.iter_mut().for_each(|x| {
                                *x += offset;
                                // if *x <= file_name_end {
                                    // *x -= 1;
                                // }
                            });
                            // TODO: truncate the invisible text and indices caused by the inserted
                            // file name.

                            (trimmed_text_with_visible_filename, indices)
                        }
                        _ => (trimmed_text, indices),
                    }
                }
                _ => (trimmed_text, indices),
            };

            grep_result.matched_item.display_text.replace(better_trimmed_text);
            grep_result.matched_item.indices = indices;
        } else {
            // Use the origin `output_text` as the final `display_text`.
            grep_result.matched_item.display_text.replace(output_text);
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
    skipped_chars: Option<usize>,
) -> (Vec<String>, Vec<Vec<usize>>, LinesTruncatedMap) {
    let mut truncated_map = HashMap::new();
    let mut lnum = 0usize;
    let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = lines
        .into_iter()
        .zip(indices)
        .map(|(line, mut indices)| {
            lnum += 1;

            if let Some(trimmed) = truncate_line_v1(&line, &mut indices, winwidth, skipped_chars) {
                truncated_map.insert(lnum, line);
                (trimmed.trimmed_text, trimmed.indices)
            } else {
                (line, indices)
            }
        })
        .unzip();
    (lines, indices, truncated_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrepResult;
    use std::sync::Arc;
    use types::ClapItem;

    #[test]
    fn test_grep_print() {
        // GrepResult { matched_item: MatchedItem { item: "crates/maple_core/src/paths.rs:198:31:let expected = \"~/.rustup/.../src/rust/library/alloc/src/string.rs\";", rank: [874, -30, -68, 0], indices: [68, 69, 77, 91, 92], display_text: None, output_text: None }, path: "/home/xlc/.vim/plugged/vim-clap/crates/maple_core/src/paths.rs", line_number: 198, line_number_start: 32, line_number_end: 35, column: 31, column_start: 36, column_end: 38 }, winwidth: 62, icon: Enabled(Grep)
        let line = r#"crates/maple_core/src/paths.rs:198:31:let expected = "~/.rustup/.../src/rust/library/alloc/src/string.rs";"#;
        let mut items = [GrepResult {
            matched_item: MatchedItem::new(
                Arc::new(line.to_string()) as Arc<dyn ClapItem>,
                [874, -30, -68, 0],
                vec![68, 69, 77, 91, 92],
            ),
            path: "/home/xlc/.vim/plugged/vim-clap/crates/maple_core/src/paths.rs".into(),
            line_number: 198,
            column: 31,
            column_end: 38,
        }];
        let winwidth = 62;

        truncate_grep_results(items.iter_mut(), winwidth, None);
    }
}
