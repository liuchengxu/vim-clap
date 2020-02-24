use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::PathBuf;

use anyhow::Result;
use extracted_fzy::match_and_score_with_positions;
use fuzzy_matcher::skim::fuzzy_indices;
use rayon::prelude::*;
use structopt::clap::arg_enum;

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

pub type JustifiedMap = HashMap<String, String>;
pub type FuzzyMatchedLine = (String, f64, Vec<usize>);

impl Source {
    pub fn filter(self, algo: Algo, query: &str) -> Result<Vec<FuzzyMatchedLine>> {
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

pub fn fuzzy_filter_and_rank(
    query: &str,
    input: Option<PathBuf>,
    algo: Algo,
) -> Result<Vec<(String, f64, Vec<usize>)>> {
    let source = if let Some(fpath) = input {
        Source::File(fpath)
    } else {
        Source::Stdin
    };

    let mut ranked = source.filter(algo, query)?;

    ranked.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

    Ok(ranked)
}

// Long matched lines can cause the matched items invisible.
pub fn justify(
    ranked: impl IntoIterator<Item = FuzzyMatchedLine>,
    win_width: u16,
    starting_point: Option<usize>,
) -> (Vec<FuzzyMatchedLine>, JustifiedMap) {
    let win_width = win_width as usize;
    let mut justified_map = HashMap::new();
    let justified = ranked
        .into_iter()
        .map(|(line, score, indices)| {
            if let Some(last) = indices.last() {
                if *last > win_width {
                    let dots = "...";
                    let mut start = *last - win_width + dots.len();
                    if start > indices[0] {
                        start = indices[0];
                    }
                    if let Some(starting_point) = starting_point {
                        start += starting_point;
                    }
                    let end = line.len();
                    let truncated = if let Some(starting_point) = starting_point {
                        format!(
                            "{}{}{}",
                            &line[..starting_point],
                            dots,
                            &line[start - 2..end]
                        )
                    } else {
                        format!("{}{}", dots, &line[start..end])
                    };
                    let offset = line.len() - truncated.len();
                    let truncated_indices = indices.iter().map(|x| x - offset).collect::<Vec<_>>();
                    justified_map.insert(truncated.clone(), line.clone());
                    (truncated, score, truncated_indices)
                } else {
                    (line, score, indices)
                }
            } else {
                (line, score, indices)
            }
        })
        .collect::<Vec<_>>();
    (justified, justified_map)
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
    #[test]
    fn truncate_to_matched() {
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

        let source: Source = vec![
          "fuzzy-filter/target/debug/deps/librustversion-b273394e6c9c64f6.dylib.dSYM/Contents/Resources/DWARF/librustversion-b273394e6c9c64f6.dylib".into(),
          "fuzzy-filter/target/debug/deps/librustversion-15764ff2535f190d.dylib.dSYM/Contents/Resources/DWARF/librustversion-15764ff2535f190d.dylib".into(),
          "target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib".into(),
          "target/debug/deps/libstructopt_derive-3921fbf02d8d2ffe.dylib.dSYM/Contents/Resources/DWARF/libstructopt_derive-3921fbf02d8d2ffe.dylib".into(),
        ].into();
        let query = "srlisrsr";

        // let source: Source = vec![
        // "/Users/xuliucheng/Library/Caches/Homebrew/universal-ctags--git/Units/afl-fuzz.r/github-issue-625-r.d/input.r".into()
        // ].into();
        // let query = "srcggithub";
        //
        //

        let mut ranked = source.filter(Algo::Fzy, query).unwrap();
        ranked.par_sort_unstable_by(|(_, v1, _), (_, v2, _)| v2.partial_cmp(&v1).unwrap());

        println!("");
        println!("ranked: {:?}", ranked);

        let win_width = 62u16;
        let (justified, truncated_map) = justify(ranked, win_width, None);
        for (truncated_line, _score, truncated_indices) in justified.iter() {
            println!("truncated: {}", "-".repeat(win_width as usize));
            println!(
                "truncated: {}",
                wrap_matches(&truncated_line, &truncated_indices)
            );
        }

        println!("justified: {:?}", justified);
        println!("truncated_map: {:?}", truncated_map);

        // [--------------------------]
        // [------------------...|---------------xx--x---]
        //                     [-----------------xx--x---]
        // [-----------------xx--x---]
        // println!("{:#?}", ranked);
    }
}
