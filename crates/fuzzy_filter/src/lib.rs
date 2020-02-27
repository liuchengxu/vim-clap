use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::PathBuf;

use anyhow::Result;
use extracted_fzy::match_and_score_with_positions;
use fuzzy_matcher::skim::fuzzy_indices;
use rayon::prelude::*;
use structopt::clap::arg_enum;

pub const DOTS: &str = "...";

// Implement arg_enum for using it in the command line arguments.
arg_enum! {
  #[derive(Debug)]
  pub enum Algo {
      Skim,
      Fzy,
  }
}

/// The filtering source can from stdin, an input file or Vec<StringString>
pub enum Source {
    Stdin,
    File(PathBuf),
    List(Vec<String>),
}

impl From<Vec<String>> for Source {
    fn from(source_list: Vec<String>) -> Self {
        Self::List(source_list)
    }
}

impl From<PathBuf> for Source {
    fn from(fpath: PathBuf) -> Self {
        Self::File(fpath)
    }
}

pub type LinesTruncatedMap = HashMap<String, String>;
pub type FuzzyMatchedLineInfo = (String, f64, Vec<usize>);

impl Source {
    pub fn filter(self, algo: Algo, query: &str) -> Result<Vec<FuzzyMatchedLineInfo>> {
        let scorer = |line: &str| match algo {
            Algo::Skim => {
                fuzzy_indices(line, &query).map(|(score, indices)| (score as f64, indices))
            }
            Algo::Fzy => match_and_score_with_positions(&query, line),
        };

        let filtered = match self {
            Self::Stdin => io::stdin()
                .lock()
                .lines()
                .filter_map(|lines_iter| {
                    lines_iter.ok().and_then(|line| {
                        scorer(&line).map(|(score, indices)| (line, score, indices))
                    })
                })
                .collect::<Vec<_>>(),
            Self::File(fpath) => std::fs::read_to_string(fpath)?
                .par_lines()
                .filter_map(|line| {
                    scorer(&line).map(|(score, indices)| (line.into(), score, indices))
                })
                .collect::<Vec<_>>(),
            Self::List(list) => list
                .iter()
                .filter_map(|line| {
                    scorer(&line).map(|(score, indices)| (line.into(), score, indices))
                })
                .collect::<Vec<_>>(),
        };

        Ok(filtered)
    }
}

/// Return the ranked results after applying fuzzy filter given the query String and a list of candidates.
pub fn fuzzy_filter_and_rank(
    query: &str,
    input: Option<PathBuf>,
    algo: Algo,
) -> Result<Vec<(String, f64, Vec<usize>)>> {
    let source = input.map(Into::into).unwrap_or(Source::Stdin);

    let mut ranked = source.filter(algo, query)?;

    ranked.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

    Ok(ranked)
}

/// Long matched lines can cause the matched items invisible.
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
pub fn truncate_long_matched_lines(
    lines: impl IntoIterator<Item = FuzzyMatchedLineInfo>,
    winwidth: usize,
    starting_point: Option<usize>,
) -> (Vec<FuzzyMatchedLineInfo>, LinesTruncatedMap) {
    let mut truncated_map = HashMap::new();
    let lines = lines
        .into_iter()
        .map(|(line, score, indices)| {
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
                let truncated = if let Some(starting_point) = starting_point {
                    let icon: String = line.chars().take(starting_point).collect();
                    start += starting_point;
                    format!("{}{}{}", icon, DOTS, &line[start..end])
                } else {
                    format!("{}{}", DOTS, &line[start..end])
                };
                let offset = line_len - truncated.len();
                let truncated_indices = indices.iter().map(|x| x - offset).collect::<Vec<_>>();
                truncated_map.insert(truncated.clone(), line);
                (truncated, score, truncated_indices)
            } else {
                (line, score, indices)
            }
        })
        .collect::<Vec<_>>();
    (lines, truncated_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use termion::style::{Invert, Reset};

    fn wrap_matches(line: &str, indices: &[usize]) -> String {
        let mut ret = String::new();
        let mut peekable = indices.iter().peekable();
        for (idx, ch) in line.chars().enumerate() {
            let next_id = **peekable.peek().unwrap_or(&&line.len());
            if next_id == idx {
                ret.push_str(format!("{}{}{}", Invert, ch, Reset).as_str());
                peekable.next();
            } else {
                ret.push(ch);
            }
        }

        ret
    }

    fn run_test(source: Source, query: &str, starting_point: Option<usize>, winwidth: usize) {
        let mut ranked = source.filter(Algo::Fzy, query).unwrap();
        ranked.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

        println!("");
        println!("query: {:?}", query);

        let (truncated_lines, truncated_map) =
            truncate_long_matched_lines(ranked, winwidth, starting_point);
        for (truncated_line, _score, truncated_indices) in truncated_lines.iter() {
            println!("truncated: {}", "-".repeat(winwidth));
            println!(
                "truncated: {}",
                wrap_matches(&truncated_line, &truncated_indices)
            );
            println!("raw_line: {}", truncated_map.get(truncated_line).unwrap());
        }
    }

    #[test]
    fn case1() {
        let source: Source = vec![
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
        let source: Source = vec![
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
        let source: Source = vec![
        "/Users/xuliucheng/Library/Caches/Homebrew/universal-ctags--git/Units/afl-fuzz.r/github-issue-625-r.d/input.r".into()
        ].into();
        let query = "srcggithub";
        run_test(source, query, None, 50usize);
    }

    #[test]
    fn case4() {
        let source: Source = vec![
            "        // Wait until propagation delay period after block we plan to mine on".into(),
        ]
        .into();
        let query = "bmine";
        run_test(source, query, None, 58usize);
    }

    #[test]
    fn starting_point_should_work() {
        let source: Source = vec![
          " crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib".into(),
          " crates/fuzzy_filter/target/debug/deps/libstructopt_derive-5cce984f248086cc.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-5cce984f248086cc.dylib".into()
        ].into();
        let query = "srlisrlisrsr";
        run_test(source, query, Some(2), 50usize);

        let source: Source = vec![
          "crates/fuzzy_filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib".into(),
          "crates/fuzzy_filter/target/debug/deps/libstructopt_derive-5cce984f248086cc.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-5cce984f248086cc.dylib".into()
        ].into();
        let query = "srlisrlisrsr";
        run_test(source, query, None, 50usize);
    }
}
