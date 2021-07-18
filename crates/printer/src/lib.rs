//! This crate provides the feature of diplaying the information of filtered lines
//! by printing them to stdout in JSON format.

use std::collections::HashMap;
use std::slice::IterMut;

use icon::{IconPainter, ICON_LEN};
use source_item::{FilteredItem, SourceItem};
use utility::{println_json, println_json_with_length};

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
fn utf8_str_slice(line: &str, start: usize, end: usize) -> String {
    line.chars().take(end).skip(start).collect()
}

fn truncate_line_impl(
    winwidth: usize,
    line: &str,
    indices: &[usize],
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
            format!("{}{}{}", icon, DOTS, utf8_str_slice(&line, start, end))
        } else {
            format!("{}{}", DOTS, utf8_str_slice(&line, start, end))
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
        let line = filtered_item.display_text_before_truncated();

        if let Some((truncated, truncated_indices)) =
            truncate_line_impl(winwidth, &line, &filtered_item.match_indices, skipped)
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
                truncate_line_impl(winwidth, &line, &indices, skipped)
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

/// Returns the info of the truncated top items ranked by the filtering score.
pub fn process_top_items<T>(
    mut top_list: Vec<FilteredItem<T>>,
    winwidth: usize,
    icon_painter: Option<IconPainter>,
) -> (Vec<String>, Vec<Vec<usize>>, LinesTruncatedMap) {
    let truncated_map = truncate_long_matched_lines(top_list.iter_mut(), winwidth, None);
    if let Some(painter) = icon_painter {
        let (lines, indices): (Vec<_>, Vec<Vec<usize>>) = top_list
            .into_iter()
            .enumerate()
            .map(
                |(
                    idx,
                    FilteredItem {
                        source_item,
                        match_indices,
                        display_text,
                        ..
                    },
                )| {
                    let text = display_text.unwrap_or_else(|| source_item.display_text().into());
                    let idxs = match_indices;
                    let iconized = if let Some(origin_text) = truncated_map.get(&(idx + 1)) {
                        format!("{} {}", painter.get_icon(origin_text), text)
                    } else {
                        painter.paint(&text)
                    };
                    (iconized, idxs.iter().map(|x| x + ICON_LEN).collect())
                },
            )
            .unzip();

        (lines, indices, truncated_map)
    } else {
        let (lines, indices): (Vec<_>, Vec<_>) = top_list
            .into_iter()
            .map(
                |FilteredItem {
                     source_item,
                     match_indices,
                     ..
                 }| (source_item.raw, match_indices),
            )
            .unzip();

        (lines, indices, truncated_map)
    }
}

/// Prints the results of filter::sync_run() to stdout.
pub fn print_sync_filter_results(
    ranked: Vec<FilteredItem>,
    number: Option<usize>,
    winwidth: usize,
    icon_painter: Option<IconPainter>,
) {
    if let Some(number) = number {
        let total = ranked.len();
        let (lines, indices, truncated_map) = process_top_items(
            ranked.into_iter().take(number).collect(),
            winwidth,
            icon_painter,
        );
        if truncated_map.is_empty() {
            println_json!(total, lines, indices);
        } else {
            println_json!(total, lines, indices, truncated_map);
        }
    } else {
        for FilteredItem {
            source_item,
            match_indices,
            display_text,
            ..
        } in ranked.into_iter()
        {
            let text = display_text.unwrap_or_else(|| source_item.display_text().into());
            let indices = match_indices;
            println_json!(text, indices);
        }
    }
}

/// Prints the results of filter::dyn_run() to stdout.
pub fn print_dyn_filter_results(
    ranked: Vec<FilteredItem>,
    total: usize,
    number: usize,
    winwidth: usize,
    icon_painter: Option<IconPainter>,
) {
    let (lines, indices, truncated_map) = process_top_items(
        ranked.into_iter().take(number).collect(),
        winwidth,
        icon_painter,
    );

    if truncated_map.is_empty() {
        println_json_with_length!(total, lines, indices);
    } else {
        println_json_with_length!(total, lines, indices, truncated_map);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use filter::{
        matcher::{Algo, Bonus, MatchType, Matcher},
        Source,
    };
    use rayon::prelude::*;

    fn wrap_matches(line: &str, indices: &[usize]) -> String {
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

    fn run(params: TestParams) {
        let TestParams {
            text,
            truncated_text,
            query,
            highlighted,
            skipped,
            winwidth,
        } = params;

        let source = Source::List(std::iter::once(text.into()));

        let matcher = Matcher::new(Algo::Fzy, MatchType::Full, Bonus::FileName);
        let mut ranked = source.filter(matcher, &query).unwrap();
        ranked.par_sort_unstable_by(|v1, v2| v2.score.partial_cmp(&v1.score).unwrap());

        let (truncated_lines, _truncated_map) =
            truncate_long_matched_lines(ranked, winwidth, skipped);

        let (truncated_text_got, _score, truncated_indices) =
            truncated_lines[0].clone().deconstruct();

        assert_eq!(truncated_text, truncated_text_got);

        let highlighted_got = truncated_indices
            .iter()
            .filter_map(|i| truncated_text_got.chars().nth(*i))
            .collect::<String>();

        assert_eq!(highlighted, highlighted_got);

        println!("\n   winwidth: {}", "─".repeat(winwidth));
        println!(
            "    display: {}",
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
    fn test_printer_basics() {
        test_printer!(
            "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/file.scss",
            "..he/matched/items/will/be/invisible/file.scss",
            ("files", "files", None, 50usize)
        );

        test_printer!(
            "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/another-file.scss",
            "..ed/items/will/be/invisible/another-file.scss",
            ("files", "files", None, 50usize)
        );

        test_printer!(
            "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/file.js",
            "..then/the/matched/items/will/be/invisible/file.js",
            ("files", "files", None, 50usize)
        );

        test_printer!(
            "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/another-file.js",
            "../matched/items/will/be/invisible/another-file.js",
            ("files", "files", None, 50usize)
        );

        test_printer!(
            "/Users/xuliucheng/Library/Caches/Homebrew/universal-ctags--git/Units/afl-fuzz.r/github-issue-625-r.d/input.r",
            "..s/Homebrew/universal-ctags--git/Units/afl-fuzz..",
            ("srcggithub", "srcg", None, 50usize)
        );

        test_printer!(
            "        // Wait until propagation delay period after block we plan to mine on",
            "..pagation delay period after block we plan to mine on",
            ("bmine", "bmine", None, 58usize)
        );

        test_printer!(
          "fuzzy-filter/target/debug/deps/librustversion-b273394e6c9c64f6.dylib.dSYM/Contents/Resources/DWARF/librustversion-b273394e6c9c64f6.dylib",
          "..stversion-b273394e6c9c64f6.dylib.dSYM/Contents..",
          ("srlisresource", "srlis", None, 50usize)
        );

        test_printer!(
          "target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib",
          "..structopt_derive-3921fbf02d8d2ffe.dylib.dSYM/C..",
          ("srlisresource", "srli", None, 50usize)
        );

        test_printer!(
          "target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib",
          "..structopt_derive-3921fbf02d8d2ffe.dylib.dSYM/C..",
          ("srlisresource", "srli", None, 50usize)
        );

        test_printer!(
          "fuzzy-filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib",
          "..stversion-15764ff2535f190d.dylib.dSYM/Contents..",
          ("srlisresource", "srlis", None, 50usize)
        );
    }

    #[test]
    fn test_grep_line() {
        test_printer!(
            " bin/node/cli/src/command.rs:127:1:                          let PartialComponents { client, task_manager, ..}",
            " ..       let PartialComponents { client, task_manager, ..}",
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

    #[test]
    fn test_print_multibyte_string_slice() {
        let multibyte_str = "README.md:23:1:Gourinath Banda. “Scalable Real-Time Kernel for Small Embedded Systems”. En- glish. PhD thesis. Denmark: University of Southern Denmark, June 2003. URL: http://citeseerx.ist.psu.edu/viewdoc/download;jsessionid=84D11348847CDC13691DFAED09883FCB?doi=10.1.1.118.1909&rep=rep1&type=pdf.";
        let start = 33;
        let end = 300;
        let expected = "Scalable Real-Time Kernel for Small Embedded Systems”. En- glish. PhD thesis. Denmark: University of Southern Denmark, June 2003. URL: http://citeseerx.ist.psu.edu/viewdoc/download;jsessionid=84D11348847CDC13691DFAED09883FCB?doi=10.1.1.118.1909&rep=rep1&type=pdf.";
        assert_eq!(expected, utf8_str_slice(multibyte_str, start, end));
    }
}
