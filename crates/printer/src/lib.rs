//! This crate provides the feature of diplaying the information of filtered lines
//! by printing them to stdout in JSON format.

mod truncation;

use icon::{Icon, ICON_LEN};
use types::FilteredItem;
use utility::{println_json, println_json_with_length};

pub use self::truncation::{
    truncate_grep_lines, truncate_long_matched_lines, utf8_str_slice, LinesTruncatedMap,
};

/// 1. Truncate the line.
/// 2. Add an icon.
#[derive(Debug, Clone)]
pub struct DecoratedLines {
    /// Maybe truncated.
    pub lines: Vec<String>,
    pub indices: Vec<Vec<usize>>,
    pub truncated_map: LinesTruncatedMap,
}

impl From<(Vec<String>, Vec<Vec<usize>>, LinesTruncatedMap)> for DecoratedLines {
    fn from(
        (lines, indices, truncated_map): (Vec<String>, Vec<Vec<usize>>, LinesTruncatedMap),
    ) -> Self {
        Self {
            lines,
            indices,
            truncated_map,
        }
    }
}

impl DecoratedLines {
    pub fn print_json_with_length(&self, total: Option<usize>) {
        let Self {
            lines,
            indices,
            truncated_map,
        } = self;

        #[allow(non_upper_case_globals)]
        const method: &str = "s:process_filter_message";
        if let Some(total) = total {
            println_json_with_length!(method, lines, indices, total, truncated_map);
        } else {
            println_json_with_length!(method, lines, indices, truncated_map);
        }
    }

    pub fn print_json(&self, total: Option<usize>) {
        let Self {
            lines,
            indices,
            truncated_map,
        } = self;

        if let Some(total) = total {
            println_json!(lines, indices, total, truncated_map);
        } else {
            println_json!(lines, indices, truncated_map);
        }
    }

    pub fn print_on_session_create(&self) {
        let Self {
            lines,
            truncated_map,
            ..
        } = self;
        let method = "s:init_display";
        println_json_with_length!(lines, truncated_map, method);
    }
}

/// Returns the info of the truncated top items ranked by the filtering score.
pub fn decorate_lines<T>(
    mut top_list: Vec<FilteredItem<T>>,
    winwidth: usize,
    icon: Icon,
) -> DecoratedLines {
    let truncated_map = truncate_long_matched_lines(top_list.iter_mut(), winwidth, None);
    if let Some(painter) = icon.painter() {
        let (lines, indices): (Vec<_>, Vec<Vec<usize>>) = top_list
            .into_iter()
            .enumerate()
            .map(|(idx, filtered_item)| {
                let text = filtered_item.display_text();
                let iconized = if let Some(origin_text) = truncated_map.get(&(idx + 1)) {
                    format!("{} {}", painter.icon(origin_text), text)
                } else {
                    painter.paint(&text)
                };
                (iconized, filtered_item.shifted_indices(ICON_LEN))
            })
            .unzip();

        (lines, indices, truncated_map).into()
    } else {
        let (lines, indices): (Vec<_>, Vec<_>) = top_list
            .into_iter()
            .map(|filtered_item| {
                (
                    filtered_item.display_text().to_owned(),
                    filtered_item.match_indices,
                )
            })
            .unzip();

        (lines, indices, truncated_map).into()
    }
}

/// Prints the results of filter::sync_run() to stdout.
pub fn print_sync_filter_results(
    ranked: Vec<FilteredItem>,
    number: Option<usize>,
    winwidth: usize,
    icon: Icon,
) {
    if let Some(number) = number {
        let total = ranked.len();
        let mut ranked = ranked;
        ranked.truncate(number);
        decorate_lines(ranked, winwidth, icon).print_json(Some(total));
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
    icon: Icon,
) {
    decorate_lines(ranked.into_iter().take(number).collect(), winwidth, icon)
        .print_json_with_length(Some(total));
}

#[cfg(test)]
mod tests {
    use super::*;
    use filter::{
        matcher::{Bonus, FuzzyAlgorithm, MatchType, Matcher},
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

        let matcher = Matcher::new(FuzzyAlgorithm::Fzy, MatchType::Full, Bonus::FileName);
        let mut ranked = source
            .filter_and_collect(matcher, &query.clone().into())
            .unwrap();
        ranked.par_sort_unstable_by(|v1, v2| v2.score.partial_cmp(&v1.score).unwrap());

        let _truncated_map = truncate_long_matched_lines(ranked.iter_mut(), winwidth, skipped);

        let FilteredItem { match_indices, .. } = ranked[0].clone();

        let truncated_indices = match_indices;

        let truncated_text_got = ranked[0].display_text();

        assert_eq!(truncated_text, ranked[0].display_text());

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
