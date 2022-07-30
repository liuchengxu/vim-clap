//! This crate provides the feature of diplaying the information of filtered lines
//! by printing them to stdout in JSON format.

mod trimmer;
mod truncation;

use icon::{Icon, ICON_LEN};
use types::MatchedItem;
use utility::{println_json, println_json_with_length};

pub use self::truncation::{
    truncate_grep_lines, truncate_long_matched_lines, truncate_long_matched_lines_v0,
    LinesTruncatedMap,
};

/// 1. Truncate the line.
/// 2. Add an icon.
#[derive(Debug, Clone)]
pub struct DisplayLines {
    /// Lines to display, maybe truncated.
    pub lines: Vec<String>,
    /// Position of highlights in the lines above.
    pub indices: Vec<Vec<usize>>,
    /// A map of the line number to the original untruncated line.
    pub truncated_map: LinesTruncatedMap,
    /// An icon is added to the head of line.
    ///
    /// The icon is added after the truncating processing.
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

    pub fn print_on_session_create(&self) {
        let Self {
            lines,
            truncated_map,
            icon_added,
            ..
        } = self;
        #[allow(non_upper_case_globals)]
        const method: &str = "s:init_display";
        println_json_with_length!(method, lines, icon_added, truncated_map);
    }

    pub fn print_on_dyn_run(&self, matched: usize, processed: usize) {
        let Self {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = self;

        #[allow(non_upper_case_globals)]
        const method: &str = "s:process_filter_message";
        if truncated_map.is_empty() {
            println_json_with_length!(method, lines, indices, icon_added, matched, processed);
        } else {
            println_json_with_length!(
                method,
                lines,
                indices,
                icon_added,
                matched,
                processed,
                truncated_map
            );
        }
    }

    fn print_on_dyn_run_finished(
        &self,
        total_matched: usize,
        maybe_total_processed: Option<usize>,
    ) {
        let Self {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = self;

        #[allow(non_upper_case_globals)]
        const method: &str = "s:process_filter_message";
        if let Some(total_processed) = maybe_total_processed {
            println_json_with_length!(
                method,
                lines,
                indices,
                icon_added,
                truncated_map,
                total_matched,
                total_processed
            );
        } else {
            println_json_with_length!(
                method,
                lines,
                indices,
                icon_added,
                truncated_map,
                total_matched
            );
        }
    }

    fn print_json(&self, total: usize) {
        let Self {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = self;

        println_json!(lines, indices, truncated_map, icon_added, total);
    }
}

/// Returns the info of the truncated top items ranked by the filtering score.
pub fn decorate_lines(
    matched_items: Vec<MatchedItem>,
    winwidth: usize,
    icon: Icon,
) -> DisplayLines {
    let mut matched_items = matched_items;
    let mut truncated_map = truncate_long_matched_lines(matched_items.iter_mut(), winwidth, None);
    if let Some(painter) = icon.painter() {
        let (lines, indices): (Vec<_>, Vec<Vec<usize>>) = matched_items
            .into_iter()
            .enumerate()
            .map(|(idx, matched_item)| {
                let display_text = matched_item.display_text();
                let iconized = if let Some(output_text) = truncated_map.get_mut(&(idx + 1)) {
                    let icon = matched_item
                        .item
                        .icon()
                        .unwrap_or_else(|| painter.icon(output_text));
                    *output_text = format!("{icon} {output_text}");
                    format!("{icon} {display_text}")
                } else {
                    painter.paint(&display_text)
                };
                (iconized, matched_item.shifted_indices(ICON_LEN))
            })
            .unzip();

        DisplayLines::new(lines, indices, truncated_map, true)
    } else {
        let (lines, indices): (Vec<_>, Vec<_>) = matched_items
            .into_iter()
            .map(|matched_item| {
                (
                    matched_item.display_text().to_string(),
                    matched_item.indices,
                )
            })
            .unzip();

        DisplayLines::new(lines, indices, truncated_map, false)
    }
}

/// Prints the results of filter::sync_run() to stdout.
pub fn print_sync_filter_results(
    matched_items: Vec<MatchedItem>,
    number: Option<usize>,
    winwidth: usize,
    icon: Icon,
) {
    if let Some(number) = number {
        let total_matched = matched_items.len();
        let mut matched_items = matched_items;
        matched_items.truncate(number);
        decorate_lines(matched_items, winwidth, icon).print_json(total_matched);
    } else {
        matched_items.iter().for_each(|matched_item| {
            let indices = &matched_item.indices;
            let text = matched_item.display_text();
            println_json!(text, indices);
        });
    }
}

/// Prints the results of filter::dyn_run() to stdout.
pub fn print_dyn_matched_items(
    matched_items: Vec<MatchedItem>,
    total_matched: usize,
    total_processed: Option<usize>,
    winwidth: usize,
    icon: Icon,
) {
    decorate_lines(matched_items, winwidth, icon)
        .print_on_dyn_run_finished(total_matched, total_processed);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use filter::{
        matcher::{Bonus, FuzzyAlgorithm, MatchScope, Matcher},
        Source, SourceItem,
    };
    use rayon::prelude::*;
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
        let matcher = Matcher::new(Bonus::FileName, FuzzyAlgorithm::Fzy, MatchScope::Full);

        let mut ranked = Source::List(std::iter::once(Arc::new(line.into()) as Arc<dyn ClapItem>))
            .run_and_collect(matcher, &query.into())
            .unwrap();
        ranked.par_sort_unstable_by(|v1, v2| v2.score.partial_cmp(&v1.score).unwrap());

        ranked
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
        let _truncated_map = truncate_long_matched_lines(ranked.iter_mut(), winwidth, skipped);

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
            " ..           let PartialComponents { client, task_manager, ..}",
            ("PartialComponents", "PartialComponents", Some(2), 64)
        );
    }

    #[test]
    fn starting_point_should_work() {
        const QUERY: &str = "srlisrlisrsr";

        test_printer!(
            " crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib",
            " ..s/fuzzy_filter/target/debug/deps/librustvers..",
            (QUERY, "srlisr", Some(2), 50)
        );

        test_printer!(
            " crates/fuzzy_filter/target/debug/deps/libstructopt_derive-5cce984f248086cc.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-5cce984f248086cc.dylib",
            " ..s/fuzzy_filter/target/debug/deps/libstructop..",
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
}
