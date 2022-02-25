//! This module provides the feature of search based `jump-to-definition`, inspired
//! by https://github.com/jacktasia/dumb-jump, powered by a set of regular expressions
//! based on the file extension, using the ripgrep tool.
//!
//! The matches are run through a shared set of heuristic methods to find the best candidate.
//!
//! # Dependency
//!
//! The executable rg with `--json` and `--pcre2` is required to be installed on the system.

mod default_types;
mod definition;
mod runner;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use dumb_analyzer::{get_comment_syntax, resolve_reference_kind};
use rayon::prelude::*;

use self::definition::{
    definitions_and_references, get_language_by_ext, DefinitionSearchResult, MatchKind,
};
use self::runner::{MatchFinder, RegexRunner};
use crate::find_usages::{AddressableUsage, Usage, Usages};
use crate::tools::ripgrep::{Match, Word};
use crate::utils::ExactOrInverseTerms;

/// [`Usage`] with some structured information.
#[derive(Clone, Debug, Default)]
pub struct RegexUsage {
    pub line: String,
    pub indices: Vec<usize>,
    pub path: String,
    pub line_number: usize,
    pub pattern_weight: usize,
}

impl PartialEq for RegexUsage {
    fn eq(&self, other: &Self) -> bool {
        (self.pattern_weight, &self.path, self.line_number)
            == (self.pattern_weight, &other.path, other.line_number)
    }
}

impl Eq for RegexUsage {}

#[derive(Clone, Debug)]
pub struct RegexSearcher {
    pub word: String,
    pub extension: String,
    pub dir: Option<PathBuf>,
}

impl RegexSearcher {
    pub async fn print_usages(&self, exact_or_inverse_terms: &ExactOrInverseTerms) -> Result<()> {
        let lang = get_language_by_ext(&self.extension)?;

        let comments = get_comment_syntax(&self.extension);

        // TODO: also take word as query?
        let word = Word::new(self.word.clone())?;

        let match_finder = MatchFinder {
            word: &word,
            file_ext: &self.extension,
            dir: self.dir.as_ref(),
        };

        let regex_runner = RegexRunner::new(match_finder, lang);

        let usages: Usages = self
            .regex_search(regex_runner, comments, exact_or_inverse_terms)
            .await?
            .into();

        usages.print();

        Ok(())
    }

    /// Search the definitions and references if language type is detected, otherwise
    /// search the occurrences.
    pub async fn search_usages(
        &self,
        classify: bool,
        exact_or_inverse_terms: &ExactOrInverseTerms,
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

        let lang = match get_language_by_ext(extension) {
            Ok(lang) => lang,
            Err(_) => {
                // Search the occurrences if no language detected.
                let occurrences = match_finder.find_occurrences(true).await?;
                let usages = occurrences
                    .into_par_iter()
                    .filter_map(|matched| {
                        exact_or_inverse_terms
                            .check_jump_line(matched.build_jump_line("refs", &word))
                            .map(|(line, indices)| matched.into_addressable_usage(line, indices))
                    })
                    .collect::<Vec<_>>();
                return Ok(usages);
            }
        };

        let regex_runner = RegexRunner::new(match_finder, lang);

        let comments = get_comment_syntax(extension);

        // render the results in group.
        if classify {
            let res = definitions_and_references(regex_runner, comments).await?;

            let _usages = res
                .into_par_iter()
                .flat_map(|(match_kind, matches)| render_classify(matches, &match_kind, &word))
                .map(|(line, indices)| Usage::new(line, indices))
                .collect::<Vec<_>>();

            unimplemented!("Classify regex search")
            // Ok(usages.into())
        } else {
            self.regex_search(regex_runner, comments, exact_or_inverse_terms)
                .await
        }
    }

    /// Search the usages using the pre-defined regex matching rules.
    ///
    /// If the result from regex matching is empty, try the pure grep approach.
    async fn regex_search<'a>(
        &'a self,
        regex_runner: RegexRunner<'a>,
        comments: &[&str],
        exact_or_inverse_terms: &ExactOrInverseTerms,
    ) -> Result<Vec<AddressableUsage>> {
        let (definitions, occurrences) = regex_runner.all(comments).await;

        let defs = definitions.flatten();

        // There are some negative definitions we need to filter them out, e.g., the word
        // is a subtring in some identifer but we consider every word is a valid identifer.
        let positive_defs = defs
            .par_iter()
            .filter(|def| occurrences.contains(def))
            .collect::<Vec<_>>();

        let regex_usages = definitions
            .into_par_iter()
            .flat_map(|DefinitionSearchResult { kind, matches }| {
                matches
                    .into_par_iter()
                    .filter_map(|matched| {
                        if positive_defs.contains(&&matched) {
                            exact_or_inverse_terms
                                .check_jump_line(
                                    matched
                                        .build_jump_line(kind.as_ref(), regex_runner.finder.word),
                                )
                                .map(|(line, indices)| {
                                    matched.into_addressable_usage(line, indices)
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
                        exact_or_inverse_terms
                            .check_jump_line(
                                matched.build_jump_line(kind, regex_runner.finder.word),
                            )
                            .map(|(line, indices)| matched.into_addressable_usage(line, indices))
                    } else {
                        None
                    }
                }),
            )
            .collect::<Vec<_>>();

        // Pure results by grepping the word.
        if regex_usages.is_empty() {
            let lines = regex_runner.regexp_search(comments).await?;
            let grep_usages = lines
                .into_par_iter()
                .filter_map(|matched| {
                    exact_or_inverse_terms
                        .check_jump_line(matched.build_jump_line("grep", regex_runner.finder.word))
                        .map(|(line, indices)| matched.into_addressable_usage(line, indices))
                })
                .collect::<Vec<_>>();
            return Ok(grep_usages);
        }

        Ok(regex_usages)
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
                inner_group.push((format!("[{}]", kind), vec![]));
                kind_inserted = true;
            }

            inner_group.push((format!("  {} [{}]", lines[0].path(), lines.len()), vec![]));

            inner_group.extend(lines.iter().map(|line| line.build_jump_line_bare(word)));

            inner_group
        })
        .collect()
}
