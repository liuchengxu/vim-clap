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
mod executable_searcher;

use self::definition::{find_definitions_and_references, DefinitionSearchResult, MatchKind};
use self::executable_searcher::{word_regex_search_with_extension, LanguageRegexSearcher};
use crate::find_usages::{AddressableUsage, Usage, UsageMatcher, Usages};
use crate::tools::rg::{get_language, Match, Word};
use code_tools::analyzer::{resolve_reference_kind, Priority};
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::{Error, Result};
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
        Some(self.cmp(other))
    }
}

impl Ord for RegexUsage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.pattern_priority
            .cmp(&other.pattern_priority)
            .then_with(|| self.path.cmp(&other.path))
            .then_with(|| self.line_number.cmp(&other.line_number))
    }
}

#[derive(Clone, Debug)]
pub struct RegexSearcher {
    pub word: String,
    pub extension: String,
    pub dir: Option<PathBuf>,
}

impl RegexSearcher {
    pub fn cli_usages(&self, usage_matcher: &UsageMatcher) -> Result<Usages> {
        let usages: Usages = self.search_usages(false, usage_matcher)?.into();
        Ok(usages)
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

        let re = regex::Regex::new(&format!("\\b{word}\\b"))
            .map_err(|e| Error::other(format!("{word} is an invalid regex expression: {e}")))?;

        let word = Word::new(word.clone(), re);

        let Some(lang) = get_language(extension) else {
            // Search the occurrences if no language detected.
            let occurrences =
                word_regex_search_with_extension(&word.raw, true, extension, dir.as_ref())?;
            let mut usages = occurrences
                .into_iter()
                .filter_map(|matched| {
                    usage_matcher
                        .match_jump_line(matched.build_jump_line("refs", &word))
                        .map(|(line, indices)| RegexUsage::from_matched(&matched, line, indices))
                })
                .collect::<Vec<_>>();
            usages.par_sort_unstable();
            return Ok(usages.into_iter().map(Into::into).collect());
        };

        let lang_regex_searcher =
            LanguageRegexSearcher::new(dir.clone(), word.clone(), lang.to_string());

        let comments = code_tools::language::get_line_comments(extension);

        // render the results in group.
        if classify {
            let res = find_definitions_and_references(lang_regex_searcher, comments)?;

            let _usages = res
                .into_iter()
                .flat_map(|(match_kind, matches)| render_classify(matches, &match_kind, &word))
                .map(|(line, indices)| Usage::new(line, indices))
                .collect::<Vec<_>>();

            unimplemented!("Classify regex search")
            // Ok(usages.into())
        } else {
            self.regex_search(lang_regex_searcher, comments, usage_matcher)
        }
    }

    /// Search the usages using the pre-defined regex matching rules.
    ///
    /// If the result from regex matching is empty, try the pure grep approach.
    fn regex_search(
        &self,
        lang_regex_searcher: LanguageRegexSearcher,
        comments: &[String],
        usage_matcher: &UsageMatcher,
    ) -> Result<Vec<AddressableUsage>> {
        let (definitions, occurrences) = lang_regex_searcher.all(comments);

        let defs = definitions.flatten();

        // There are some negative definitions we need to filter them out, e.g., the word
        // is a substring in some identifier but we consider every word is a valid identifier.
        let positive_defs = defs
            .iter()
            .filter(|def| occurrences.contains(def))
            .collect::<Vec<_>>();

        let word = &lang_regex_searcher.word;

        let mut regex_usages = definitions
            .into_iter()
            .flat_map(|DefinitionSearchResult { kind, matches }| {
                matches
                    .into_iter()
                    .filter_map(|matched| {
                        if positive_defs.contains(&&matched) {
                            usage_matcher
                                .match_jump_line(matched.build_jump_line(kind.as_ref(), word))
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
                occurrences.into_iter().filter_map(|matched| {
                    if !defs.contains(&matched) {
                        let (kind, _) = resolve_reference_kind(matched.pattern(), &self.extension);
                        usage_matcher
                            .match_jump_line(matched.build_jump_line(kind, word))
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
            let lines = lang_regex_searcher.regexp_search(comments)?;
            let mut grep_usages = lines
                .into_par_iter()
                .filter_map(|matched| {
                    usage_matcher
                        .match_jump_line(matched.build_jump_line("grep", word))
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
            word: "clap#legacy#filter#async#dyn#start_filter_with_cache".into(),
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
                .contains("function! clap#legacy#filter#async#dyn#start_filter_with_cache"));
            assert!(usages[1]
                .line
                .contains("call clap#legacy#filter#async#dyn#start_filter_with_cache"));
        }
    }
}
