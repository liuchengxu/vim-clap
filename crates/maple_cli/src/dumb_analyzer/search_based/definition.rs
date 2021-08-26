use std::path::PathBuf;
use std::{collections::HashMap, fmt::Display};

use anyhow::{anyhow, Result};
use once_cell::sync::{Lazy, OnceCell};
use serde::Deserialize;

use crate::command::dumb_jump::Lines;
use crate::tools::ripgrep::{Match, Word};
use crate::utils::ExactOrInverseTerms;

use super::runner::{
    find_definition_matches_with_kind, find_occurrences_by_lang, naive_grep_fallback,
};

/// A map of the ripgrep language to a set of regular expressions.
///
/// Ref: https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el.
static RG_PCRE2_REGEX_RULES: Lazy<HashMap<&str, DefinitionRules>> = Lazy::new(|| {
    serde_json::from_str(include_str!(
        "../../../../../scripts/dumb_jump/rg_pcre2_regex.json"
    ))
    .expect("Wrong path for rg_pcre2_regex.json")
});

/// Map of file extension to ripgrep language.
///
/// https://github.com/BurntSushi/ripgrep/blob/20534fad04/crates/ignore/src/default_types.rs
static RG_LANGUAGE_EXT_TABLE: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    super::default_types::DEFAULT_TYPES
        .iter()
        .flat_map(|(lang, values)| {
            values
                .iter()
                .filter_map(|v| {
                    v.split('.').last().and_then(|ext| {
                        // Simply ignore the abnormal cases.
                        if ext.contains('[') || ext.contains('*') {
                            None
                        } else {
                            Some((ext, *lang))
                        }
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
});

/// Finds the ripgrep language given the file extension `ext`.
pub fn get_language_by_ext(ext: &str) -> Result<&&str> {
    RG_LANGUAGE_EXT_TABLE
        .get(ext)
        .ok_or_else(|| anyhow!("dumb_analyzer is unsupported for {}", ext))
}

/// Map of file extension to the comment prefix.
///
/// Keyed by the extension name.
pub fn get_comments_by_ext(ext: &str) -> &[&str] {
    static LANGUAGE_COMMENT_TABLE: OnceCell<HashMap<&str, Vec<&str>>> = OnceCell::new();

    let table = LANGUAGE_COMMENT_TABLE.get_or_init(|| {
        let comments: HashMap<&str, Vec<&str>> = serde_json::from_str(include_str!(
            "../../../../../scripts/dumb_jump/comments_map.json"
        ))
        .expect("Wrong path for comments_map.json");
        comments
    });

    table
        .get(ext)
        .unwrap_or_else(|| table.get("*").expect("`*` entry exists; qed"))
}

/// Type of match result of ripgrep.
#[derive(Clone, Debug, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum MatchKind {
    /// Results matched from the definition regexp.
    Definition(DefinitionKind),
    /// Occurrences with the definition items ignored.
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
    fn kind_rules_for(&self, kind: &DefinitionKind) -> Result<impl Iterator<Item = &str>> {
        self.0
            .get(kind)
            .ok_or_else(|| anyhow!("invalid definition kind {:?} for the rules", kind))
            .map(|x| x.iter().map(|x| x.as_str()))
    }
}

/// Returns the definition rules given `lang`.
pub fn get_definition_rules(lang: &str) -> Result<&DefinitionRules> {
    /// A map of extension => ripgrep language.
    static EXTENSION_LANGUAGE_MAP: Lazy<HashMap<&str, &str>> =
        Lazy::new(|| [("js", "javascript")].iter().cloned().collect());

    match RG_PCRE2_REGEX_RULES.get(lang) {
        Some(rules) => Ok(rules),
        None => EXTENSION_LANGUAGE_MAP
            .get(lang)
            .and_then(|l| RG_PCRE2_REGEX_RULES.get(l))
            .ok_or_else(|| {
                anyhow!(
                    "Language {} can not be found in dumb_jump/rg_pcre2_regex.json",
                    lang
                )
            }),
    }
}

pub fn build_full_regexp(lang: &str, kind: &DefinitionKind, word: &Word) -> Result<String> {
    use itertools::Itertools;
    let regexp = get_definition_rules(lang)?
        .kind_rules_for(kind)?
        .map(|x| x.replace("\\\\", "\\"))
        .map(|x| x.replace("JJJ", &word.raw))
        .join("|");
    Ok(regexp)
}

/// Returns true if the ripgrep match is a comment line.
#[inline]
pub(super) fn is_comment(mat: &Match, comments: &[&str]) -> bool {
    comments.iter().any(|c| mat.line_starts_with(c))
}

/// Collects all kinds of definitions concurrently.
pub async fn all_definitions(
    lang: &str,
    word: &Word,
    dir: &Option<PathBuf>,
) -> Result<Vec<(DefinitionKind, Vec<Match>)>> {
    let all_def_futures = get_definition_rules(lang)?
        .0
        .keys()
        .map(|kind| find_definition_matches_with_kind(lang, kind, &word, dir));

    let maybe_defs = futures::future::join_all(all_def_futures).await;

    Ok(maybe_defs.into_iter().filter_map(|def| def.ok()).collect())
}

/// Collects the occurrences and all definitions concurrently.
async fn definitions_and_occurences(
    word: &Word,
    lang: &str,
    dir: &Option<PathBuf>,
    comments: &[&str],
) -> (Vec<(DefinitionKind, Vec<Match>)>, Vec<Match>) {
    let (definitions, occurrences) = futures::future::join(
        all_definitions(lang, word, dir),
        find_occurrences_by_lang(word, lang, dir, comments),
    )
    .await;

    (
        definitions.unwrap_or_default(),
        occurrences.unwrap_or_default(),
    )
}

fn flatten(definitions: &[(DefinitionKind, Vec<Match>)]) -> Vec<Match> {
    let defs_count = definitions.iter().map(|(_, items)| items.len()).sum();
    let mut defs = Vec::with_capacity(defs_count);
    for (_, items) in definitions.iter() {
        defs.extend_from_slice(items);
    }
    defs
}

pub async fn definitions_and_references_lines(
    lang: &str,
    word: &Word,
    dir: &Option<PathBuf>,
    comments: &[&str],
    exact_or_inverse_terms: &ExactOrInverseTerms,
) -> Result<Lines> {
    let (definitions, occurrences) = definitions_and_occurences(word, lang, dir, comments).await;

    let defs = flatten(&definitions);

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
                .filter_map(|ref line| {
                    if positive_defs.contains(&line) {
                        exact_or_inverse_terms
                            .check_jump_line(line.build_jump_line(kind.as_ref(), &word))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .chain(
            // references are these occurrences not in the definitions.
            occurrences.iter().filter_map(|ref line| {
                if !defs.contains(&line) {
                    exact_or_inverse_terms.check_jump_line(line.build_jump_line("refs", &word))
                } else {
                    None
                }
            }),
        )
        .unzip();

    if lines.is_empty() {
        let lines = naive_grep_fallback(word, lang, dir, comments).await?;
        let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = lines
            .into_iter()
            .filter_map(|line| {
                exact_or_inverse_terms.check_jump_line(line.build_jump_line("plain", &word))
            })
            .unzip();
        return Ok(Lines::new(lines, indices));
    }

    Ok(Lines::new(lines, indices))
}

pub async fn definitions_and_references(
    lang: &str,
    word: &Word,
    dir: &Option<PathBuf>,
    comments: &[&str],
) -> Result<HashMap<MatchKind, Vec<Match>>> {
    let (definitions, mut occurrences) =
        definitions_and_occurences(word, lang, dir, comments).await;

    let defs = flatten(&definitions);

    // There are some negative definitions we need to filter them out, e.g., the word
    // is a subtring in some identifer but we consider every word is a valid identifer.
    let positive_defs = defs
        .iter()
        .filter(|def| occurrences.contains(def))
        .collect::<Vec<_>>();

    let res: HashMap<MatchKind, Vec<Match>> = definitions
        .into_iter()
        .filter_map(|(kind, mut defs)| {
            defs.retain(|ref def| positive_defs.contains(&def));
            if defs.is_empty() {
                None
            } else {
                Some((kind.into(), defs))
            }
        })
        .chain(std::iter::once((MatchKind::Reference("refs"), {
            occurrences.retain(|r| !defs.contains(&r));
            occurrences
        })))
        .collect();

    if res.is_empty() {
        naive_grep_fallback(word, lang, dir, comments)
            .await
            .map(|results| std::iter::once((MatchKind::Occurrence("plain"), results)).collect())
    } else {
        Ok(res)
    }
}
