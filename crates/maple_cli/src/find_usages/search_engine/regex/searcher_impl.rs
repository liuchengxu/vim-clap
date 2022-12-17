use super::definition::{
    build_full_regexp, get_definition_rules, is_comment, DefinitionKind, DefinitionSearchResult,
    Definitions, Occurrences,
};
use crate::tools::ripgrep::{Match, Word};
use anyhow::Result;
use dumb_analyzer::get_comment_syntax;
use rayon::prelude::*;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::process::Command;

/// Searches a directory for pattern matches using ripgrep.
#[derive(Debug, Clone)]
pub struct ExecutableSearcher {
    /// Directory to perform the ripgrep search.
    pub dir: Option<PathBuf>,
    /// Keyword of searching.
    pub word: Word,
    /// Extension of the source file.
    pub file_ext: String,
}

impl ExecutableSearcher {
    pub(super) fn word_regexp_search(&self, ignore_comment: bool) -> std::io::Result<Vec<Match>> {
        let mut command = Command::new("rg");
        command
            .arg("--json")
            .arg("--word-regexp")
            .arg(&self.word.raw)
            .arg("-g")
            .arg(format!("*.{}", self.file_ext));
        self.search(
            command,
            if ignore_comment {
                Some(get_comment_syntax(&self.file_ext))
            } else {
                None
            },
        )
    }

    /// Executes `command` as a child process.
    ///
    /// Convert the entire output into a stream of ripgrep `Match`.
    fn search(&self, cmd: Command, maybe_comments: Option<&[&str]>) -> std::io::Result<Vec<Match>> {
        let mut cmd = cmd;

        if let Some(ref dir) = self.dir {
            cmd.current_dir(dir);
        }

        let cmd_output = cmd.output()?;

        if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from_utf8_lossy(&cmd_output.stderr),
            ));
        }

        Ok(cmd_output
            .stdout
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

/// [`ExecutableSearcher`] with a known language type.
#[derive(Debug, Clone)]
pub struct RegexSearcherImpl {
    /// Searcher.
    pub searcher: ExecutableSearcher,
    /// Language type defined by ripgrep.
    pub lang: String,
}

impl RegexSearcherImpl {
    pub fn new(searcher: ExecutableSearcher, lang: String) -> Self {
        Self { searcher, lang }
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
    fn definitions(&self) -> Result<Vec<DefinitionSearchResult>> {
        Ok(get_definition_rules(&self.lang)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Can not find the definition rules",
                )
            })?
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
    fn occurrences(&self, comments: &[&str]) -> std::io::Result<Vec<Match>> {
        let mut command = Command::new("rg");
        command
            .arg("--json")
            .arg("--word-regexp")
            .arg(&self.searcher.word.raw)
            .arg("--type")
            .arg(&self.lang);
        self.searcher.search(command, Some(comments))
    }

    pub(super) fn regexp_search(&self, comments: &[&str]) -> std::io::Result<Vec<Match>> {
        let mut command = Command::new("rg");
        command
            .arg("--json")
            .arg("--regexp")
            .arg(self.searcher.word.raw.replace(char::is_whitespace, ".*"))
            .arg("--type")
            .arg(&self.lang);
        self.searcher.search(command, Some(comments))
    }

    /// Returns a tuple of (definition_kind, ripgrep_matches) by searching given language `lang`.
    fn find_definitions(
        &self,
        kind: &DefinitionKind,
    ) -> std::io::Result<(DefinitionKind, Vec<Match>)> {
        let regexp = build_full_regexp(&self.lang, kind, &self.searcher.word).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Can not find the definition rule",
            )
        })?;
        let mut command = Command::new("rg");
        command
            .arg("--trim")
            .arg("--json")
            .arg("--pcre2")
            .arg("--regexp")
            .arg(regexp)
            .arg("--type")
            .arg(&self.lang);
        self.searcher
            .search(command, None)
            .map(|defs| (kind.clone(), defs))
    }
}
