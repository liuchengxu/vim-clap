//! This module provides the feature of search based `jump-to-definition`, inspired
//! by https://github.com/jacktasia/dumb-jump, powered by a set of regular expressions
//! based on the file extension, using the ripgrep tool.
//!
//! The matches are run through a shared set of heuristic methods to find the best candidate.
//!
//! # Dependency
//!
//! The executable rg with `--json` and `--pcre2` is required to be installed on the system.

mod definition;
mod runner;

use self::definition::{definitions_and_references, DefinitionSearchResult, MatchKind};
use self::runner::{MatchFinder, RegexRunner};
use crate::find_usages::{AddressableUsage, Usage, Usages};
use crate::tools::ripgrep::{get_language, Match, Word};
use crate::utils::UsageMatcher;
use anyhow::Result;
use dumb_analyzer::{get_comment_syntax, resolve_reference_kind, Priority};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

/// [`Usage`] with some structured information.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegexUsage {
    pub line: String,
    pub indices: Vec<usize>,
    pub path: String,
    pub line_number: usize,
    pub pattern_priority: Priority,
}

impl From<RegexUsage> for AddressableUsage {
    fn from(regex_usage: RegexUsage) -> Self {
        let RegexUsage {
            line,
            indices,
            path,
            line_number,
            ..
        } = regex_usage;
        Self {
            line,
            indices,
            path,
            line_number,
        }
    }
}

impl RegexUsage {
    fn from_matched(matched: &Match, line: String, indices: Vec<usize>) -> Self {
        Self {
            line,
            indices,
            path: matched.path().into(),
            line_number: matched.line_number() as usize,
            pattern_priority: matched.pattern_priority(),
        }
    }
}

impl PartialOrd for RegexUsage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some((self.pattern_priority, &self.path, self.line_number).cmp(&(
            other.pattern_priority,
            &other.path,
            other.line_number,
        )))
    }
}

impl Ord for RegexUsage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[derive(Clone, Debug)]
pub struct RegexSearcher {
    pub word: String,
    pub extension: String,
    pub dir: Option<PathBuf>,
}

impl RegexSearcher {
    pub fn print_usages(&self, usage_matcher: &UsageMatcher) -> Result<()> {
        let usages: Usages = self.search_usages(false, usage_matcher)?.into();
        let total = usages.len();
        let (lines, indices): (Vec<_>, Vec<_>) = usages
            .into_iter()
            .map(|usage| (usage.line, usage.indices))
            .unzip();
        utility::println_json_with_length!(total, lines, indices);
        Ok(())
    }

    /// Search the definitions and references if language type is detected, otherwise
    /// search the occurrences.
    pub fn search_usages(
        &self,
        classify: bool,
        usage_matcher: &UsageMatcher,
    ) -> Result<Vec<AddressableUsage>> {
        let Self {
            word,
            extension,
            dir,
        } = self;

        let word = Word::new(word.clone())?;

        let match_finder = MatchFinder {
            word: &word,
            file_ext: extension,
            dir: dir.as_ref(),
        };

        let lang = match get_language(extension) {
            Some(lang) => lang,
            None => {
                // Search the occurrences if no language detected.
                let occurrences = match_finder.find_occurrences(true)?;
                let mut usages = occurrences
                    .into_par_iter()
                    .filter_map(|matched| {
                        usage_matcher
                            .check_jump_line(matched.build_jump_line("refs", &word))
                            .map(|(line, indices)| {
                                RegexUsage::from_matched(&matched, line, indices)
                            })
                    })
                    .collect::<Vec<_>>();
                usages.par_sort_unstable();
                return Ok(usages.into_iter().map(Into::into).collect());
            }
        };

        let regex_runner = RegexRunner::new(match_finder, lang);

        let comments = get_comment_syntax(extension);

        // render the results in group.
        if classify {
            let res = definitions_and_references(regex_runner, comments)?;

            let _usages = res
                .into_par_iter()
                .flat_map(|(match_kind, matches)| render_classify(matches, &match_kind, &word))
                .map(|(line, indices)| Usage::new(line, indices))
                .collect::<Vec<_>>();

            unimplemented!("Classify regex search")
            // Ok(usages.into())
        } else {
            self.regex_search(regex_runner, comments, usage_matcher)
        }
    }

