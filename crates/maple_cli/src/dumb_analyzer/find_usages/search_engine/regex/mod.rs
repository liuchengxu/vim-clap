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
mod worker;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use rayon::prelude::*;

use self::definition::{
    definitions_and_references, get_comments_by_ext, get_language_by_ext, search_usages_impl,
    MatchKind,
};
use self::worker::{
    find_definition_matches_with_kind, find_occurrence_matches_by_ext, find_occurrences_by_lang,
};
use crate::dumb_analyzer::find_usages::UsagesInfo;
use crate::tools::ripgrep::{Match, Word};
use crate::utils::ExactOrInverseTerms;

#[derive(Clone, Debug)]
pub struct RegexSearcher {
    pub word: String,
    pub extension: String,
    pub dir: Option<PathBuf>,
}

impl RegexSearcher {
    /// Search the definitions and references if language type is detected, otherwise
    /// search the occurrences.
    pub async fn search_usages(
        self,
        classify: bool,
        exact_or_inverse_terms: &ExactOrInverseTerms,
    ) -> Result<UsagesInfo> {
        let Self {
            word,
            extension,
            dir,
        } = self;

        let word = Word::new(word)?;

        let lang = match get_language_by_ext(&extension) {
            Ok(lang) => lang,
            Err(_) => {
                // Search the occurrences if no language detected.
                let occurrences = find_occurrence_matches_by_ext(&word, &extension, &dir).await?;
                let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = occurrences
                    .into_par_iter()
                    .filter_map(|line| {
                        exact_or_inverse_terms.check_jump_line(line.build_jump_line("refs", &word))
                    })
                    .unzip();
                return Ok(UsagesInfo::new(lines, indices));
            }
        };

        let comments = get_comments_by_ext(&extension);

        // render the results in group.
        if classify {
            let res = definitions_and_references(lang, &word, &dir, comments).await?;

            let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = res
                .into_par_iter()
                .flat_map(|(match_kind, matches)| render_classify(matches, &match_kind, &word))
                .unzip();

            Ok(UsagesInfo::new(lines, indices))
        } else {
            search_usages_impl(lang, &word, &dir, comments, exact_or_inverse_terms).await
        }
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
