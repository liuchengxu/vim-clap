use super::definition::{
    build_full_regexp, get_definition_rules, is_comment, DefinitionKind, DefinitionSearchResult,
    Definitions, Occurrences,
};
use crate::tools::rg::{Match, Word, RG_EXISTS};
use rayon::prelude::*;
use std::convert::TryFrom;
use std::io::{Error, ErrorKind, Result};
use std::path::PathBuf;
use std::process::Command;

/// Searches a directory for pattern matches using ripgrep.
#[derive(Debug)]
pub struct ExecutableSearcher {
    command: Command,
}

impl ExecutableSearcher {
    fn new(command: Command) -> Result<Self> {
        if !*RG_EXISTS {
            return Err(Error::new(
                ErrorKind::NotFound,
                String::from("rg executable not found"),
            ));
        }

        Ok(Self { command })
    }

    /// Executes `command` as a child process.
    ///
    /// Convert the entire output into a stream of ripgrep `Match`.
    fn search(self, maybe_comments: Option<&[String]>) -> Result<Vec<Match>> {
        let mut cmd = self.command;

        let cmd_output = cmd.output()?;

        if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
            return Err(Error::other(String::from_utf8_lossy(&cmd_output.stderr)));
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

pub(super) fn word_regex_search_with_extension(
    search_pattern: &str,
    ignore_comment: bool,
    file_extension: &str,
    maybe_dir: Option<&PathBuf>,
) -> Result<Vec<Match>> {
    let mut command = Command::new("rg");
    command
        .arg("--json")
        .arg("--word-regexp")
        .arg(search_pattern)
        .arg("-g")
        .arg(format!("*.{file_extension}"));
    if let Some(ref dir) = maybe_dir {
        command.current_dir(dir);
    }
    ExecutableSearcher::new(command)?.search(if ignore_comment {
        Some(code_tools::language::get_line_comments(file_extension))
    } else {
        None
    })
}

/// [`LanguageRegexSearcher`] with a known language type.
#[derive(Debug, Clone)]
pub struct LanguageRegexSearcher {
    /// Directory to perform the ripgrep search.
    pub dir: Option<PathBuf>,
    /// Keyword of searching.
    pub word: Word,
    /// Language type defined by ripgrep.
    pub lang: String,
}

impl LanguageRegexSearcher {
    pub fn new(dir: Option<PathBuf>, word: Word, lang: String) -> Self {
        Self { dir, word, lang }
    }

    /// Finds the occurrences and all definitions concurrently.
    pub fn all(&self, comments: &[String]) -> (Definitions, Occurrences) {
        (
            Definitions {
                defs: self.definitions().unwrap_or_default(),
            },
            Occurrences(self.occurrences(comments).unwrap_or_default()),
        )
    }

    /// Returns all kinds of definitions.
    fn definitions(&self) -> Result<Vec<DefinitionSearchResult>> {
        Ok(get_definition_rules(&self.lang)
            .ok_or_else(|| Error::other("Can not find the definition rules"))?
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
    fn occurrences(&self, comments: &[String]) -> Result<Vec<Match>> {
        let mut command = Command::new("rg");
        command
            .arg("--json")
            .arg("--word-regexp")
            .arg(&self.word.raw)
            .arg("--type")
            .arg(&self.lang);
        if let Some(ref dir) = self.dir {
            command.current_dir(dir);
        }
        ExecutableSearcher::new(command)?.search(Some(comments))
    }

    pub(super) fn regexp_search(&self, comments: &[String]) -> Result<Vec<Match>> {
        let mut command = Command::new("rg");
        command
            .arg("--json")
            .arg("--regexp")
            .arg(self.word.raw.replace(char::is_whitespace, ".*"))
            .arg("--type")
            .arg(&self.lang);
        if let Some(ref dir) = self.dir {
            command.current_dir(dir);
        }
        ExecutableSearcher::new(command)?.search(Some(comments))
    }

    /// Returns a tuple of (definition_kind, ripgrep_matches) by searching given language `lang`.
    fn find_definitions(&self, kind: &DefinitionKind) -> Result<(DefinitionKind, Vec<Match>)> {
        let regexp = build_full_regexp(&self.lang, kind, &self.word)
            .ok_or_else(|| Error::other("Can not find the definition rule"))?;
        let mut command = Command::new("rg");
        command
            .arg("--trim")
            .arg("--json")
            .arg("--pcre2")
            .arg("--regexp")
            .arg(regexp)
            .arg("--type")
            .arg(&self.lang);
        if let Some(ref dir) = self.dir {
            command.current_dir(dir);
        }
        ExecutableSearcher::new(command)?
            .search(None)
            .map(|defs| (kind.clone(), defs))
    }
}