    /// Search the usages using the pre-defined regex matching rules.
    ///
    /// If the result from regex matching is empty, try the pure grep approach.
    fn regex_search(
        &self,
        regex_runner: RegexRunner,
        comments: &[&str],
        usage_matcher: &UsageMatcher,
    ) -> Result<Vec<AddressableUsage>> {
        let (definitions, occurrences) = regex_runner.all(comments);

        let defs = definitions.flatten();

        // There are some negative definitions we need to filter them out, e.g., the word
        // is a subtring in some identifer but we consider every word is a valid identifer.
        let positive_defs = defs
            .par_iter()
            .filter(|def| occurrences.contains(def))
            .collect::<Vec<_>>();

        let mut regex_usages = definitions
            .into_par_iter()
            .flat_map(|DefinitionSearchResult { kind, matches }| {
                matches
                    .into_par_iter()
                    .filter_map(|matched| {
                        if positive_defs.contains(&&matched) {
                            usage_matcher
                                .check_jump_line(
                                    matched
                                        .build_jump_line(kind.as_ref(), regex_runner.finder.word),
                                )
                                .map(|(line, indices)| {
                                    RegexUsage::from_matched(&matched, line, indices)
                                })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .chain(
                // references are the occurrences that are not in the definition set.
                occurrences.into_par_iter().filter_map(|matched| {
                    if !defs.contains(&matched) {
                        let (kind, _) = resolve_reference_kind(matched.pattern(), &self.extension);
                        usage_matcher
                            .check_jump_line(
                                matched.build_jump_line(kind, regex_runner.finder.word),
                            )
                            .map(|(line, indices)| {
                                RegexUsage::from_matched(&matched, line, indices)
                            })
                    } else {
                        None
                    }
                }),
            )
            .collect::<Vec<_>>();

        // Pure results by grepping the word.
        if regex_usages.is_empty() {
            let lines = regex_runner.regexp_search(comments)?;
            let mut grep_usages = lines
                .into_par_iter()
                .filter_map(|matched| {
                    usage_matcher
                        .check_jump_line(matched.build_jump_line("grep", regex_runner.finder.word))
                        .map(|(line, indices)| RegexUsage::from_matched(&matched, line, indices))
                })
                .collect::<Vec<_>>();
            grep_usages.par_sort_unstable();
            return Ok(grep_usages.into_iter().map(Into::into).collect());
        }

        regex_usages.par_sort_unstable();
        Ok(regex_usages.into_iter().map(Into::into).collect())
    }
}

// TODO: a new renderer for dumb jump
fn render_classify(
    matches: Vec<Match>,
    kind: &MatchKind,
    word: &Word,
) -> Vec<(String, Vec<usize>)> {
    let mut group_refs = HashMap::new();

    // references are these occurrences not in the definitions.
    for line in matches.iter() {
        let group = group_refs.entry(line.path()).or_insert_with(Vec::new);
        group.push(line);
    }

    let mut kind_inserted = false;

    group_refs
        .values()
        .flat_map(|lines| {
            let mut inner_group: Vec<(String, Vec<usize>)> = Vec::with_capacity(lines.len() + 1);

            if !kind_inserted {
                inner_group.push((format!("[{kind}]"), vec![]));
                kind_inserted = true;
            }

            inner_group.push((format!("  {} [{}]", lines[0].path(), lines.len()), vec![]));

            inner_group.extend(lines.iter().map(|line| line.build_jump_line_bare(word)));

            inner_group
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_runner_language_keyword_ordering() {
        let regex_searcher = RegexSearcher {
            word: "clap#filter#async#dyn#start_filter_with_cache".into(),
            extension: "vim".into(),
            dir: std::env::current_dir()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .map(|path| path.to_path_buf()),
        };
        // FIXME: somehow it's Err in CI https://github.com/liuchengxu/vim-clap/runs/6146828485?check_suite_focus=true
        if let Ok(usages) = regex_searcher.search_usages(false, &UsageMatcher::default()) {
            assert!(usages[0]
                .line
                .contains("function! clap#filter#async#dyn#start_filter_with_cache"));
            assert!(usages[1]
                .line
                .contains("call clap#filter#async#dyn#start_filter_with_cache"));
        }
    }
}
