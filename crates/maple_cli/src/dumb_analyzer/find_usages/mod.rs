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
mod search;

use std::path::PathBuf;

use anyhow::Result;
use rayon::prelude::*;

use self::definition::{get_definition_rules, DefinitionSearchResult, Definitions, Occurrences};
use self::search::{find_definition_matches_with_kind, find_occurrences_by_lang};
use crate::tools::ripgrep::{Match, Word};

pub use self::definition::{
    definitions_and_references, definitions_and_references_lines, get_comments_by_ext,
    get_language_by_ext, DefinitionRules, MatchKind,
};
pub use self::search::find_occurrence_matches_by_ext;

/// Usages consists of [`Definitions`] and [`Occurrences`].
#[derive(Debug, Clone)]
pub struct FindUsages<'a> {
    lang: &'a str,
    word: &'a Word,
    dir: &'a Option<PathBuf>,
}

impl<'a> FindUsages<'a> {
    /// Constructs a new instance of [`FindUsages`].
    pub fn new(lang: &'a str, word: &'a Word, dir: &'a Option<PathBuf>) -> Self {
        Self { lang, word, dir }
    }

    /// Finds the occurrences and all definitions concurrently.
    pub async fn all(&self, comments: &[&str]) -> (Definitions, Occurrences) {
        let (definitions, occurrences) =
            futures::future::join(self.definitions(), self.occurrences(comments)).await;

        (
            Definitions {
                defs: definitions.unwrap_or_default(),
            },
            Occurrences(occurrences.unwrap_or_default()),
        )
    }

    /// Returns all kinds of definitions.
    pub async fn definitions(&self) -> Result<Vec<DefinitionSearchResult>> {
        let all_def_futures = get_definition_rules(self.lang)?
            .0
            .keys()
            .map(|kind| find_definition_matches_with_kind(self.lang, kind, self.word, self.dir));

        let maybe_defs = futures::future::join_all(all_def_futures).await;

        Ok(maybe_defs
            .into_par_iter()
            .filter_map(|def| {
                def.ok()
                    .map(|(kind, matches)| DefinitionSearchResult { kind, matches })
            })
            .collect())
    }

    /// Returns all the occurrences.
    #[inline]
    async fn occurrences(&self, comments: &[&str]) -> Result<Vec<Match>> {
        find_occurrences_by_lang(self.word, self.lang, self.dir, comments).await
    }
}
