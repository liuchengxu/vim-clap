use std::convert::TryFrom;
use std::path::PathBuf;

use anyhow::Result;
use dumb_analyzer::get_comment_syntax;
use rayon::prelude::*;

use super::definition::{
    build_full_regexp, get_definition_rules, is_comment, DefinitionKind, DefinitionSearchResult,
    Definitions, Occurrences,
};
use crate::process::AsyncCommand;
use crate::tools::ripgrep::{Match, Word};

/// Searches a directory for pattern matches using ripgrep.
#[derive(Debug, Clone)]
pub struct MatchFinder<'a> {
    /// Directory to perform the ripgrep search.
    pub dir: Option<&'a PathBuf>,
    /// Keyword of searching.
    pub word: &'a Word,
    /// Extension of the source file.
    pub file_ext: &'a str,
}

impl<'a> MatchFinder<'a> {
    pub(super) fn find_occurrences(&self, ignore_comment: bool) -> Result<Vec<Match>> {
        let command = format!(
            "rg --json --word-regexp '{}' -g '*.{}'",
            self.word.raw, self.file_ext
        );
        self.find_matches(
            command,
            if ignore_comment {
                Some(get_comment_syntax(self.file_ext))
            } else {
                None
            },
        )
    }

    /// Executes `command` as a child process.
    ///
    /// Convert the entire output into a stream of ripgrep `Match`.
    fn find_matches(&self, command: String, maybe_comments: Option<&[&str]>) -> Result<Vec<Match>> {
        let mut cmd = AsyncCommand::new(command);

        if let Some(ref dir) = self.dir {
            cmd.current_dir(dir);
        }

        let stdout = cmd.stdout()?;

        Ok(stdout
            .par_split(|x| x == &b'\n')
            .filter_map(|s| {
                Match::try_from(s).ok().filter(|matched| {
                    maybe_comments
                        .map(|comments| !is_comment(matched, comments))
                        .unwrap_or(true)
                })
            })
            .collect())
    }
}

/// [`MatchFinder`] with a known language type.
#[derive(Debug, Clone)]
pub struct RegexRunner<'a> {
    /// Match finder.
    pub finder: MatchFinder<'a>,
    /// Language type defined by ripgrep.
    pub lang: &'a str,
}

impl<'a> RegexRunner<'a> {
    pub fn new(finder: MatchFinder<'a>, lang: &'a str) -> Self {
        Self { finder, lang }
    }

    /// Finds the occurrences and all definitions concurrently.
    pub fn all(&self, comments: &[&str]) -> (Definitions, Occurrences) {
        let (definitions, occurrences) = (self.definitions(), self.occurrences(comments));

        (
            Definitions {
                defs: definitions.unwrap_or_default(),
            },
            Occurrences(occurrences.unwrap_or_default()),
        )
    }

    /// Returns all kinds of definitions.
    pub fn definitions(&self) -> Result<Vec<DefinitionSearchResult>> {
        Ok(get_definition_rules(self.lang)?
            .0
            .keys()
            .map(|kind| self.find_definitions(kind))
            .par_bridge()
            .filter_map(|def| {
                def.ok()
                    .map(|(kind, matches)| DefinitionSearchResult { kind, matches })
            })
            .collect())
    }

    /// Finds all the occurrences of `word`.
    ///
    /// Basically the occurrences are composed of definitions and usages.
    fn occurrences(&self, comments: &[&str]) -> Result<Vec<Match>> {
        let command = format!(
            "rg --json --word-regexp '{}' --type {}",
            self.finder.word.raw, self.lang
        );

        self.finder.find_matches(command, Some(comments))
    }

    pub(super) fn regexp_search(&self, comments: &[&str]) -> Result<Vec<Match>> {
        let command = format!(
            "rg --json -e '{}' --type {}",
            self.finder.word.raw.replace(char::is_whitespace, ".*"),
            self.lang
        );
        self.finder.find_matches(command, Some(comments))
    }

    /// Returns a tuple of (definition_kind, ripgrep_matches) by searching given language `lang`.
    fn find_definitions(&self, kind: &DefinitionKind) -> Result<(DefinitionKind, Vec<Match>)> {
        let regexp = build_full_regexp(self.lang, kind, self.finder.word)?;
        let command = format!(
            "rg --trim --json --pcre2 --type {} -e '{}'",
            self.lang, regexp
        );
        self.finder
            .find_matches(command, None)
            .map(|defs| (kind.clone(), defs))
    }
}
