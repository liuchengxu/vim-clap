//! This crate provides the feature of diplaying the information of filtered lines
//! by printing them to stdout in JSON format.

use icon::{IconPainter, ICON_LEN};
use std::collections::HashMap;
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

/// Tuple of (matched line text, filtering score, indices of matched elements)
pub type FilterResult = (String, i64, Vec<usize>);

// https://stackoverflow.com/questions/51982999/slice-a-string-containing-unicode-chars
#[inline]
fn utf8_str_slice(line: &str, start: usize, end: usize) -> String {
    line.chars().take(end).skip(start).collect()
}

/// Long matched lines can cause the matched items invisible.
///
/// # Arguments
///
/// - winwidth: width of the display window.
/// - skipped: number of skipped chars, used when need to skip the leading icons.
///
/// [--------------------------]
///                                              end
/// [-------------------------------------xx--x---]
///                     start    winwidth
/// |~~~~~~~~~~~~~~~~~~[------------------xx--x---]
///
///  `start >= indices[0]`
/// |----------[x-------------------------xx--x---]
///
/// |~~~~~~~~~~~~~~~~~~[----------xx--x------------------------------x-----]
///  `last_idx - start >= winwidth`
/// |~~~~~~~~~~~~~~~~~~~~~~~~~~~~[xx--x------------------------------x-----]
///
pub fn truncate_long_matched_lines<T>(
    lines: impl IntoIterator<Item = (String, T, Vec<usize>)>,
    winwidth: usize,
    skipped: Option<usize>,
) -> (Vec<(String, T, Vec<usize>)>, LinesTruncatedMap) {
    let mut truncated_map = HashMap::new();
    let mut lnum = 0usize;
    let lines = lines
        .into_iter()
        .map(|(line, score, indices)| {
            lnum += 1;
            if !indices.is_empty() {
                let last_idx = indices.last().expect("indices are non-empty; qed");
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
                    let truncated = if let Some(n) = skipped {
                        let icon: String = line.chars().take(n).collect();
                        start += n;
                        format!("{}{}{}", icon, DOTS, utf8_str_slice(&line, start, end))
                    } else {
                        format!("{}{}", DOTS, utf8_str_slice(&line, start, end))
                    };
                    let offset = line_len - truncated.len();
                    let truncated_indices = indices.iter().map(|x| x - offset).collect::<Vec<_>>();
                    truncated_map.insert(lnum, line);
                    (truncated, score, truncated_indices)
                } else {
                    (line, score, indices)
                }
            } else {
                (line, score, indices)
            }
        })
        .collect::<Vec<_>>();
    (lines, truncated_map)
}

/// Returns the info of the truncated top items ranked by the filtering score.
pub fn process_top_items<T>(
    top_size: usize,
    top_list: impl IntoIterator<Item = (String, T, Vec<usize>)>,
    winwidth: Option<usize>,
    icon_painter: Option<IconPainter>,
) -> (Vec<String>, Vec<Vec<usize>>, LinesTruncatedMap) {
    let (truncated_lines, truncated_map) =
        truncate_long_matched_lines(top_list, winwidth.unwrap_or(62), None);
    let mut lines = Vec::with_capacity(top_size);
    let mut indices = Vec::with_capacity(top_size);
    if let Some(painter) = icon_painter {
        for (idx, (text, _, idxs)) in truncated_lines.iter().enumerate() {
            let iconized = if let Some(origin_text) = truncated_map.get(&(idx + 1)) {
                format!("{} {}", painter.get_icon(origin_text), text)
            } else {
                painter.paint(&text)
            };
            lines.push(iconized);
            indices.push(idxs.iter().map(|x| x + ICON_LEN).collect());
        }
    } else {
        for (text, _, idxs) in truncated_lines {
            lines.push(text);
            indices.push(idxs);
        }
    }
    (lines, indices, truncated_map)
}

/// Prints the results of filter::sync_run() to stdout.
pub fn print_sync_filter_results(
    ranked: Vec<FilterResult>,
    number: Option<usize>,
    winwidth: Option<usize>,
    icon_painter: Option<IconPainter>,
) {
    if let Some(number) = number {
        let total = ranked.len();
        let (lines, indices, truncated_map) = process_top_items(
            number,
            ranked.into_iter().take(number),
            winwidth,
            icon_painter,
        );
        if truncated_map.is_empty() {
            println_json!(total, lines, indices);
        } else {
            println_json!(total, lines, indices, truncated_map);
        }
    } else {
        for (text, _, indices) in ranked.iter() {
            println_json!(text, indices);
        }
    }
}

