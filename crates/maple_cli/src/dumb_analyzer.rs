//! Inspired by https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el.
//!
//! This module requires the executable rg with `--json` and `--pcre2` is installed in the system.

use std::convert::TryFrom;
use std::path::PathBuf;
use std::{collections::HashMap, fmt::Display};

use anyhow::{anyhow, Result};
use once_cell::sync::{Lazy, OnceCell};
use serde::Deserialize;

use crate::process::tokio::TokioCommand;
use crate::tools::ripgrep::{Match, Word};

static RG_PCRE2_REGEX_RULES: OnceCell<HashMap<String, DefinitionRules>> = OnceCell::new();

static LANGUAGE_COMMENT_TABLE: OnceCell<HashMap<String, Vec<String>>> = OnceCell::new();

/// Map of file extension to language.
///
/// https://github.com/BurntSushi/ripgrep/blob/20534fad04/crates/ignore/src/default_types.rs
static LANGUAGE_EXT_TABLE: Lazy<HashMap<String, String>> = Lazy::new(|| {
    vec![
        ("clj", "clojure"),
        ("cpp", "cpp"),
        ("go", "go"),
        ("java", "java"),
        ("lua", "lua"),
        ("py", "python"),
        ("r", "r"),
        ("rb", "ruby"),
        ("rs", "rust"),
        ("scala", "scala"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
});

/// Finds the language given the file extension.
pub fn get_language_by_ext(ext: &str) -> Result<&str> {
    LANGUAGE_EXT_TABLE
        .get(ext)
        .map(|x| x.as_str())
        .ok_or_else(|| anyhow!("dumb_analyzer is unsupported for {}", ext))
}

/// Map of file extension to the comment prefix.
pub fn get_comments_by_ext(ext: &str) -> &[String] {
    let table = LANGUAGE_COMMENT_TABLE.get_or_init(|| {
        let comments: HashMap<String, Vec<String>> =
            serde_json::from_str(include_str!("../../../scripts/dumb_jump/comments_map.json"))
                .unwrap();
        comments
    });

    table.get(ext).unwrap_or_else(|| table.get("*").unwrap())
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
pub enum MatchKind {
    Definition(DefinitionKind),
    Reference(&'static str),
    /// Pure grep results.
    Occurrence(&'static str),
}

impl Display for MatchKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Definition(def_kind) => write!(f, "{}", def_kind.as_ref()),
            Self::Reference(ref_kind) => write!(f, "{}", ref_kind),
            Self::Occurrence(grep_kind) => write!(f, "{}", grep_kind),
        }
    }
}

impl AsRef<str> for MatchKind {
    fn as_ref(&self) -> &str {
        match self {
            Self::Definition(def_kind) => def_kind.as_ref(),
            Self::Reference(ref_kind) => ref_kind,
            Self::Occurrence(grep_kind) => grep_kind,
        }
    }
}

impl From<DefinitionKind> for MatchKind {
    fn from(def_kind: DefinitionKind) -> Self {
        Self::Definition(def_kind)
    }
}

/// Unit type wrapper of the kind of definition.
///
/// Possibale values: variable, function, type, etc.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
pub struct DefinitionKind(String);

impl AsRef<str> for DefinitionKind {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

/// Unit type wrapper of the regexp of a definition kind.
///
/// See more info in rg_pcre2_regex.json.
#[derive(Clone, Debug, Deserialize)]
pub struct DefinitionRegexp(Vec<String>);

impl DefinitionRegexp {
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter()
    }
}

/// Definition rules of a language.
#[derive(Clone, Debug, Deserialize)]
pub struct DefinitionRules(HashMap<DefinitionKind, DefinitionRegexp>);

impl DefinitionRules {
    pub fn kind_rules_for(&self, kind: &DefinitionKind) -> Result<impl Iterator<Item = &str>> {
        self.0
            .get(kind)
            .ok_or_else(|| anyhow!("invalid definition kind {:?} for the rules", kind))
            .map(|x| x.iter().map(|x| x.as_str()))
    }

    pub fn build_full_regexp(lang: &str, kind: &DefinitionKind, word: &Word) -> Result<String> {
        let regexp = LanguageDefinition::get_rules(lang)?
            .kind_rules_for(kind)?
            .map(|x| x.replace("\\\\", "\\"))
            .map(|x| x.replace("JJJ", &word.raw))
            .collect::<Vec<_>>()
            .join("|");
        Ok(regexp)
    }

    pub async fn all_definitions(
        lang: &str,
        word: Word,
        dir: &Option<PathBuf>,
    ) -> Result<Vec<(DefinitionKind, Vec<Match>)>> {
        let all_def_futures = LanguageDefinition::get_rules(lang)?
            .0
            .keys()
            .map(|kind| find_definition_matches_with_kind(lang, kind, &word, dir))
            .collect::<Vec<_>>();

        let maybe_defs = futures::future::join_all(all_def_futures).await;

        Ok(maybe_defs.into_iter().filter_map(|def| def.ok()).collect())
    }

    pub async fn definitions_and_references_lines(
        lang: &str,
        word: Word,
        dir: &Option<PathBuf>,
        comments: &[String],
    ) -> Result<crate::cmd::dumb_jump::Lines> {
        let (occurrences, definitions) = futures::future::join(
            find_all_occurrences_by_type(word.clone(), lang, dir, comments),
            Self::all_definitions(lang, word.clone(), dir),
        )
        .await;

        let (occurrences, definitions) = (occurrences?, definitions?);

        let defs = definitions
            .iter()
            .map(|(_, defs)| defs)
            .flatten()
            .collect::<Vec<_>>();

        // There are some negative definitions we need to filter them out, e.g., the word
        // is a subtring in some identifer but we consider every word is a valid identifer.
        let positive_defs = defs
            .iter()
            .filter(|def| occurrences.contains(def))
            .collect::<Vec<_>>();

        let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = definitions
            .iter()
            .flat_map(|(kind, lines)| {
                lines
                    .iter()
                    .filter(|line| positive_defs.contains(&line))
                    .map(|line| line.build_jump_line(kind.as_ref(), &word))
                    .collect::<Vec<_>>()
            })
            .chain(
                // references are these occurrences not in the definitions.
                occurrences
                    .iter()
                    .filter(|r| !defs.contains(&r))
                    .map(|line| line.build_jump_line("references", &word)),
            )
            .unzip();

        if lines.is_empty() {
            let lines = naive_grep_fallback(word.clone(), lang, dir, comments).await?;
            let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = lines
                .into_iter()
                .map(|line| line.build_jump_line("plain", &word))
                .unzip();
            return Ok(crate::cmd::dumb_jump::Lines::new(lines, indices));
        }

        Ok(crate::cmd::dumb_jump::Lines::new(lines, indices))
    }

    pub async fn definitions_and_references_matches(
        lang: &str,
        word: Word,
        dir: &Option<PathBuf>,
        comments: &[String],
    ) -> Result<Vec<Match>> {
        let (occurrences, definitions) = futures::future::join(
            find_all_occurrences_by_type(word.clone(), lang, dir, comments),
            Self::all_definitions(lang, word.clone(), dir),
        )
        .await;

        let (occurrences, definitions) = (occurrences?, definitions?);

        let defs = definitions
            .clone()
            .into_iter()
            .map(|(_, defs)| defs)
            .flatten()
            .collect::<Vec<Match>>();

        // There are some negative definitions we need to filter them out, e.g., the word
        // is a subtring in some identifer but we consider every word is a valid identifer.
        let positive_defs = defs
            .iter()
            .filter(|def| occurrences.contains(def))
            .collect::<Vec<_>>();

        let res = definitions
            .into_iter()
            .flat_map(|(_kind, lines)| {
                lines
                    .into_iter()
                    .filter(|ref line| positive_defs.contains(&line))
            })
            .chain({
                // references are these occurrences not in the definitions.
                occurrences.into_iter().filter(|r| !defs.contains(&r))
            })
            .collect::<Vec<Match>>();

        if res.is_empty() {
            naive_grep_fallback(word, lang, dir, comments).await
        } else {
            Ok(res)
        }
    }

    pub async fn definitions_and_references(
        lang: &str,
        word: Word,
        dir: &Option<PathBuf>,
        comments: &[String],
    ) -> Result<HashMap<MatchKind, Vec<Match>>> {
        let (occurrences, definitions) = futures::future::join(
            find_all_occurrences_by_type(word.clone(), lang, dir, comments),
            Self::all_definitions(lang, word.clone(), dir),
        )
        .await;

        let (occurrences, definitions) = (occurrences?, definitions?);

        let defs = definitions
            .clone()
            .into_iter()
            .map(|(_, defs)| defs)
            .flatten()
            .collect::<Vec<Match>>();

        // There are some negative definitions we need to filter them out, e.g., the word
        // is a subtring in some identifer but we consider every word is a valid identifer.
        let positive_defs = defs
            .iter()
            .filter(|def| occurrences.contains(def))
            .collect::<Vec<_>>();

        let res: HashMap<MatchKind, Vec<Match>> = definitions
            .into_iter()
            .filter_map(|(kind, lines)| {
                let defs = lines
                    .into_iter()
                    .filter(|ref line| positive_defs.contains(&line))
                    .collect::<Vec<_>>();

                if defs.is_empty() {
                    None
                } else {
                    Some((kind.into(), defs))
                }
            })
            .chain(std::iter::once((
                MatchKind::Reference("refs"),
                occurrences
                    .into_iter()
                    .filter(|r| !defs.contains(&r))
                    .collect::<Vec<_>>(),
            )))
            .collect();

        if res.is_empty() {
            // todo!()
            naive_grep_fallback(word, lang, dir, comments)
                .await
                .map(|results| std::iter::once((MatchKind::Occurrence("plain"), results)).collect())
        } else {
            Ok(res)
        }
    }
}

#[derive(Clone, Debug)]
pub struct LanguageDefinition;

impl LanguageDefinition {
    pub fn get_rules(lang: &str) -> Result<&DefinitionRules> {
        RG_PCRE2_REGEX_RULES
            .get_or_init(|| {
                let rules: HashMap<String, DefinitionRules> = serde_json::from_str(include_str!(
                    "../../../scripts/dumb_jump/rg_pcre2_regex.json"
                ))
                .unwrap();
                rules
            })
            .get(lang)
            .ok_or_else(|| anyhow!("Language {} is unsupported in dumb analyzer", lang))
    }
}

/// Executes the command as a child process, converting all the output into a stream of `JsonLine`.
async fn collect_matches(
    command: String,
    dir: &Option<PathBuf>,
    comments: Option<&[String]>,
) -> Result<Vec<Match>> {
    let mut tokio_cmd = TokioCommand::new(command);

    if let Some(ref dir) = dir {
        tokio_cmd.current_dir(dir.to_path_buf());
    }

    let lines = tokio_cmd.lines().await?;

    Ok(lines
        .iter()
        .filter_map(|s| Match::try_from(s.as_str()).ok())
        .filter(|mat| {
            // Filter out the comment line
            if let Some(comments) = comments {
                !comments
                    .iter()
                    .any(|c| mat.line().trim_start().starts_with(c))
            } else {
                true
            }
        })
        .collect())
}

/// Finds all the occurrences of `word`.
///
/// Basically the occurrences are composed of definitions and usages.
async fn find_all_occurrences_by_type(
    word: Word,
    lang_type: &str,
    dir: &Option<PathBuf>,
    comments: &[String],
) -> Result<Vec<Match>> {
    let command = format!(
        "rg --json --word-regexp '{}' --type {}",
        word.raw, lang_type
    );

    collect_matches(command, dir, Some(comments)).await
}

async fn naive_grep_fallback(
    word: Word,
    lang_type: &str,
    dir: &Option<PathBuf>,
    comments: &[String],
) -> Result<Vec<Match>> {
    let command = format!(
        "rg --json -e '{}' --type {}",
        word.raw.replace(char::is_whitespace, ".*"),
        lang_type
    );
    collect_matches(command, dir, Some(comments)).await
}

pub async fn find_occurrence_matches_by_ext(
    word: &Word,
    ext: &str,
    dir: &Option<PathBuf>,
) -> Result<Vec<Match>> {
    let command = format!("rg --json --word-regexp '{}' -g '*.{}'", word.raw, ext);
    let comments = get_comments_by_ext(ext);
    let occurrences = collect_matches(command, dir, Some(comments)).await?;

    Ok(occurrences)
}

#[allow(unused)]
async fn find_definitions_matches(
    lang: &str,
    kind: &DefinitionKind,
    word: &Word,
    dir: &Option<PathBuf>,
) -> Result<Vec<Match>> {
    let regexp = DefinitionRules::build_full_regexp(lang, kind, word)?;
    let command = format!("rg --trim --json --pcre2 --type {} -e '{}'", lang, regexp);
    collect_matches(command, dir, None).await
}

async fn find_definition_matches_with_kind(
    lang: &str,
    kind: &DefinitionKind,
    word: &Word,
    dir: &Option<PathBuf>,
) -> Result<(DefinitionKind, Vec<Match>)> {
    let regexp = DefinitionRules::build_full_regexp(lang, kind, word)?;
    let command = format!("rg --trim --json --pcre2 --type {} -e '{}'", lang, regexp);
    collect_matches(command, dir, None)
        .await
        .map(|defs| (kind.clone(), defs))
}
