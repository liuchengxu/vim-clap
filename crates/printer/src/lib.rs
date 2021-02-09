//! This crate provides the feature of diplaying the information of filtered lines
//! by printing them to stdout in JSON format.

use std::collections::HashMap;

use icon::{IconPainter, ICON_LEN};
use source_item::SourceItem;
use utility::{println_json, println_json_with_length};

const DOTS: &str = "..";
const DOTS_LEN: usize = DOTS.len();

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
pub type FilterResult = (SourceItem, i64, Vec<usize>);

// https://stackoverflow.com/questions/51982999/slice-a-string-containing-unicode-chars
#[inline]
#[allow(unused)]
fn utf8_str_slice(line: &str, start: usize, end: usize) -> String {
    line.chars().take(end).skip(start).collect()
}

#[derive(Debug, Clone)]
pub struct Printer {
    winwidth: usize,
    skipped: usize,
}

/// sign column width 2 and ?
/// TBH, I don't know why the offset is 4.
#[cfg(not(test))]
const WIDTH_OFFSET: usize = 4;

#[cfg(test)]
const WIDTH_OFFSET: usize = 0;

impl Printer {
    pub fn new(winwidth: usize, skipped: usize) -> Self {
        Self { winwidth, skipped }
    }

    pub fn display(&self, origin_line: &str, indices: &[usize]) -> Option<(String, Vec<usize>)> {
        if self.skipped > 0 {
            let skipped_line = origin_line.chars().skip(self.skipped).collect::<String>();
            let indices = indices.iter().map(|x| x + self.skipped).collect::<Vec<_>>();
            let winwidth = self.winwidth - self.skipped;

            let display_result = Self::_display(&skipped_line, &indices, winwidth);

            let skipped_part = origin_line.chars().take(self.skipped).collect::<String>();

            display_result
                .map(|(truncated, indices)| (format!("{}{}", skipped_part, truncated), indices))
        } else {
            Self::_display(origin_line, indices, self.winwidth)
        }
    }

    /// Truncates the `origin_line` to match the given window width `winwidth`.
    fn _display(
        origin_line: &str,
        indices: &[usize],
        winwidth: usize,
    ) -> Option<(String, Vec<usize>)> {
        // This should never happen in practice.
        if indices.is_empty() {
            return None;
        }

        let indices_len = indices.len();
        if indices_len == 1 {
            // TODO: ensure the single item visible.
            Some((origin_line.into(), indices.to_owned()))
        } else {
            let first_idx = indices[0];
            let last_idx = indices.last().expect("indices is not empty; qed");
            let span = last_idx - first_idx;

            // All matched items are visible.
            if span <= winwidth {
                let n = if origin_line.len() - first_idx < winwidth {
                    first_idx.saturating_sub(winwidth - (origin_line.len() - first_idx))
                } else {
                    first_idx
                };
                let truncated: String = origin_line.chars().skip(n).take(winwidth).collect();
                Some((truncated, indices.iter().map(|x| x - n).collect()))
            } else {
                // the matched items are partially visible.
                // prepend `..` at the beginning and display the line from the first item.
                let n = first_idx - DOTS_LEN;

                let initial: String = origin_line
                    .chars()
                    .skip(n)
                    .take(winwidth - DOTS_LEN * 2) //TODO: precise end
                    .collect();

                Some((
                    format!("{}{}{}", DOTS, initial, DOTS),
                    indices.iter().map(|x| x - n + DOTS_LEN).collect(),
                ))
            }
        }
    }
}

/// Long matched lines can cause the matched items invisible.
///
/// # Arguments
///
/// - winwidth: width of the display window.
/// - skipped: number of skipped chars, used when need to skip the leading icons.
pub fn truncate_long_matched_lines<T>(
    lines: impl IntoIterator<Item = (SourceItem, T, Vec<usize>)>,
    winwidth: usize,
    skipped: Option<usize>,
) -> (Vec<(String, T, Vec<usize>)>, LinesTruncatedMap) {
    let skipped = skipped.unwrap_or(0);
    let printer = Printer {
        winwidth: winwidth - WIDTH_OFFSET,
        skipped,
    };

    let mut truncated_map = HashMap::new();
    let mut lnum = 0usize;

    let lines = lines
        .into_iter()
        .map(|(item, score, indices)| {
            let origin_line = item.display_text.unwrap_or(item.raw);
            lnum += 1;

            if let Some((truncated_line, adjusted_indices)) =
                printer.display(&origin_line, &indices)
            {
                truncated_map.insert(lnum, origin_line);
                (truncated_line, score, adjusted_indices)
            } else {
                (origin_line, score, indices)
            }
        })
        .collect::<Vec<_>>();

    (lines, truncated_map)
}

