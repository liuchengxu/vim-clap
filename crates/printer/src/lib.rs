//! This crate provides the feature of diplaying the information of filtered lines
//! by printing them to stdout in JSON format.

mod trimmer;
mod truncation;

use self::truncation::LinesTruncatedMap;
use icon::{Icon, ICON_CHAR_LEN};
use serde::Serialize;
use std::path::PathBuf;
use truncation::truncate_grep_results;
use types::MatchedItem;

pub use self::trimmer::v1::{trim_text, TrimInfo, TrimmedText};
pub use self::truncation::{
    truncate_grep_lines, truncate_item_output_text, truncate_item_output_text_v0,
};

/// Combine json and println macro.
#[macro_export]
macro_rules! println_json {
  ( $( $field:expr ),+ ) => {
    {
      println!("{}", serde_json::json!({ $(stringify!($field): $field,)* }))
    }
  }
}

/// Combine json and println macro.
///
/// Neovim needs Content-length info when using stdio-based communication.
#[macro_export]
macro_rules! println_json_with_length {
  ( $( $field:expr ),+ ) => {
    {
      let msg = serde_json::json!({ $(stringify!($field): $field,)* });
      if let Ok(s) = serde_json::to_string(&msg) {
          println!("Content-length: {}\n\n{}", s.len(), s);
      }
    }
  }
}

/// This structure holds the data that can be easily used to update the UI on the Vim side.
///
/// Potential processing to the display text:
///
/// 1. Truncate the line if the window can't fit it.
/// 2. Add an icon to the beginning.
#[derive(Debug, Clone, Serialize)]
pub struct DisplayLines {
    /// Lines to display, maybe truncated.
    pub lines: Vec<String>,
    /// Byte position of highlights in the lines above.
    pub indices: Vec<Vec<usize>>,
    /// A map of the line number to the original untruncated line.
    pub truncated_map: LinesTruncatedMap,
    /// Whether an icon is added to the head of line.
    ///
    /// The icon is added after the truncation.
    pub icon_added: bool,
}

impl DisplayLines {
    pub fn new(
        lines: Vec<String>,
        indices: Vec<Vec<usize>>,
        truncated_map: LinesTruncatedMap,
        icon_added: bool,
    ) -> Self {
        Self {
            lines,
            indices,
            truncated_map,
            icon_added,
        }
    }