/// Prints the results of filter::dyn_run() to stdout.
pub fn print_dyn_filter_results(
    ranked: Vec<FilterResult>,
    total: usize,
    number: usize,
    winwidth: Option<usize>,
    icon_painter: Option<IconPainter>,
) {
    let (lines, indices, truncated_map) = process_top_items(
        number,
        ranked.into_iter().take(number),
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
    use filter::{matcher::Algo, Source};
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

    fn run_test<I: Iterator<Item = String>>(
        source: Source<I>,
        query: &str,
        skipped: Option<usize>,
        winwidth: usize,
    ) {
        let mut ranked = source.filter(Algo::Fzy, query).unwrap();
        ranked.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

        println!();
        println!("query: {:?}", query);

        let (truncated_lines, truncated_map) =
            truncate_long_matched_lines(ranked, winwidth, skipped);
        for (idx, (truncated_line, _score, truncated_indices)) in truncated_lines.iter().enumerate()
        {
            println!("truncated: {}", "-".repeat(winwidth));
            println!(
                "truncated: {}",
                wrap_matches(&truncated_line, &truncated_indices)
            );
            println!("raw_line: {}", truncated_map.get(&(idx + 1)).unwrap());
        }
    }

    #[test]
    fn case1() {
        let source: Source<_> = vec![
        "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/file.scss".into(),
        "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/another-file.scss"
            .into(),
        "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/file.js".into(),
        "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/another-file.js"
            .into(),
    ]
        .into();
        let query = "files";
        run_test(source, query, None, 50usize);
    }

    #[test]
    fn case2() {
        let source: Source<_> = vec![
        "fuzzy-filter/target/debug/deps/librustversion-b273394e6c9c64f6.dylib.dSYM/Contents/Resources/DWARF/librustversion-b273394e6c9c64f6.dylib".into(),
        "fuzzy-filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib".into(),
        "target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib".into(),
        "target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib".into(),
        ].into();
        let query = "srlisresource";
        run_test(source, query, None, 50usize);
    }

    #[test]
    fn case3() {
        let source: Source<_> = vec![
        "/Users/xuliucheng/Library/Caches/Homebrew/universal-ctags--git/Units/afl-fuzz.r/github-issue-625-r.d/input.r".into()
        ].into();
        let query = "srcggithub";
        run_test(source, query, None, 50usize);
    }

    #[test]
    fn case4() {
        let source: Source<_> = vec![
            "        // Wait until propagation delay period after block we plan to mine on".into(),
        ]
        .into();
        let query = "bmine";
        run_test(source, query, None, 58usize);
    }

    #[test]
    fn starting_point_should_work() {
        let source: Source<_> = vec![
          " crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib".into(),
          " crates/fuzzy_filter/target/debug/deps/libstructopt_derive-5cce984f248086cc.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-5cce984f248086cc.dylib".into()
        ].into();
        let query = "srlisrlisrsr";
        run_test(source, query, Some(2), 50usize);

        let source: Source<_> = vec![
          "crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib".into(),
          "crates/fuzzy_filter/target/debug/deps/libstructopt_derive-5cce984f248086cc.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-5cce984f248086cc.dylib".into()
        ].into();
        let query = "srlisrlisrsr";
        run_test(source, query, None, 50usize);
    }

    #[test]
    fn test_print_multibyte_string_slice() {
        let multibyte_str = "README.md:23:1:Gourinath Banda. “Scalable Real-Time Kernel for Small Embedded Systems”. En- glish. PhD thesis. Denmark: University of Southern Denmark, June 2003. URL: http://citeseerx.ist.psu.edu/viewdoc/download;jsessionid=84D11348847CDC13691DFAED09883FCB?doi=10.1.1.118.1909&rep=rep1&type=pdf.";
        let start = 33;
        let end = 300;
        // println!("{}", &multibyte_str[33..300]);
        println!("{}", utf8_str_slice(multibyte_str, start, end));
    }
}