/// Returns the info of the truncated top items ranked by the filtering score.
pub fn process_top_items<T>(
    top_list: impl IntoIterator<Item = (SourceItem, T, Vec<usize>)>,
    winwidth: Option<usize>,
    icon_painter: Option<IconPainter>,
) -> (Vec<String>, Vec<Vec<usize>>, LinesTruncatedMap) {
    let (truncated_lines, truncated_map) =
        truncate_long_matched_lines(top_list, winwidth.unwrap_or(62), None);
    if let Some(painter) = icon_painter {
        let (lines, indices): (Vec<_>, Vec<Vec<usize>>) = truncated_lines
            .into_iter()
            .enumerate()
            .map(|(idx, (text, _, idxs))| {
                let iconized = if let Some(origin_text) = truncated_map.get(&(idx + 1)) {
                    format!("{} {}", painter.get_icon(origin_text), text)
                } else {
                    painter.paint(&text)
                };
                (iconized, idxs.iter().map(|x| x + ICON_LEN).collect())
            })
            .unzip();

        (lines, indices, truncated_map)
    } else {
        let (lines, indices): (Vec<_>, Vec<_>) = truncated_lines
            .into_iter()
            .map(|(text, _, idxs)| (text, idxs))
            .unzip();

        (lines, indices, truncated_map)
    }
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
        let (lines, indices, truncated_map) =
            process_top_items(ranked.into_iter().take(number), winwidth, icon_painter);
        if truncated_map.is_empty() {
            println_json!(total, lines, indices);
        } else {
            println_json!(total, lines, indices, truncated_map);
        }
    } else {
        for (item, _, indices) in ranked.into_iter() {
            let text = item.display_text.unwrap_or(item.raw);
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
    let (lines, indices, truncated_map) =
        process_top_items(ranked.into_iter().take(number), winwidth, icon_painter);

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

    fn run_test<I: Iterator<Item = SourceItem>>(
        source: Source<I>,
        query: &str,
        skipped: Option<usize>,
        winwidth: usize,
    ) {
        let ranked = filter::sync_run(
            query,
            source,
            Algo::Fzy,
            MatchType::Full,
            vec![Bonus::FileName],
        )
        .unwrap();

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
            println!(" raw_line: {}", truncated_map.get(&(idx + 1)).unwrap());
        }
    }

    fn into_source(lines: Vec<&str>) -> Source<std::vec::IntoIter<SourceItem>> {
        Source::List(
            lines
                .into_iter()
                .map(|s| s.to_string().into())
                .collect::<Vec<SourceItem>>()
                .into_iter(),
        )
    }

    #[test]
    fn test_printer() {
        fn ensure_printer_works(
            winwidth: usize,
            skipped: usize,
            query: &str,
            origin_line: &str,
            expected_display_line: &str,
        ) {
            let matcher = Matcher::new(Algo::Fzy, MatchType::Full, Bonus::FileName);

            let printer = Printer::new(winwidth, skipped);
            let (_, indices) = matcher.base_match(&origin_line.into(), query).unwrap();
            let (display_line, _adjusted_indices) = printer.display(origin_line, &indices).unwrap();
            assert_eq!(display_line.len() - skipped, winwidth);
            assert_eq!(display_line, expected_display_line);
        }

        // span < winwidth
        let winwidth = 50;
        let query = "files";
        let origin_line =
            "directories/are/nested/a/lot/then/the/matched/items/will/be/invisible/file.scss";
        let expected_display_line = "then/the/matched/items/will/be/invisible/file.scss";
        ensure_printer_works(winwidth, 0, query, origin_line, expected_display_line);

        let winwidth = 58;
        let query = "bmine";
        let origin_line =
            "        // Wait until propagation delay period after block we plan to mine on";
        let expected_display_line = "il propagation delay period after block we plan to mine on";
        ensure_printer_works(winwidth, 0, query, origin_line, expected_display_line);

        let winwidth = 50;
        let query = "srlisresource";
        let origin_line = "fuzzy-filter/target/debug/deps/librustversion-b273394e6c9c64f6.dylib.dSYM/Contents/Resources/DWARF/librustversion-b273394e6c9c64f6.dylib";
        let expected_display_line = "..rustversion-b273394e6c9c64f6.dylib.dSYM/Conten..";
        ensure_printer_works(winwidth, 0, query, origin_line, expected_display_line);

        // skipped should work.
        let winwidth = 50;
        let query = "srlisrlisrsr";
        let origin_line = " crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib";
        // let expected_display_line = " ..s/fuzzy_filter/target/debug/deps/librustvers..";
        let expected_display_line = " ..fuzzy_filter/target/debug/deps/librustversio..";
        ensure_printer_works(winwidth, 2, query, origin_line, expected_display_line);

        let winwidth = 50;
        let query = "srlisrlisrsr";
        let origin_line = " crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib";
        let expected_display_line = "..tes/fuzzy_filter/target/debug/deps/librustvers..";
        ensure_printer_works(winwidth, 0, query, origin_line, expected_display_line);

        let origin_line = " crates/printer/src/lib.rs";
        let matcher = Matcher::new(Algo::Fzy, MatchType::Full, Bonus::FileName);
        let printer = Printer::new(60, 2);
        let (display_line, _adjusted_indices) =
            printer.display(origin_line, &[10, 12, 13]).unwrap();
        let expected_display_line = " crates/printer/src/lib.rs";
        assert_eq!(origin_line, expected_display_line);
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