    pub fn print_json(&self, total: usize) {
        let Self {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = self;

        println_json!(lines, indices, truncated_map, icon_added, total);
    }
}

/// Converts the char positions to byte positions as Vim and Neovim highlights is byte-positioned.
fn char_indices_to_byte_indices(s: &str, char_indices: &[usize]) -> Vec<usize> {
    s.char_indices()
        .enumerate()
        .filter_map(|(char_idx, (byte_idx, _char))| {
            if char_indices.contains(&char_idx) {
                Some(byte_idx)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

fn convert_truncated_matched_items_to_display_lines(
    matched_items: impl IntoIterator<Item = MatchedItem>,
    icon: Icon,
    mut truncated_map: LinesTruncatedMap,
) -> DisplayLines {
    if let Some(icon_kind) = icon.icon_kind() {
        let (lines, indices): (Vec<_>, Vec<Vec<usize>>) = matched_items
            .into_iter()
            .enumerate()
            .map(|(idx, matched_item)| {
                let display_text = matched_item.display_text();
                let iconized = if let Some(output_text) = truncated_map.get_mut(&(idx + 1)) {
                    let icon = matched_item
                        .item
                        .icon(icon)
                        .expect("Icon must be provided if specified");
                    *output_text = format!("{icon} {output_text}");
                    format!("{icon} {display_text}")
                } else {
                    icon_kind.add_icon_to_text(&display_text)
                };
                let (line, indices) = (iconized, matched_item.shifted_indices(ICON_CHAR_LEN));
                let indices = char_indices_to_byte_indices(&line, &indices);
                (line, indices)
            })
            .unzip();

        DisplayLines::new(lines, indices, truncated_map, true)
    } else {
        let (lines, indices): (Vec<_>, Vec<_>) = matched_items
            .into_iter()
            .map(|matched_item| {
                let (line, indices) = (
                    matched_item.display_text().to_string(),
                    matched_item.indices,
                );
                let indices = char_indices_to_byte_indices(&line, &indices);
                (line, indices)
            })
            .unzip();

        DisplayLines::new(lines, indices, truncated_map, false)
    }
}

/// Returns the info of the truncated top items ranked by the filtering score.
pub fn to_display_lines(
    mut matched_items: Vec<MatchedItem>,
    winwidth: usize,
    icon: Icon,
) -> DisplayLines {
    let truncated_map = truncate_item_output_text(matched_items.iter_mut(), winwidth, None);
    convert_truncated_matched_items_to_display_lines(matched_items, icon, truncated_map)
}

#[derive(Debug)]
pub struct GrepResult {
    pub matched_item: MatchedItem,
    /// File path in the final grep line, might be relative path.
    pub path: PathBuf,
    pub line_number: usize,
    pub column: usize,
    pub column_end: usize,
}

pub fn grep_results_to_display_lines(
    mut grep_results: Vec<GrepResult>,
    winwidth: usize,
    icon: Icon,
) -> DisplayLines {
    let truncated_map = truncate_grep_results(grep_results.iter_mut(), winwidth, None);
    convert_truncated_matched_items_to_display_lines(
        grep_results.into_iter().map(|i| i.matched_item),
        icon,
        truncated_map,
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use filter::{
        filter_sequential,
        matcher::{Bonus, MatcherBuilder},
        SequentialSource, SourceItem,
    };
    use std::sync::Arc;
    use types::{ClapItem, Query};

    pub(crate) fn wrap_matches(line: &str, indices: &[usize]) -> String {
        let mut ret = String::new();
        let mut peekable = indices.iter().peekable();
        for (idx, ch) in line.chars().enumerate() {
            let next_id = **peekable.peek().unwrap_or(&&line.len());
            if next_id == idx {
                #[cfg(not(target_os = "windows"))]
                {
                    ret.push_str(
                        format!("{}{}{}", termion::style::Invert, ch, termion::style::Reset)
                            .as_str(),
                    );
                }

                #[cfg(target_os = "windows")]
                {
                    ret.push_str(format!("~{}~", ch).as_str());
                }

                peekable.next();
            } else {
                ret.push(ch);
            }
        }

        ret
    }

    struct TestParams {
        text: String,
        truncated_text: String,
        query: String,
        highlighted: String,
        skipped: Option<usize>,
        winwidth: usize,
    }

    pub(crate) fn filter_single_line(
        line: impl Into<SourceItem>,
        query: impl Into<Query>,
    ) -> Vec<MatchedItem> {
        let matcher = MatcherBuilder::new()
            .bonuses(vec![Bonus::FileName])
            .build(query.into());

        filter_sequential(
            SequentialSource::List(std::iter::once(Arc::new(line.into()) as Arc<dyn ClapItem>)),
            matcher,
        )
        .unwrap()
    }

    fn run(params: TestParams) {
        let TestParams {
            text,
            truncated_text,
            query,
            highlighted,
            skipped,
            winwidth,
        } = params;

        let mut ranked = filter_single_line(text, &query);
        let _truncated_map = truncate_item_output_text(ranked.iter_mut(), winwidth, skipped);

        let MatchedItem { indices, .. } = ranked[0].clone();
        let truncated_indices = indices;

        let truncated_text_got = ranked[0].display_text();
        assert_eq!(truncated_text, truncated_text_got);

        let highlighted_got = truncated_indices
            .iter()
            .filter_map(|i| truncated_text_got.chars().nth(*i))
            .collect::<String>();
        assert_eq!(highlighted, highlighted_got);

        println!("\n      winwidth: {}", "─".repeat(winwidth));
        println!(
            "       display: {}",
            wrap_matches(&truncated_text_got, &truncated_indices)
        );
        // The highlighted result can be case insensitive.
        assert!(query
            .to_lowercase()
            .starts_with(&highlighted.to_lowercase()));
    }

    macro_rules! test_printer {
        (
          $text:expr,
          $truncated_text:expr,
          ($query:expr, $highlighted:expr, $skipped:expr, $winwidth:expr)
        ) => {
            let params = TestParams {
                text: $text.into(),
                truncated_text: $truncated_text.into(),
                query: $query.into(),
                highlighted: $highlighted.into(),
                skipped: $skipped,
                winwidth: $winwidth,
            };
            run(params);
        };
    }

    #[test]
    fn test_grep_line() {
        test_printer!(
            " bin/node/cli/src/command.rs:127:1:                          let PartialComponents { client, task_manager, ..}",
            " ..         let PartialComponents { client, task_manager, ..}",
            ("PartialComponents", "PartialComponents", Some(2), 64)
        );
    }

    #[test]
    fn starting_point_should_work() {
        const QUERY: &str = "srlisrlisrsr";
        // TODO: revisit the tests, may not be accurate.

        test_printer!(
            " crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib",
            " ..tes/fuzzy_filter/target/debug/deps/librust..",
            (QUERY, "srlis", Some(2), 50)
        );

        test_printer!(
            " crates/fuzzy_filter/target/debug/deps/libstructopt_derive-5cce984f248086cc.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-5cce984f248086cc.dylib",
            " ..tes/fuzzy_filter/target/debug/deps/libstru..",
            (QUERY, "srlis", Some(2), 50)
        );

        test_printer!(
            "crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib",
            "..s/fuzzy_filter/target/debug/deps/librustversio..",
            (QUERY, "srlisr", None, 50)
        );

        test_printer!(
          "crates/fuzzy_filter/target/debug/deps/libstructopt_derive-5cce984f248086cc.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-5cce984f248086cc.dylib",
          "..s/fuzzy_filter/target/debug/deps/libstructopt_..",
            (QUERY, "srlis", None, 50)
        );
    }

    #[test]
    fn test_char_position_to_byte_position() {
        let line = "1 # 存储项目";
        let char_pos = vec![4, 5];
        let expected_byte_pos = vec![4, 7];

        assert_eq!(
            expected_byte_pos,
            char_indices_to_byte_indices(line, &char_pos)
        );

        let line = "abcdefg";
        let char_pos = vec![4, 5];
        let expected_byte_pos = vec![4, 5];

        assert_eq!(
            expected_byte_pos,
            char_indices_to_byte_indices(line, &char_pos)
        )
    }
}
